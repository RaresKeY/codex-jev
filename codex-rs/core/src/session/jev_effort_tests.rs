use super::*;
use pretty_assertions::assert_eq;

#[test]
fn jev_effort_keeps_latest_image_only_request_after_many_tools() {
    let old = serde_json::from_value(json!({
        "type": "message", "role": "user",
        "content": [{"type": "input_text", "text": "unrelated older ask"}],
    }))
    .expect("older message");
    let image = serde_json::from_value(json!({
        "type": "message", "role": "user",
        "content": [{"type": "input_image", "image_url": "private-image"}],
    }))
    .expect("image-only message");
    let mut history = vec![old, image];
    for index in 0..40 {
        history.push(serde_json::from_value(json!({
            "type": "function_call_output", "call_id": format!("call-{index}"), "output": "done",
        })).expect("tool output"));
    }
    let payload = request(&history, "gpt-6.1-sol", &[ReasoningEffort::Low]);
    assert_eq!(
        (&payload["task"], &payload["context"]["images_attached"]),
        (&json!(""), &json!(true))
    );
    assert!(
        !serde_json::to_string(&payload)
            .expect("payload")
            .contains("private-image")
    );
}

#[test]
fn jev_effort_bounds_recent_evidence_and_omits_reasoning_and_media() {
    let mut history = vec![
        serde_json::from_value(json!({
            "type": "message", "role": "user", "content": [
                {"type": "input_text", "text": "task".repeat(20_000)},
                {"type": "input_image", "image_url": "private-image"},
            ],
        }))
        .expect("user message"),
    ];
    for index in 0..40 {
        history.push(
            serde_json::from_value(json!({
                "type": "function_call_output", "call_id": format!("call-{index}"),
                "output": format!("{}\nfinal error {index}", "é".repeat(10_000)),
            }))
            .expect("tool output"),
        );
    }
    history.push(
        serde_json::from_value(json!({
            "type": "reasoning", "summary": [], "encrypted_content": "private-reasoning",
        }))
        .expect("reasoning"),
    );
    let payload = request(&history, "gpt-6.1-sol", &[ReasoningEffort::Low]);
    let transcript = &payload["context"]["transcript"];
    let entries = transcript["entries"].as_array().expect("entries");
    assert!(entries.len() <= 24);
    assert!(
        serde_json::to_vec(transcript)
            .expect("transcript JSON")
            .len()
            <= 16_000
    );
    assert!(
        entries
            .iter()
            .all(|entry| entry["text"].as_str().expect("entry text").len() <= 1536)
    );
    assert!(
        entries.last().expect("last entry")["text"]
            .as_str()
            .expect("text")
            .ends_with("final error 39")
    );
    assert_eq!(transcript["omitted"], true);
    assert!(payload["task"].as_str().expect("task").len() <= 2048);
    let serialized = serde_json::to_string(&payload).expect("payload JSON");
    assert!(!serialized.contains("private-image"));
    assert!(!serialized.contains("private-reasoning"));
}

#[test]
fn jev_effort_rejects_model_changes_and_unsupported_effort() {
    let allowed = [ReasoningEffort::Low, ReasoningEffort::High];
    for output in [
        br#"{"model":"gpt-6-astra","effort":"low"}"#.as_slice(),
        br#"{"model":"gpt-6.1-sol","effort":"max"}"#.as_slice(),
        br#"{"model":"gpt-6.1-sol","effort":"jev"}"#.as_slice(),
        br#"{"model":"gpt-6.1-sol","effort":"low","unexpected":true}"#.as_slice(),
    ] {
        assert!(parse_decision(output, "gpt-6.1-sol", &allowed).is_err());
    }
    assert_eq!(
        parse_decision(
            br#"{"model":"gpt-6.1-sol","effort":"high","stats":{"effort_confidence":0.9,"input_tokens":120,"output_tokens":8}}"#,
            "gpt-6.1-sol",
            &allowed,
        ),
        Ok(ReasoningEffort::High),
    );
}

#[tokio::test]
async fn jev_effort_preserves_safe_budget_error() {
    let temp = tempfile::tempdir().expect("temporary helper directory");
    let helper = temp.path().join("jev-route.py");
    std::fs::write(
        &helper,
        "import json, sys\njson.load(sys.stdin)\nprint(json.dumps({'error':'Jev weekly token budget exhausted.'}))\nraise SystemExit(1)\n",
    )
    .expect("write helper");
    assert_eq!(
        choose(
            &helper,
            &serde_json::json!({"task":"continue"}),
            "gpt-6.1-sol",
            &[ReasoningEffort::Low],
        )
        .await,
        Err("Jev weekly token budget exhausted.".to_string()),
    );
}

#[tokio::test]
async fn jev_effort_rejects_unbounded_helper_output() {
    let temp = tempfile::tempdir().expect("temporary helper directory");
    let helper = temp.path().join("jev-route.py");
    std::fs::write(&helper, "import sys\nsys.stdin.read()\nprint('x' * 4097)\n")
        .expect("write helper");
    assert_eq!(
        choose(
            &helper,
            &serde_json::json!({"task":"continue"}),
            "gpt-6.1-sol",
            &[ReasoningEffort::Low],
        )
        .await,
        Err("Jev helper output exceeded its limit.".to_string()),
    );
}
