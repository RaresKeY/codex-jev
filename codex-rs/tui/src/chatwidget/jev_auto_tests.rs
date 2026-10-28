use super::*;
use crate::bottom_pane::TerminalTitleItem;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use pretty_assertions::assert_eq;

fn model_status(chat: &mut ChatWidget) -> [Option<String>; 3] {
    let status = [
        StatusLineItem::ModelName,
        StatusLineItem::ModelWithReasoning,
        StatusLineItem::Reasoning,
    ]
    .map(|item| chat.status_line_value_for_item(item));
    let title = [
        TerminalTitleItem::Model,
        TerminalTitleItem::ModelWithReasoning,
        TerminalTitleItem::Reasoning,
    ]
    .map(|item| chat.terminal_title_value_for_item(item, Instant::now()));
    assert_eq!(status, title);
    status
}

fn hold_route(chat: &mut ChatWidget) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    chat.jev_auto.enabled = true;
    chat.jev_auto.pending = Some(PendingJevRoute {
        id,
        message: "first prompt".into(),
        history: UserMessageHistoryRecord::UserMessageText,
        shell: ShellEscapePolicy::Allow,
        source: UserMessageSource::Prompt,
        images: None,
        worker: tokio::spawn(std::future::pending()),
    });
    chat.update_task_running_state();
    id
}

#[tokio::test]
async fn jev_stats_follow_current_route_and_respect_the_footer_toggle() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.local_settings.tui.animations = false;
    chat.local_settings.tui.status_line = Some(vec!["jev-stats".to_string()]);
    chat.enable_jev_auto();
    assert_eq!(
        chat.status_line_value_for_item(StatusLineItem::JevStats),
        Some("Jev ready".into())
    );
    let id = hold_route(&mut chat);
    assert!(
        crate::chatwidget::tests::helpers::render_bottom_popup(&chat, 80).contains("Jev routing…")
    );
    chat.finish_jev_route(
        id,
        Ok(crate::jev_route::JevDecision {
            model: "gpt-6.1-sol".into(),
            effort: ReasoningEffortConfig::Low,
            elapsed: Duration::from_millis(800),
            stats: Some(crate::jev_route::JevRouteStats {
                model_confidence: 0.82,
                effort_confidence: 0.91,
                input_tokens: 480,
                output_tokens: 12,
            }),
        }),
    );
    let expected = "Jev 0.8s M82% E91% 480/12 tok";
    assert_eq!(
        chat.status_line_value_for_item(StatusLineItem::JevStats),
        Some(expected.into())
    );
    assert_eq!(
        chat.status_surface_preview_data()
            .value_for(StatusLineItem::JevStats.preview_item()),
        Some(expected)
    );
    for width in [48, 80] {
        assert!(
            crate::chatwidget::tests::helpers::render_bottom_popup(&chat, width).contains(expected)
        );
    }
    chat.local_settings.tui.status_line = Some(Vec::new());
    chat.refresh_status_line();
    assert!(!crate::chatwidget::tests::helpers::render_bottom_popup(&chat, 80).contains(expected));
    chat.disable_jev_auto();
    assert_eq!(chat.jev_auto.stats_display(), "Jev off");
}

#[tokio::test]
async fn jev_stats_ignore_stale_failures_and_clear_on_cancellation() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let id = hold_route(&mut chat);
    chat.finish_jev_route(uuid::Uuid::new_v4(), Err("stale failure".into()));
    assert_eq!(chat.jev_auto.stats_display(), "Jev routing…");
    chat.finish_jev_route(id, Err("routing failed".into()));
    assert_eq!(chat.jev_auto.stats_display(), "Jev failed");
    hold_route(&mut chat);
    chat.cancel_jev_route();
    assert_eq!(chat.jev_auto.stats_display(), "Jev ready");
    chat.finish_jev_route(id, Err("late failure".into()));
    assert_eq!(chat.jev_auto.stats_display(), "Jev ready");
}

