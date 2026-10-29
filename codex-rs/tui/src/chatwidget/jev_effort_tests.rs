use super::*;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn jev_effort_selection_preserves_model_and_submits_the_mode_once() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_model("gpt-6.1-sol");
    chat.enable_jev_auto();
    chat.enable_jev_effort();
    let marker = ReasoningEffortConfig::Custom("jev".to_string());
    assert_eq!(
        (
            chat.jev_auto.enabled,
            chat.current_model(),
            chat.effective_reasoning_effort(),
        ),
        (false, "gpt-6.1-sol", Some(marker.clone())),
    );
    let item = chat
        .jev_effort_picker_item()
        .expect("supported current model");
    assert!(item.is_current);
    insta::assert_snapshot!(
        format!("{}\n{}", item.name, item.description.unwrap()),
        @r###"
Effort only (Jev)
Keep gpt-6.1-sol; choose effort before every model step
"###
    );
    while events.try_recv().is_ok() {}
    (item.actions[0])(&chat.app_event_tx);
    assert!(matches!(
        events.try_recv(),
        Ok(AppEvent::EnableJevEffort { thread_id, model })
            if thread_id == chat.thread_id && model == "gpt-6.1-sol"
    ));
    chat.handle_composer_input_result(
        InputResult::Submitted {
            text: "continue the implementation".to_string(),
            text_elements: Vec::new(),
        },
        /*had_modal_or_popup*/ false,
    );
    assert!(chat.jev_auto.pending.is_none());
    let turns = std::iter::from_fn(|| ops.try_recv().ok())
        .filter_map(|op| match op {
            crate::app_command::AppCommand::UserTurn { effort, .. } => Some(effort),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(turns, vec![Some(marker)]);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Low));
    assert!(!chat.jev_effort_enabled());
}

#[tokio::test]
async fn jev_effort_picker_does_not_offer_unsupported_models_or_reserve() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    for model in ["custom-model", crate::model_catalog::LUNA_RESERVE_MODEL] {
        chat.set_model(model);
        assert!(chat.jev_effort_picker_item().is_none());
    }
}

#[tokio::test]
async fn manual_effort_exits_jev_from_plan_and_remains_manual_in_default() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.set_model("gpt-6.1-sol");
    chat.enable_jev_effort();
    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);
    assert!(chat.jev_effort_enabled());

    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Low));
    assert_eq!(
        (
            chat.current_collaboration_mode().reasoning_effort(),
            chat.config.plan_mode_reasoning_effort.clone(),
            chat.effective_reasoning_effort(),
        ),
        (
            Some(ReasoningEffortConfig::Low),
            Some(ReasoningEffortConfig::Low),
            Some(ReasoningEffortConfig::Low),
        ),
    );
    let default_mask = collaboration_modes::default_mask(chat.model_catalog.as_ref())
        .expect("expected default collaboration mode");
    chat.set_collaboration_mask(default_mask);
    assert_eq!(
        (chat.current_model(), chat.effective_reasoning_effort()),
        ("gpt-6.1-sol", Some(ReasoningEffortConfig::Low)),
    );
    assert!(!chat.jev_effort_enabled());
}

#[tokio::test]
async fn manual_plan_effort_clears_jev_before_switching_from_default_to_plan() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.set_model("gpt-6.1-sol");
    chat.enable_jev_effort();
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::High));
    assert_eq!(
        (
            chat.current_collaboration_mode().reasoning_effort(),
            chat.config.plan_mode_reasoning_effort.clone(),
            chat.effective_reasoning_effort(),
        ),
        (
            Some(ReasoningEffortConfig::High),
            Some(ReasoningEffortConfig::High),
            Some(ReasoningEffortConfig::High),
        ),
    );
    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);
    assert_eq!(
        (chat.current_model(), chat.effective_reasoning_effort()),
        ("gpt-6.1-sol", Some(ReasoningEffortConfig::High)),
    );
    assert!(!chat.jev_effort_enabled());
}
