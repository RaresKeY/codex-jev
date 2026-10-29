use super::*;
use crate::collaboration_modes;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn jev_effort_mode_changes_wait_for_backend_acknowledgement() -> Result<()> {
    for mode in [ModeKind::Default, ModeKind::Plan] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        app.config.model = Some("gpt-6.1-sol".to_string());
        app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
        let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let started = server.start_thread(&app.config).await?;
        let thread_id = started.session.thread_id;
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await?;
        app.chat_widget
            .set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
        if mode == ModeKind::Plan {
            let mask = collaboration_modes::plan_mask(app.model_catalog.as_ref()).unwrap();
            app.chat_widget.set_collaboration_mask(mask);
        }
        server.thread_settings_update_supported = false;
        app.enable_jev_effort(&mut server, Some(thread_id), "gpt-6.1-sol".to_string())
            .await;
        assert!(!app.chat_widget.jev_effort_configured());

        server.thread_settings_update_supported = true;
        app.enable_jev_effort(&mut server, Some(thread_id), "gpt-6.1-sol".to_string())
            .await;
        assert!(app.chat_widget.jev_effort_configured());
        let selected = next_thread_settings_updated(&mut server, thread_id).await;
        assert_eq!(
            selected
                .thread_settings
                .collaboration_mode
                .settings
                .reasoning_effort,
            Some(ReasoningEffortConfig::Custom("jev".to_string())),
        );
        server.thread_settings_update_supported = false;
        app.enable_jev_auto(&mut server).await;
        assert!(app.chat_widget.jev_effort_configured());

        server.thread_settings_update_supported = true;
        app.enable_jev_auto(&mut server).await;
        assert!(!app.chat_widget.jev_effort_configured());
        let selected = next_thread_settings_updated(&mut server, thread_id).await;
        assert_ne!(
            selected
                .thread_settings
                .collaboration_mode
                .settings
                .reasoning_effort,
            Some(ReasoningEffortConfig::Custom("jev".to_string())),
        );
        assert_eq!(
            (
                app.chat_widget.current_model(),
                app.config.model_reasoning_effort.clone()
            ),
            ("gpt-6.1-sol", Some(ReasoningEffortConfig::Medium)),
        );
        server.shutdown().await?;
    }
    Ok(())
}
