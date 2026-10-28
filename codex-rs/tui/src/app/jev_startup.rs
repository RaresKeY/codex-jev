//! Explicit launch model settings take precedence over the fork's Auto default.

use super::*;
use crate::app_server_session::ResumeModelSettings;

pub(super) fn apply_launch_overrides(chat_widget: &mut ChatWidget, overrides: &ConfigOverrides) {
    if config_persistence::resume_model_settings_for_overrides(chat_widget.config_ref(), overrides)
        == ResumeModelSettings::OverrideFromCurrentConfig
    {
        chat_widget.disable_jev_auto();
    }
}

#[cfg(test)]
#[path = "jev_startup_tests.rs"]
mod tests;