#[tokio::test]
async fn custom_provider_and_reserve_sessions_keep_their_selected_model() {
    for (provider, model) in [
        ("custom-provider", "custom-model"),
        ("openai", crate::model_catalog::LUNA_RESERVE_MODEL),
    ] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        chat.enable_jev_auto();
        chat.handle_thread_session(crate::session_state::ThreadSessionState {
            windows_sandbox_host: crate::app::WindowsSandboxHost::Local,
            thread_id: ThreadId::new(),
            forked_from_id: None,
            fork_parent_title: None,
            thread_name: None,
            model: model.to_string(),
            model_provider_id: provider.to_string(),
            service_tier: None,
            approval_policy: codex_app_server_protocol::AskForApproval::Never,
            approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer::User,
            permission_profile: PermissionProfile::read_only(),
            active_permission_profile: None,
            cwd: chat.config.cwd.clone(),
            runtime_workspace_roots: Vec::new(),
            instruction_source_paths: Vec::new(),
            reasoning_effort: None,
            collaboration_mode: None,
            daybreak_enabled: false,
            message_history: None,
            network_proxy: None,
            rollout_path: None,
        });
        assert_eq!(
            (chat.jev_auto.enabled, chat.current_model()),
            (false, model)
        );
        chat.enable_jev_auto();
        assert_eq!(
            (chat.jev_auto.enabled, chat.current_model()),
            (provider != "openai", model),
        );
    }
}

#[tokio::test]
async fn acknowledged_usage_fallback_cancels_pending_jev_without_losing_the_prompt() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let id = hold_route(&mut chat);
    let mut mode = chat.effective_collaboration_mode();
    mode.settings.model = crate::model_catalog::LUNA_RESERVE_MODEL.to_string();
    chat.finish_backend_banner_fallback(mode);
    chat.finish_jev_route(
        id,
        Ok(crate::jev_route::JevDecision {
            model: "gpt-6.1-sol".to_string(),
            effort: ReasoningEffortConfig::Low,
            stats: None,
            elapsed: Duration::ZERO,
        }),
    );
    assert_eq!(
        (
            chat.jev_auto.enabled,
            chat.current_model(),
            chat.bottom_pane.composer_draft_snapshot().text,
        ),
        (
            false,
            crate::model_catalog::LUNA_RESERVE_MODEL,
            "first prompt".to_string()
        ),
    );
}

#[tokio::test]
async fn auto_status_waits_for_a_decision_and_resets_after_manual_selection() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.local_settings.tui.animations = false;
    chat.local_settings.tui.status_line = Some(vec!["model-with-reasoning".to_string()]);
    chat.set_model("manual-model");
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));
    chat.enable_jev_auto();
    assert_eq!(
        model_status(&mut chat),
        [Some("Auto (Jev)".into()), Some("Auto (Jev)".into()), None]
    );
    let mut snapshots = Vec::new();
    for width in [48, 80] {
        snapshots.push(format!(
            "Before routing, width {width}:\n{}",
            crate::chatwidget::tests::helpers::render_bottom_popup(&chat, width)
        ));
    }

    let id = hold_route(&mut chat);
    assert_eq!(
        model_status(&mut chat),
        [Some("Auto (Jev)".into()), Some("Auto (Jev)".into()), None]
    );
    chat.finish_jev_route(
        id,
        Ok(crate::jev_route::JevDecision {
            model: "gpt-6.1-sol".to_string(),
            effort: ReasoningEffortConfig::Low,
            stats: None,
            elapsed: Duration::ZERO,
        }),
    );
    assert_eq!(
        model_status(&mut chat),
        [
            Some("GPT-6.1-Sol".into()),
            Some("Auto (Jev) · GPT-6.1-Sol low".into()),
            Some("low".into()),
        ]
    );
    assert!(
        std::iter::from_fn(|| ops.try_recv().ok())
            .any(|op| matches!(op, crate::app_command::AppCommand::UserTurn { .. }))
    );
    for width in [48, 80] {
        snapshots.push(format!(
            "After routing, width {width}:\n{}",
            crate::chatwidget::tests::helpers::render_bottom_popup(&chat, width)
        ));
    }
    insta::assert_snapshot!("jev_auto_model_status", snapshots.join("\n\n"));

    // Selecting the already-active Auto row preserves a real decision.
    chat.enable_jev_auto();
    assert_eq!(
        model_status(&mut chat)[1],
        Some("Auto (Jev) · GPT-6.1-Sol low".into())
    );
    chat.disable_jev_auto();
    chat.set_model("manual-model");
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));
    assert_eq!(
        model_status(&mut chat),
        [
            Some("manual-model".into()),
            Some("manual-model high".into()),
            Some("high".into()),
        ]
    );
    chat.enable_jev_auto();
    assert_eq!(
        model_status(&mut chat),
        [Some("Auto (Jev)".into()), Some("Auto (Jev)".into()), None]
    );
}

