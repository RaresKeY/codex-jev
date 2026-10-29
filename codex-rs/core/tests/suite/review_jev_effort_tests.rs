//! Review delegates use concrete effort without running the root-only Jev router.

use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(ReasoningEffort::Custom("jev".to_string()), None, ReasoningEffort::Medium; "jev inherited model")]
#[test_case(ReasoningEffort::Custom("jev".to_string()), Some("gpt-6.1-sol"), ReasoningEffort::Medium; "jev explicit same model")]
#[test_case(ReasoningEffort::Custom("jev".to_string()), Some("gpt-5.4"), ReasoningEffort::Low; "jev different model compatibility")]
#[test_case(ReasoningEffort::High, None, ReasoningEffort::High; "manual inherited model")]
#[test_case(ReasoningEffort::High, Some("gpt-5.4"), ReasoningEffort::High; "manual different model")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_inherits_concrete_effort(
    parent_effort: ReasoningEffort,
    review_model: Option<&str>,
    expected_effort: ReasoningEffort,
) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("review-answer", "Review completed successfully."),
            responses::ev_completed("review-done"),
        ]),
    )
    .await;
    let review_model = review_model.map(str::to_owned);
    let expected_model = review_model.as_deref().unwrap_or("gpt-6.1-sol").to_owned();
    let test = test_codex()
        .with_model_info_override("gpt-6.1-sol", |model| {
            model.default_reasoning_level = Some(ReasoningEffort::Medium);
            model.supported_reasoning_levels = [
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
            ]
            .into_iter()
            .map(|effort| ReasoningEffortPreset {
                effort,
                description: "test".to_string(),
            })
            .collect();
        })
        .with_model_info_override("gpt-5.4", |model| {
            model.default_reasoning_level = Some(ReasoningEffort::High);
            model.supported_reasoning_levels = [ReasoningEffort::Low, ReasoningEffort::High]
                .into_iter()
                .map(|effort| ReasoningEffortPreset {
                    effort,
                    description: "test".to_string(),
                })
                .collect();
        })
        .with_config(move |config| {
            config.model = Some("gpt-6.1-sol".to_string());
            config.model_provider_id = "openai".to_string();
            config.model_provider.supports_websockets = false;
            config.model_reasoning_effort = Some(parent_effort);
            config.review_model = review_model;
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Review the current change.".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await?;
    let exited = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExitedReviewMode(_))
    })
    .await;
    let EventMsg::ExitedReviewMode(exited) = exited else {
        unreachable!()
    };
    assert_eq!(
        exited
            .review_output
            .map(|output| output.overall_explanation),
        Some("Review completed successfully.".to_string()),
    );
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let request = response.single_request().body_json();
    assert_eq!(
        (
            request["model"].clone(),
            request["reasoning"]["effort"].clone()
        ),
        (json!(expected_model), json!(expected_effort)),
    );
    Ok(())
}
