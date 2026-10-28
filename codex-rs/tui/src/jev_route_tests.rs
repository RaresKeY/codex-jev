use super::*;
use pretty_assertions::assert_eq;

#[test]
fn context_bounds_unicode_evidence_and_discards_stale_response() {
    let mut context = JevContext::default();
    for request in [
        "old request",
        "retained request",
        "continue",
        &"界".repeat(300),
    ] {
        context.record_request(request);
    }
    context.recent_response = Some(JevExcerpt::new("old answer"));
    context.last_turn_status = Some("completed");
    context.record_request("new task");

    assert_eq!(
        serde_json::to_value(context).unwrap(),
        serde_json::json!({
            "source": "current_widget_visible_history",
            "partial": true,
            "recent_requests": [
                {"text": "continue", "truncated": false},
                {"text": "界".repeat(256), "truncated": true},
                {"text": "new task", "truncated": false},
            ],
            "recent_response": null,
            "last_turn_status": null,
            "active_goal": null,
            "images_attached": false,
        })
    );
}

#[test]
fn excerpt_does_not_split_a_multibyte_character() {
    assert_eq!(
        JevExcerpt::new(&format!("{}界", "x".repeat(767))),
        JevExcerpt {
            text: "x".repeat(767),
            truncated: true
        },
    );
}

#[test]
fn helper_metadata_is_validated_and_legacy_decisions_omit_missing_metrics() {
    let mut output = serde_json::json!({
        "model": "gpt-6.1-terra", "effort": "medium",
        "stats": {
            "model_confidence": 0.82, "effort_confidence": 0.91,
            "input_tokens": 480, "output_tokens": 12,
        },
    });
    let decision = parse_decision(&serde_json::to_vec(&output).unwrap()).unwrap();
    let stats = decision.stats.unwrap();
    assert_eq!((stats.input_tokens, stats.output_tokens), (480, 12));
    assert_eq!(
        (stats.model_confidence, stats.effort_confidence),
        (0.82, 0.91)
    );
    for (field, invalid) in [
        ("model_confidence", serde_json::json!(-0.1)),
        ("effort_confidence", serde_json::json!(1.01)),
        ("model_confidence", serde_json::json!("NaN")),
        ("input_tokens", serde_json::json!(-1)),
        ("output_tokens", serde_json::json!(1.5)),
    ] {
        let mut bad = output.clone();
        bad["stats"][field] = invalid;
        assert!(parse_decision(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    output.as_object_mut().unwrap().remove("stats");
    let legacy = parse_decision(&serde_json::to_vec(&output).unwrap()).unwrap();
    assert_eq!(legacy.model, "gpt-6.1-terra");
    assert!(legacy.stats.is_none());
}