#[tokio::test]
async fn failed_first_route_does_not_expose_the_inherited_model() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    let id = hold_route(&mut chat);
    chat.finish_jev_route(id, Err("Model change was not acknowledged.".into()));
    assert_eq!(
        model_status(&mut chat),
        [Some("Auto (Jev)".into()), Some("Auto (Jev)".into()), None]
    );
    assert_eq!(
        chat.bottom_pane.composer_draft_snapshot().text,
        "first prompt"
    );
}

#[tokio::test]
async fn classification_counts_as_pending_input_during_mcp_startup() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.mcp_startup_status = Some(HashMap::new());
    hold_route(&mut chat);

    assert!(chat.is_user_turn_pending_or_running());
    chat.queue_user_message("second prompt".into());
    chat.queue_user_message("third prompt".into());
    assert_eq!(
        chat.input_queue
            .queued_user_messages
            .iter()
            .map(|queued| queued.user_message.text.as_str())
            .collect::<Vec<_>>(),
        vec!["second prompt", "third prompt"]
    );
    assert!(!chat.maybe_send_next_queued_input());
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_preserves_drafts_and_ignores_a_late_decision() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    let id = hold_route(&mut chat);
    chat.restore_user_message_to_composer("still typing".into());
    chat.queue_user_message("queued follow-up".into());

    assert!(chat.cancel_jev_route());
    let restored = chat.bottom_pane.composer_draft_snapshot().text;
    assert!(restored.contains("first prompt"));
    assert!(restored.contains("still typing"));
    assert!(!chat.input_queue.suppress_queue_autosend);
    assert!(chat.input_queue.recovered_queue);
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);

    chat.finish_jev_route(
        id,
        Ok(crate::jev_route::JevDecision {
            model: "gpt-6.1-sol".to_string(),
            effort: codex_protocol::openai_models::ReasoningEffort::Low,
            stats: None,
            elapsed: Duration::ZERO,
        }),
    );
    assert_eq!(chat.bottom_pane.composer_draft_snapshot().text, restored);
    assert_eq!(
        model_status(&mut chat),
        [Some("Auto (Jev)".into()), Some("Auto (Jev)".into()), None]
    );
    while let Ok(op) = ops.try_recv() {
        assert!(!matches!(
            op,
            crate::app_command::AppCommand::UserTurn { .. }
        ));
    }
    assert!(!chat.maybe_send_next_queued_input());

    // This current-thread test never yields while the new worker is pending.
    // Cancel it before polling so the retry path cannot invoke the classifier.
    chat.handle_composer_input_result(
        InputResult::Submitted {
            text: restored,
            text_elements: Vec::new(),
        },
        /*had_modal_or_popup*/ false,
    );
    assert!(chat.jev_auto.pending.is_some());
    assert!(chat.cancel_jev_route());
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
}

