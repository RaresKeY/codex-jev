//! Private subprocesses keep the bundled-helper boundary realistic without shared test state.

use codex_core::StartIfIdleSubmission;
use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_protocol::models::ImageReference;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;
use wiremock::ResponseTemplate;

const SKILL_TASK: &str = "Use $jev-effort-skill to find the cause of the failing test.";
const AGENT_EVIDENCE: &str = "The worker found an unresolved race.";
const IMAGE_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

const HELPER: &str = r#"import json, os, sys
from pathlib import Path
request = json.load(sys.stdin)
with Path(__file__).with_name('requests.jsonl').open('a') as output:
    output.write(json.dumps(request) + '\n')
if os.environ['CODEX_JEV_TEST_CASE'] == 'budget':
    print(json.dumps({'error': 'Jev weekly token budget exhausted.'}))
    raise SystemExit(1)
if os.environ['CODEX_JEV_TEST_CASE'] == 'image':
    assert request['model'] == 'gpt-6-luna'
    assert request['context']['images_attached'] is True
    print(json.dumps({'error': 'Select an image-capable model before using Jev effort routing.'}))
    raise SystemExit(1)
entries = request['context']['transcript']['entries']
has_worker_evidence = any(item['role'] == 'assistant' and item['text'] == 'The worker found an unresolved race.' for item in entries)
effort = 'high' if has_worker_evidence or any(item['role'] == 'tool_output' for item in entries) else 'low'
print(json.dumps({'model': request['model'], 'effort': effort}))
"#;

