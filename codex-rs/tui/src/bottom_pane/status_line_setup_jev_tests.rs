use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

#[test]
fn jev_stats_can_be_toggled_on_and_off_without_changing_other_selections() {
    let (tx, mut rx) = unbounded_channel();
    let mut selected = vec!["model-with-reasoning".to_string()];
    for expected in [
        vec![StatusLineItem::ModelWithReasoning, StatusLineItem::JevStats],
        vec![StatusLineItem::ModelWithReasoning],
    ] {
        let mut view = StatusLineSetupView::new(
            Some(&selected),
            /*use_theme_colors*/ false,
            StatusSurfacePreviewData::default(),
            AppEventSender::new(tx.clone()),
            crate::keymap::RuntimeKeymap::defaults().list,
        );
        for character in "jev-stats".chars() {
            view.handle_key_event(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        view.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let AppEvent::StatusLineSetup {
            items,
            use_theme_colors,
        } = rx.try_recv().unwrap()
        else {
            panic!("expected status line settings");
        };
        assert_eq!(items, expected);
        assert!(!use_theme_colors);
        selected = items.iter().map(ToString::to_string).collect();
    }
}