#[tokio::test]
async fn missing_current_model_does_not_default_to_jev() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_model("model-missing-from-catalog");
    chat.open_all_models_popup();
    insta::assert_snapshot!(
        "jev_picker_missing_current_model",
        crate::chatwidget::tests::helpers::render_bottom_popup(&chat, /*width*/ 80),
    );
    assert_eq!(
        chat.bottom_pane.selected_index_for_present_view(
            super::super::model_popups::ALL_MODELS_SELECTION_VIEW_ID,
        ),
        Some(1)
    );
    while events.try_recv().is_ok() {}
    chat.handle_key_event(crossterm::event::KeyCode::Enter.into());
    assert!(matches!(
        events.try_recv(),
        Ok(AppEvent::OpenReasoningPopup { .. })
    ));
}

#[tokio::test]
async fn submission_failure_after_routing_preserves_the_draft_and_queue() {
    let (mut chat, _sender, _events, ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    let id = hold_route(&mut chat);
    chat.queue_user_message("queued follow-up".into());
    drop(ops);

    chat.finish_jev_route(
        id,
        Ok(crate::jev_route::JevDecision {
            model: "gpt-6.1-sol".to_string(),
            effort: codex_protocol::openai_models::ReasoningEffort::Low,
            stats: None,
            elapsed: Duration::ZERO,
        }),
    );
    assert_eq!(
        chat.bottom_pane.composer_draft_snapshot().text,
        "first prompt"
    );
    assert!(chat.input_queue.recovered_queue);
    assert!(!chat.input_queue.suppress_queue_autosend);
    assert!(!chat.maybe_send_next_queued_input());
}

#[tokio::test]
async fn followup_context_retains_visible_evidence_and_clears_after_revert() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.on_user_message_display(user_message_display_for_history(
        "Investigate the rendering defect".into(),
        &UserMessageHistoryRecord::UserMessageText,
    ));
    chat.on_task_started();
    chat.on_task_complete(
        Some("The first hypothesis failed; the second needs a comparison.".to_string()),
        /*completion*/ None,
        /*from_replay*/ false,
    );
    let context = serde_json::to_value(&chat.jev_auto.context).unwrap();
    assert_eq!(
        context["recent_requests"],
        serde_json::json!([
            {"text": "Investigate the rendering defect", "truncated": false}
        ])
    );
    assert_eq!(
        context["recent_response"],
        serde_json::json!({
            "text": "The first hypothesis failed; the second needs a comparison.", "truncated": false
        })
    );

    hold_route(&mut chat);
    chat.restore_user_message_to_composer("edited historical prompt".into());
    chat.reset_after_prompt_revert(/*rollout_path*/ None, &[]);
    assert!(chat.jev_auto.pending.is_none());
    assert_eq!(
        chat.bottom_pane.composer_draft_snapshot().text,
        "edited historical prompt"
    );
    assert_eq!(
        serde_json::to_value(&chat.jev_auto.context).unwrap(),
        serde_json::to_value(crate::jev_route::JevContext::default()).unwrap()
    );
}

#[tokio::test]
async fn enabling_jev_discloses_the_context_sent_to_the_provider() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    while events.try_recv().is_ok() {}
    chat.enable_jev_auto();
    let mut lines = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            lines.extend(cell.display_lines(/*width*/ 80).iter().map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            }));
        }
    }
    insta::assert_snapshot!("jev_context_disclosure", lines.join("\n"));
}

#[tokio::test]
async fn new_visible_request_cancels_classification_against_old_context() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    hold_route(&mut chat);
    chat.on_user_message_display(user_message_display_for_history(
        "Request from another client".into(),
        &UserMessageHistoryRecord::UserMessageText,
    ));
    assert!(chat.jev_auto.pending.is_none());
    assert_eq!(
        chat.bottom_pane.composer_draft_snapshot().text,
        "first prompt"
    );
    assert_eq!(
        serde_json::to_value(&chat.jev_auto.context).unwrap()["recent_requests"],
        serde_json::json!([{"text": "Request from another client", "truncated": false}]),
    );
}
