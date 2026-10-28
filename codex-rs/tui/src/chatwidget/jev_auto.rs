//! Jev routing holds the original submission until classification and settings acknowledgement.
use super::*;

pub(super) struct JevAutoState {
    pub(super) enabled: bool,
    pub(super) has_selected_model: bool,
    pub(super) applying: bool,
    pub(super) pending: Option<PendingJevRoute>,
    pub(super) context: crate::jev_route::JevContext,
    pub(super) status: super::jev_status::JevRouteStatus,
}

impl Default for JevAutoState {
    fn default() -> Self {
        Self {
            enabled: true,
            has_selected_model: false,
            applying: false,
            pending: None,
            context: Default::default(),
            status: Default::default(),
        }
    }
}

impl JevAutoState {
    pub(super) fn awaiting_model_selection(&self) -> bool {
        self.enabled && !self.has_selected_model
    }
}

pub(super) struct PendingJevRoute {
    id: uuid::Uuid,
    message: UserMessage,
    history: UserMessageHistoryRecord,
    shell: ShellEscapePolicy,
    source: UserMessageSource,
    images: Option<Vec<UserInput>>,
    worker: tokio::task::JoinHandle<()>,
}

impl Drop for PendingJevRoute {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

impl ChatWidget {
    pub(crate) fn enable_jev_auto(&mut self) {
        // A picker action queued before a usage fallback cannot bypass Reserve restrictions.
        if self.restrict_model_picker_to_luna_reserve() || self.waiting_for_luna_reserve() {
            return;
        }
        self.jev_auto.enabled = true;
        self.refresh_model_dependent_surfaces();
        self.add_info_message(
            "Auto (Jev) enabled for this chat. Each new text turn and bounded recent conversation, including tool calls and results, are sent to Jev for model and effort selection.".to_string(),
            /*hint*/ None,
        );
    }

    pub(crate) fn disable_jev_auto(&mut self) {
        self.cancel_jev_route();
        self.jev_auto.enabled = false;
        self.jev_auto.has_selected_model = false;
        self.jev_auto.status = Default::default();
        self.refresh_model_dependent_surfaces();
    }

    pub(crate) fn cancel_jev_route(&mut self) -> bool {
        let Some(pending) = self.jev_auto.pending.take() else {
            return false;
        };
        self.jev_auto.status = Default::default();
        self.input_queue.recovered_queue |= self.input_queue.has_queued_follow_up_messages();
        self.restore_user_message_to_composer(user_message_for_restore(
            pending.message.clone(),
            &pending.history,
        ));
        self.update_task_running_state();
        self.refresh_pending_input_preview();
        self.request_redraw();
        true
    }

    pub(crate) fn jev_route_is_current(&self, id: uuid::Uuid) -> bool {
        self.jev_auto
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == id)
    }

    pub(super) fn begin_jev_route(
        &mut self,
        message: UserMessage,
        history: UserMessageHistoryRecord,
        shell: ShellEscapePolicy,
        source: UserMessageSource,
        images: Option<Vec<UserInput>>,
    ) {
        let id = uuid::Uuid::new_v4();
        let task = message.text.clone();
        let mut context = self.jev_auto.context.clone();
        context.active_goal = self
            .current_goal_status
            .as_ref()
            .and_then(GoalStatusState::active_objective)
            .map(crate::jev_route::JevExcerpt::new);
        context.images_attached = !message.local_images.is_empty()
            || !message.remote_image_urls.is_empty()
            || images.as_ref().is_some_and(|images| !images.is_empty());
        let thread_id = self.thread_id;
        let events = self.app_event_tx.clone();
        let worker = tokio::spawn(async move {
            let (reply, receiver) = tokio::sync::oneshot::channel();
            events.send(AppEvent::JevContextRequested {
                id,
                thread_id,
                reply: crate::jev_context::JevContextReply(reply),
            });
            let Ok(Ok(handle)) =
                tokio::time::timeout(Duration::from_secs(/*secs*/ 3), receiver).await
            else {
                events.send(AppEvent::JevRouteReady {
                    id,
                    thread_id,
                    result: Err(
                        "Cannot read Jev conversation context. Your message was not sent."
                            .to_string(),
                    ),
                });
                return;
            };
            if let Some(thread_id) = thread_id {
                context.transcript =
                    Some(crate::jev_context::snapshot(&handle, thread_id, id).await);
            }
            let result = crate::jev_route::choose(task, context).await;
            events.send(AppEvent::JevRouteReady {
                id,
                thread_id,
                result,
            });
        });
        self.jev_auto.pending = Some(PendingJevRoute {
            id,
            message,
            history,
            shell,
            source,
            images,
            worker,
        });
        self.jev_auto.status = Default::default();
        self.update_task_running_state();
        self.set_status_header("Routing with Jev".to_string());
        self.request_redraw();
    }

    pub(crate) fn finish_jev_route(
        &mut self,
        id: uuid::Uuid,
        result: Result<crate::jev_route::JevDecision, String>,
    ) {
        if !self.jev_route_is_current(id) {
            return;
        }
        match result {
            Err(error) => {
                self.cancel_jev_route();
                self.jev_auto.status = super::jev_status::JevRouteStatus::Failed;
                self.refresh_status_line();
                self.add_error_message(error);
            }
            Ok(decision) => {
                let Some(pending) = self.jev_auto.pending.take() else {
                    return;
                };
                // Only an acknowledged routing decision owns the Auto model label.
                self.jev_auto.has_selected_model = true;
                self.jev_auto.status = super::jev_status::JevRouteStatus::Complete {
                    elapsed: decision.elapsed,
                    stats: decision.stats,
                };
                self.set_model(&decision.model);
                self.set_reasoning_effort(Some(decision.effort.clone()));
                // Plan mode has a separate reasoning setting; the routed effort owns this turn.
                if self
                    .active_collaboration_mask
                    .as_ref()
                    .is_some_and(|mask| mask.mode == Some(ModeKind::Plan))
                {
                    self.set_plan_mode_reasoning_effort(Some(decision.effort.clone()));
                }
                self.add_info_message(
                    format!("Auto (Jev): {} · {}", decision.model, decision.effort),
                    /*hint*/ None,
                );
                self.jev_auto.applying = true;
                self.update_task_running_state();
                let (accepted, _) = self.submit_user_message_with_prepared_images(
                    pending.message.clone(),
                    pending.history.clone(),
                    pending.shell,
                    pending.source,
                    pending.images.clone(),
                );
                self.jev_auto.applying = false;
                if !accepted {
                    self.input_queue.recovered_queue |=
                        self.input_queue.has_queued_follow_up_messages();
                }
            }
        }
    }

    pub(super) fn jev_picker_item(&self) -> SelectionItem {
        SelectionItem {
            name: "Auto (Jev)".to_string(),
            description: Some(
                "Send each new text turn to Jev to choose model and effort".to_string(),
            ),
            is_current: self.jev_auto.enabled,
            actions: vec![Box::new(|tx| tx.send(AppEvent::EnableJevAuto))],
            dismiss_on_select: true,
            ..Default::default()
        }
    }
}

#[cfg(test)]
#[path = "jev_auto_tests.rs"]
mod tests;