#[test_case("steps")]
#[test_case("retry")]
#[test_case("budget")]
#[test_case("skill")]
#[test_case("agent")]
#[test_case("image")]
fn jev_effort_backend(case: &str) -> anyhow::Result<()> {
    if case == "skill" {
        skip_if_wine_exec!(
            Ok(()),
            "skill paths require matching host and executor path conventions"
        );
    }
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
        "steps" | "skill" => Some(
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
        "agent" => Some(responses::mount_sse_once(&server, completion).await),
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
        "budget" | "image" => None,
        _ => anyhow::bail!("unknown test case"),
    };
    let selected_model = if case == "image" {
        "gpt-6-luna"
    } else {
        "gpt-6.1-sol"
    };
    let mut builder = test_codex()
        .with_model_info_override(selected_model, move |model| {
            model.use_responses_lite = true;
            model.supports_reasoning_effort_updates = true;
            model.default_reasoning_level = Some(ReasoningEffort::Low);
            let higher_effort = if selected_model == "gpt-6-luna" {
                model.input_modalities = vec![InputModality::Text];
                ReasoningEffort::Medium
            } else {
                ReasoningEffort::High
            };
            model.supported_reasoning_levels = [ReasoningEffort::Low, higher_effort]
                .into_iter()
                .map(|effort| ReasoningEffortPreset {
                    effort,
                    description: "test".to_string(),
                })
                .collect();
        })
        .with_config(move |config| {
            config.model = Some(selected_model.to_string());
            config.model_provider_id = "openai".to_string();
            config.model_provider.supports_websockets = false;
            config.model_reasoning_effort = Some(ReasoningEffort::Custom("jev".to_string()));
            config
                .features
                .enable(Feature::ReasoningEffortOverride)
                .expect("enable effort updates");
        });
    if case == "skill" {
        builder = builder.with_workspace_setup(|cwd, fs| async move {
            let directory = cwd.join(".agents/skills/jev-effort-skill");
            fs.create_directory(
                &PathUri::from_host_native_path(&directory)?,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_host_native_path(&directory.join("SKILL.md"))?,
                b"---\nname: jev-effort-skill\ndescription: Inspect a failing test.\n---\n\nUse the established debugging workflow.\n".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    }
    let test = builder.build_with_auto_env(&server).await?;
    if matches!(case.as_str(), "budget" | "image") {
        let mut input = vec![UserInput::Text {
            text: "Complete the current task.".to_string(),
            text_elements: Vec::new(),
        }];
        if case == "image" {
            input.push(UserInput::Image {
                image: ImageReference::Inline {
                    image_url: IMAGE_URL.to_string(),
                },
                detail: None,
            });
        }
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(input))
            .await?;
        let event = wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await;
        let EventMsg::Error(error) = event else {
            unreachable!()
        };
        let expected_error = if case == "image" {
            "Select an image-capable model before using Jev effort routing."
        } else {
            "Jev weekly token budget exhausted."
        };
        assert!(error.message.contains(expected_error));
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    } else if case == "agent" {
        let agent_message = serde_json::from_value(json!({
            "type": "agent_message", "author": "/root/worker", "recipient": "/root",
            "content": [{"type": "input_text", "text": AGENT_EVIDENCE}],
        }))?;
        let submission = test
            .codex
            .start_turn_if_idle(TurnInputRequest::new(TurnInput::ResponseItem(
                agent_message,
            )))
            .await?;
        assert!(matches!(submission, StartIfIdleSubmission::Started { .. }));
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    } else if case == "skill" {
        let skill_path = test
            .config
            .cwd
            .join(".agents/skills/jev-effort-skill/SKILL.md");
        let skill_path = skill_path
            .canonicalize()
            .unwrap_or(skill_path)
            .to_path_buf();
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![
                UserInput::Text {
                    text: SKILL_TASK.to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "jev-effort-skill".to_string(),
                    path: skill_path,
                },
            ]))
            .await?;
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
    if matches!(case.as_str(), "budget" | "image") {
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
        if case == "image" {
            assert_eq!(decisions[0]["context"]["images_attached"], true);
            let payload = serde_json::to_string(&decisions[0])?;
            assert!(!payload.contains(IMAGE_URL));
            assert!(!payload.contains("image_url"));
            assert!(!payload.contains("data:image/"));
        }
        return Ok(());
    }
    let requests = captured.expect("responses mock").requests();
    assert!(
        requests
            .iter()
            .all(|request| request.body_json()["model"] == "gpt-6.1-sol")
    );
    if case == "agent" {
        assert_eq!((decisions.len(), requests.len()), (1, 1));
        let evidence = decisions[0]["context"]["transcript"]["entries"]
            .as_array()
            .expect("entries");
        assert!(
            evidence
                .iter()
                .any(|entry| { entry["role"] == "assistant" && entry["text"] == AGENT_EVIDENCE })
        );
        assert_eq!(requests[0].inputs_of_type("agent_message").len(), 1);
        let updates = requests[0].inputs_of_type("configuration_update");
        assert_eq!(
            updates.last().expect("effort update")["reasoning"]["effort"],
            "high"
        );
        return Ok(());
    }
    assert_eq!(requests.len(), 2);
    let baseline = requests[0].body_json()["reasoning"]["effort"].clone();
    assert_ne!(baseline, json!("jev"));
    assert_eq!(requests[1].body_json()["reasoning"]["effort"], baseline);
    if case == "retry" {
        assert_eq!(decisions.len(), 1);
    } else {
        assert_eq!(decisions.len(), 2);
        let evidence = decisions[1]["context"]["transcript"]["entries"]
            .as_array()
            .expect("entries");
        assert!(evidence.iter().any(|entry| entry["role"] == "tool_output"));
        if case == "skill" {
            assert_eq!(
                decisions
                    .iter()
                    .map(|decision| decision["task"].clone())
                    .collect::<Vec<_>>(),
                vec![json!(SKILL_TASK); 2],
            );
            assert!(
                requests[0].message_input_texts("user").iter().any(|text| {
                    text.contains("<skill>") && text.contains("<name>jev-effort-skill</name>")
                }),
                "the test must exercise a real injected skill message"
            );
            assert!(requests[0].has_content_kinds(&["skills.selected_skill_instructions"]));
        }
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
