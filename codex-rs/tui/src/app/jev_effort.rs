use super::*;

impl App {
    pub(super) async fn enable_jev_auto(&mut self, app_server: &mut AppServerSession) {
        if !self.chat_widget.jev_effort_configured() {
            self.chat_widget.enable_jev_auto();
            return;
        }
        let effort = self
            .model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == self.chat_widget.current_model())
                    .map(|preset| preset.default_reasoning_effort)
            });
        let Some(effort) = effort else {
            self.chat_widget.add_error_message(
                "Select an available model before leaving Jev effort mode.".to_string(),
            );
            return;
        };
        let Some(mut params) =
            self.active_thread_reasoning_setting_update_params(Some(effort.clone()))
        else {
            self.chat_widget
                .add_error_message("Thread is unavailable. Jev mode was not changed.".to_string());
            return;
        };
        let mut mode = self.chat_widget.effective_collaboration_mode();
        mode.settings.reasoning_effort = Some(effort.clone());
        params.collaboration_mode = Some(mode);
        if self.send_thread_settings_update(app_server, params).await {
            self.chat_widget.set_reasoning_effort(Some(effort));
            self.chat_widget.enable_jev_auto();
        }
    }

    pub(super) async fn enable_jev_effort(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: Option<ThreadId>,
        model: String,
    ) {
        if self.active_thread_id != thread_id
            || self.chat_widget.thread_id() != thread_id
            || self.chat_widget.current_model() != model
            || !self.chat_widget.jev_effort_available()
        {
            return;
        }
        let effort = ReasoningEffortConfig::Custom("jev".to_string());
        let Some(mut params) =
            self.active_thread_reasoning_setting_update_params(Some(effort.clone()))
        else {
            self.chat_widget
                .add_error_message("Thread is unavailable. Jev mode was not changed.".to_string());
            return;
        };
        let mut mode = self.chat_widget.effective_collaboration_mode();
        mode.settings.reasoning_effort = Some(effort);
        params.collaboration_mode = Some(mode);
        if self.send_thread_settings_update(app_server, params).await {
            self.chat_widget.enable_jev_effort();
        }
    }
}
