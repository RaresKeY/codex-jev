use super::*;
use crate::app::test_support::make_test_app;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn jev_launch_overrides_preserve_explicit_model_provider_and_effort() {
    let mut app = make_test_app().await;
    app.local_settings.tui.status_line = Some(vec!["model-with-reasoning".to_string()]);
    let mut tui = crate::tui::test_support::make_test_tui().expect("test TUI");
    for (key, value) in [
        ("model", "gpt-6.1-sol"),
        ("model_provider", "openai"),
        ("model_reasoning_effort", "high"),
    ] {
        let mut launch_config = app.config.clone();
        launch_config.config_layer_stack = ConfigLayerStack::new(
            vec![ConfigLayerEntry::new(
                ConfigLayerSource::SessionFlags,
                TomlValue::Table(toml::map::Map::from_iter([(
                    key.to_string(),
                    TomlValue::String(value.to_string()),
                )])),
            )],
            Default::default(),
            Default::default(),
        )
        .expect("session flags layer");
        let init = app.chatwidget_init_for_forked_or_resumed_thread(
            &mut tui,
            launch_config,
            /*initial_user_message*/ None,
        );
        let mut widget = ChatWidget::new_with_app_event(init);
        apply_launch_overrides(&mut widget, &app.harness_overrides);
        assert!(!render_bottom_popup(&widget, /*width*/ 80).contains("Auto (Jev)"));
        assert_eq!(widget.current_model(), app.chat_widget.current_model());
    }

    app.harness_overrides.model = Some("gpt-6.1-sol".to_string());
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Auto (Jev)"));
}

#[tokio::test]
async fn jev_launch_overrides_leave_inherited_defaults_in_auto() {
    let mut app = make_test_app().await;
    app.local_settings.tui.status_line = Some(vec!["model-with-reasoning".to_string()]);
    let config_path = std::env::temp_dir().join("codex-jev-inherited-config.toml");
    app.config.config_layer_stack = ConfigLayerStack::default()
        .with_user_config(
            &codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(config_path)
                .expect("absolute config path"),
            TomlValue::Table(toml::map::Map::from_iter([
                (
                    "model".to_string(),
                    TomlValue::String("gpt-6.1-sol".to_string()),
                ),
                (
                    "model_reasoning_effort".to_string(),
                    TomlValue::String("high".to_string()),
                ),
            ])),
        )
        .expect("inherited user config layer");
    let mut tui = crate::tui::test_support::make_test_tui().expect("test TUI");
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    let mut widget = ChatWidget::new_with_app_event(init);
    apply_launch_overrides(&mut widget, &app.harness_overrides);
    assert!(render_bottom_popup(&widget, /*width*/ 80).contains("Auto (Jev)"));
}
