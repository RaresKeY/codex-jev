//! Private subprocesses keep the bundled-helper boundary realistic without shared test state.

use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;
use wiremock::ResponseTemplate;

const HELPER: &str = r#"import json, os, sys
from pathlib import Path
request = json.load(sys.stdin)
with Path(__file__).with_name('requests.jsonl').open('a') as output:
    output.write(json.dumps(request) + '\n')
if os.environ['CODEX_JEV_TEST_CASE'] == 'budget':
    print(json.dumps({'error': 'Jev weekly token budget exhausted.'}))
    raise SystemExit(1)
entries = request['context']['transcript']['entries']
effort = 'high' if any(item['role'] == 'tool_output' for item in entries) else 'low'
print(json.dumps({'model': request['model'], 'effort': effort}))
"#;

#[test_case("steps")]
#[test_case("retry")]
#[test_case("budget")]
fn jev_effort_backend(case: &str) -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let executable = temp
        .path()
        .join(format!("jev-tests{}", std::env::consts::EXE_SUFFIX));
    let current = std::env::current_exe()?;
    if std::fs::hard_link(&current, &executable).is_err() {
        std::fs::copy(&current, &executable)?;
    }
    std::fs::write(temp.path().join("jev-route.py"), HELPER)?;
    let mut command = assert_cmd::Command::new(executable);
    for binary in ["codex", "codex-exec", "codex-code-mode-host"] {
        if let Ok(path) = codex_utils_cargo_bin::cargo_bin(binary) {
            command.env(format!("CARGO_BIN_EXE_{binary}"), path);
        }
    }
    command
        .args([
            "--exact",
            "suite::jev_effort::jev_effort_isolated_child",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_JEV_TEST_CASE", case)
        .timeout(Duration::from_secs(/*secs*/ 90))
        .assert()
        .success();
    assert!(
        temp.path().join("requests.jsonl").is_file(),
        "isolated test did not run the helper"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "invoked by jev_effort_backend beside a private fake helper"]
async fn jev_effort_isolated_child() -> anyhow::Result<()> {
    let Ok(case) = std::env::var("CODEX_JEV_TEST_CASE") else {
        return Ok(());
    };
    let server = responses::start_mock_server().await;
    let completion = responses::sse(vec![
        responses::ev_assistant_message("answer", "done"),
        responses::ev_completed("done"),
    ]);
    let captured = match case.as_str() {
        "steps" => Some(
            responses::mount_sse_sequence(
                &server,
                vec![
                    responses::sse(vec![
                        responses::ev_function_call("tool", "unknown_jev_test_tool", "{}"),
                        responses::ev_completed("tool-step"),
                    ]),
                    completion,
                ],
            )
            .await,
        ),
        "retry" => Some(
            responses::mount_response_sequence(
                &server,
                vec![
                    ResponseTemplate::new(/*status_code*/ 500)
                        .set_body_json(json!({"error":{"message":"retry","type":"server_error"}})),
                    ResponseTemplate::new(/*status_code*/ 200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string(completion),
                ],
            )
            .await,
        ),
        "budget" => None,
        _ => anyhow::bail!("unknown test case"),
    };
    let test = test_codex()
        .with_model_info_override("gpt-6.1-sol", |model| {
            model.use_responses_lite = true;
            model.supports_reasoning_effort_updates = true;
            model.default_reasoning_level = Some(ReasoningEffort::Low);
            model.supported_reasoning_levels = [ReasoningEffort::Low, ReasoningEffort::High]
                .into_iter()
                .map(|effort| ReasoningEffortPreset {
                    effort,
                    description: "test".to_string(),
                })
                .collect();
        })
        .with_config(|config| {
            config.model = Some("gpt-6.1-sol".to_string());
            config.model_provider_id = "openai".to_string();
            config.model_provider.supports_websockets = false;
            config.model_reasoning_effort = Some(ReasoningEffort::Custom("jev".to_string()));
            config
                .features
                .enable(Feature::ReasoningEffortOverride)
                .expect("enable effort updates");
        })
        .build_with_auto_env(&server)
        .await?;
    if case == "budget" {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Complete the current task.".to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        let event = wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await;
        let EventMsg::Error(error) = event else {
            unreachable!()
        };
        assert!(error.message.contains("Jev weekly token budget exhausted."));
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    } else {
        test.submit_text_turn("Complete the current task.").await?;
    }
    let log = std::env::current_exe()?.with_file_name("requests.jsonl");
    let decisions: Vec<Value> = std::fs::read_to_string(log)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    if case == "budget" {
        let executor_requests = server
            .received_requests()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|request| {
                request.method.as_str() == "POST" && request.url.path().contains("responses")
            })
            .count();
        assert_eq!((decisions.len(), executor_requests), (1, 0));
        return Ok(());
    }
    let requests = captured.expect("responses mock").requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request.body_json()["model"] == "gpt-6.1-sol")
    );
    let baseline = requests[0].body_json()["reasoning"]["effort"].clone();
    assert_ne!(baseline, json!("jev"));
    assert_eq!(requests[1].body_json()["reasoning"]["effort"], baseline);
    if case == "retry" {
        assert_eq!(decisions.len(), 1);
    } else {
        assert_eq!(decisions.len(), 2);
        assert!(
            decisions[1]["context"]["transcript"]["entries"]
                .as_array()
                .expect("entries")
                .iter()
                .any(|entry| entry["role"] == "tool_output")
        );
        let latest = requests[1]
            .input()
            .into_iter()
            .filter(|item| item["type"] == "configuration_update")
            .last()
            .expect("effort update");
        assert_eq!(latest["reasoning"]["effort"], "high");
    }
    Ok(())
}
