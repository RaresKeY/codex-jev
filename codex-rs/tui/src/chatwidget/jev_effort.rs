use super::*;

impl ChatWidget {
    pub(crate) fn jev_effort_enabled(&self) -> bool {
        matches!(
            self.effective_reasoning_effort(),
            Some(ReasoningEffortConfig::Custom(value)) if value == "jev"
        )
    }

    pub(crate) fn jev_effort_configured(&self) -> bool {
        self.jev_effort_enabled()
            || is_jev_effort(self.current_collaboration_mode.reasoning_effort().as_ref())
            || is_jev_effort(self.config.plan_mode_reasoning_effort.as_ref())
    }

    pub(super) fn clear_jev_effort(&mut self, replacement: Option<ReasoningEffortConfig>) {
        if is_jev_effort(replacement.as_ref()) {
            return;
        }
        if is_jev_effort(self.current_collaboration_mode.reasoning_effort().as_ref()) {
            self.current_collaboration_mode = self.current_collaboration_mode.with_updates(
                /*model*/ None,
                Some(replacement.clone()),
                /*developer_instructions*/ None,
            );
        }
        if is_jev_effort(self.config.plan_mode_reasoning_effort.as_ref()) {
            self.config.plan_mode_reasoning_effort = replacement.clone();
        }
        if let Some(mask) = self.active_collaboration_mask.as_mut()
            && is_jev_effort(mask.reasoning_effort.as_ref().and_then(Option::as_ref))
        {
            mask.reasoning_effort = Some(replacement);
        }
    }

    pub(crate) fn jev_effort_available(&self) -> bool {
        self.config.model_provider_id == codex_model_provider_info::OPENAI_PROVIDER_ID
            && !self.restrict_model_picker_to_luna_reserve()
            && !self.waiting_for_luna_reserve()
            && matches!(
                self.current_model(),
                "gpt-6.1-sol" | "gpt-6.1-terra" | "gpt-6-astra" | "gpt-6-luna"
            )
            && self.model_catalog.try_list_models().is_ok_and(|models| {
                models
                    .iter()
                    .any(|preset| preset.model == self.current_model())
            })
    }

    pub(super) fn apply_jev_effort(&mut self) {
        self.disable_jev_auto();
        self.sync_jev_effort(Some(ReasoningEffortConfig::Custom("jev".to_string())));
    }

    pub(super) fn sync_jev_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        // Server snapshots can lag the user's local Auto choice.
        if is_jev_effort(effort.as_ref()) {
            self.set_reasoning_effort(effort.clone());
            self.set_plan_mode_reasoning_effort(effort);
        } else {
            self.clear_jev_effort(effort);
        }
    }

    pub(crate) fn enable_jev_effort(&mut self) {
        self.apply_jev_effort();
        self.add_info_message(
            format!(
                "Effort only (Jev): keeping {}. Jev selects effort before each model step using bounded conversation and tool evidence. Routing stops if Jev fails or its weekly budget is exhausted.",
                self.current_model()
            ),
            /*hint*/ None,
        );
    }

    pub(super) fn jev_effort_picker_item(&self) -> Option<SelectionItem> {
        if !self.jev_effort_available() {
            return None;
        }
        let thread_id = self.thread_id;
        let model = self.current_model().to_string();
        Some(SelectionItem {
            name: "Effort only (Jev)".to_string(),
            description: Some(format!(
                "Keep {model}; choose effort before every model step"
            )),
            is_current: !self.jev_auto.enabled && self.jev_effort_enabled(),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::EnableJevEffort {
                    thread_id,
                    model: model.clone(),
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        })
    }
}

fn is_jev_effort(effort: Option<&ReasoningEffortConfig>) -> bool {
    matches!(effort, Some(ReasoningEffortConfig::Custom(value)) if value == "jev")
}

#[cfg(test)]
#[path = "jev_effort_tests.rs"]
mod tests;
