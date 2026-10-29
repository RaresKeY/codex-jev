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

#[tokio::test]
async fn jev_effort_manual_server_setting_clears_both_mode_markers() {
    for mode in [ModeKind::Default, ModeKind::Plan] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        chat.thread_id = Some(ThreadId::new());
        chat.set_model("gpt-6.1-sol");
        chat.enable_jev_effort();
        chat.set_collaboration_mask(
            collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), mode)
                .expect("collaboration mode"),
        );

        let update = jev_settings_notification(&chat, ReasoningEffortConfig::Low);
        chat.on_thread_settings_updated(update);
        assert!(!chat.jev_effort_configured());
        for next_mode in [ModeKind::Default, ModeKind::Plan] {
            chat.set_collaboration_mask(
                collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), next_mode)
                    .expect("collaboration mode"),
            );
            assert_eq!(
                (chat.jev_auto.enabled, chat.effective_reasoning_effort()),
                (false, Some(ReasoningEffortConfig::Low)),
            );
        }
    }
}

#[tokio::test]
async fn jev_effort_server_marker_restores_both_modes_preserving_auto_choice() {
    for auto_enabled in [false, true] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        chat.thread_id = Some(ThreadId::new());
        chat.set_model("gpt-6.1-sol");
        if auto_enabled {
            chat.enable_jev_auto();
        }
        let marker = ReasoningEffortConfig::Custom("jev".to_string());
        let update = jev_settings_notification(&chat, marker.clone());
        chat.on_thread_settings_updated(update);
        assert_eq!(
            (
                chat.jev_auto.enabled,
                chat.current_collaboration_mode().reasoning_effort(),
                chat.config.plan_mode_reasoning_effort.clone(),
            ),
            (auto_enabled, Some(marker.clone()), Some(marker.clone())),
        );
        chat.set_collaboration_mask(
            collaboration_modes::plan_mask(chat.model_catalog.as_ref())
                .expect("plan collaboration mode"),
        );
        assert_eq!(chat.effective_reasoning_effort(), Some(marker));
    }
}

#[tokio::test]
async fn jev_effort_delayed_server_snapshots_preserve_later_auto_selection() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_model("gpt-6.1-sol");
    chat.enable_jev_effort();
    let previous =
        jev_settings_notification(&chat, ReasoningEffortConfig::Custom("jev".to_string()));
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Medium));
    chat.enable_jev_auto();
    let current = jev_settings_notification(&chat, ReasoningEffortConfig::Medium);

    chat.on_thread_settings_updated(previous);
    assert!(chat.jev_auto.enabled);
    chat.on_thread_settings_updated(current);
    assert!(chat.jev_auto.enabled);
    assert!(!chat.jev_effort_configured());
    chat.set_collaboration_mask(
        collaboration_modes::plan_mask(chat.model_catalog.as_ref())
            .expect("plan collaboration mode"),
    );
    assert_eq!(
        chat.effective_reasoning_effort(),
        Some(ReasoningEffortConfig::Medium),
    );
}

fn jev_settings_notification(
    chat: &ChatWidget,
    effort: ReasoningEffortConfig,
) -> codex_app_server_protocol::ThreadSettingsUpdatedNotification {
    let mut collaboration_mode = chat.effective_collaboration_mode();
    collaboration_mode.settings.reasoning_effort = Some(effort.clone());
    codex_app_server_protocol::ThreadSettingsUpdatedNotification {
        thread_id: chat.thread_id.expect("thread").to_string(),
        thread_settings: codex_app_server_protocol::ThreadSettings {
            disabled_plugin_ids: Vec::new(),
            cwd: chat.config.cwd.clone(),
            approval_policy: codex_app_server_protocol::AskForApproval::Never,
            approvals_reviewer: codex_app_server_protocol::ApprovalsReviewer::User,
            sandbox_policy: codex_app_server_protocol::SandboxPolicy::ReadOnly {
                network_access: false,
            },
            active_permission_profile: None,
            model: chat.current_model().to_string(),
            model_provider: chat.config.model_provider_id.clone(),
            service_tier: None,
            effort: Some(effort),
            summary: None,
            collaboration_mode,
            multi_agent_mode: Default::default(),
            personality: None,
        },
    }
}
