//! Jev routing holds the original submission until classification and settings acknowledgement.
use super::*;

#[derive(Default)]
pub(super) struct JevAutoState {
    pub(super) enabled: bool,
    pub(super) applying: bool,
    pub(super) pending: Option<PendingJevRoute>,
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
        self.jev_auto.enabled = true;
        self.refresh_model_dependent_surfaces();
        self.add_info_message(
            "Auto (Jev) enabled for this chat. Each new text turn is sent to Jev for model and effort selection.".to_string(),
            /*hint*/ None,
        );
    }

    pub(crate) fn disable_jev_auto(&mut self) {
        self.cancel_jev_route();
        self.jev_auto.enabled = false;
        self.refresh_model_dependent_surfaces();
    }

    pub(crate) fn cancel_jev_route(&mut self) -> bool {
        let Some(pending) = self.jev_auto.pending.take() else {
            return false;
        };
        self.input_queue.suppress_queue_autosend = true;
        self.restore_user_message_to_composer(user_message_for_restore(
            pending.message.clone(),
            &pending.history,
        ));
        self.update_task_running_state();
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
        let thread_id = self.thread_id;
        let events = self.app_event_tx.clone();
        let worker = tokio::spawn(async move {
            let result = crate::jev_route::choose(task).await;
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
                self.add_error_message(error);
            }
            Ok(decision) => {
                let pending = self.jev_auto.pending.take().expect("current Jev route");
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
                self.submit_user_message_with_prepared_images(
                    pending.message.clone(),
                    pending.history.clone(),
                    pending.shell,
                    pending.source,
                    pending.images.clone(),
                );
                self.jev_auto.applying = false;
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
