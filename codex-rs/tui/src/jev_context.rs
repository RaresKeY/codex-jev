//! A bounded parent transcript, following Guardian's role/tool evidence selection.
//! Read through the active server so resume, rollback and remote sessions agree.
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use serde::Serialize;
use std::time::Duration;

const MAX_ENTRIES: usize = 24;
const MAX_ENTRY_BYTES: usize = 1_536;
const MAX_TRANSCRIPT_BYTES: usize = 16_000;

pub(crate) struct JevContextReply(pub(crate) tokio::sync::oneshot::Sender<AppServerRequestHandle>);

impl std::fmt::Debug for JevContextReply {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JevContextReply(..)")
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct JevTranscript {
    source: &'static str,
    availability: &'static str,
    omitted: bool,
    entries: Vec<JevTranscriptEntry>,
}

#[derive(Clone, Debug, Serialize)]
struct JevTranscriptEntry {
    role: &'static str,
    text: String,
    truncated: bool,
}

impl JevTranscript {
    fn unavailable() -> Self {
        Self {
            source: "app_server_thread_items",
            availability: "unavailable",
            omitted: true,
            entries: Vec::new(),
        }
    }

    fn push(&mut self, role: &'static str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let truncated = text.len() > MAX_ENTRY_BYTES;
        let text = if truncated {
            // Keep both the command/brief and the final error/conclusion.
            let marker = "\n[…]\n";
            let half = (MAX_ENTRY_BYTES - marker.len()) / 2;
            let head = text.floor_char_boundary(half);
            let tail = text.ceil_char_boundary(text.len() - half);
            format!("{}{marker}{}", &text[..head], &text[tail..])
        } else {
            text.to_string()
        };
        self.entries.push(JevTranscriptEntry {
            role,
            text,
            truncated,
        });
        while self.entries.len() > MAX_ENTRIES
            || serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > MAX_TRANSCRIPT_BYTES)
        {
            self.entries.remove(0);
            self.omitted = true;
        }
    }

    fn from_page(page: ThreadItemsListResponse) -> Self {
        let mut transcript = Self {
            availability: "available",
            omitted: page.next_cursor.is_some(),
            ..Self::unavailable()
        };
        // The server returns newest first; Jev receives chronological evidence.
        for entry in page.data.iter().rev() {
            match &entry.item {
                ThreadItem::UserMessage { content, .. } => {
                    for input in content {
                        if let UserInput::Text { text, .. } = input {
                            transcript.push("user", text);
                        }
                    }
                }
                ThreadItem::AgentMessage { text, .. } => transcript.push("assistant", text),
                ThreadItem::Plan { text, .. } => transcript.push("plan", text),
                ThreadItem::CommandExecution {
                    command,
                    aggregated_output,
                    exit_code,
                    status,
                    ..
                } => {
                    transcript.push("tool_call", &format!("shell: {command}"));
                    transcript.push(
                        "tool_output",
                        &format!(
                            "shell status={status:?} exit_code={exit_code:?}\n{}",
                            aggregated_output
                                .as_deref()
                                .unwrap_or("<output unavailable>"),
                        ),
                    );
                }
                ThreadItem::FunctionCallOutput { name, output, .. } => {
                    if let Some(text) = output.to_text() {
                        transcript.push("tool_output", &format!("{name}: {text}"));
                    }
                }
                ThreadItem::McpToolCall {
                    server,
                    tool,
                    arguments,
                    result,
                    error,
                    status,
                    ..
                } => {
                    transcript.push("tool_call", &format!("{server}/{tool}: {arguments}"));
                    transcript.push("tool_output", &format!("{server}/{tool} status={status:?}"));
                    if let Some(result) = result {
                        for content in &result.content {
                            if content["type"] == "text"
                                && let Some(text) = content["text"].as_str()
                            {
                                transcript.push("tool_output", text);
                            }
                        }
                    }
                    if let Some(error) = error {
                        transcript.push("tool_output", &error.message);
                    }
                }
                ThreadItem::DynamicToolCall {
                    tool,
                    arguments,
                    content_items,
                    success,
                    status,
                    ..
                } => {
                    transcript.push("tool_call", &format!("{tool}: {arguments}"));
                    transcript.push(
                        "tool_output",
                        &format!("{tool} status={status:?} success={success:?}"),
                    );
                    for content in content_items.iter().flatten() {
                        if let DynamicToolCallOutputContentItem::InputText { text } = content {
                            transcript.push("tool_output", text);
                        }
                    }
                }
                ThreadItem::FileChange {
                    changes, status, ..
                } => {
                    for change in changes {
                        transcript.push("tool_call", &format!("file change: {}", change.path));
                    }
                    transcript.push("tool_output", &format!("file change status={status:?}"));
                }
                ThreadItem::CollabAgentToolCall {
                    tool,
                    prompt,
                    status,
                    ..
                } => {
                    transcript.push(
                        "tool_call",
                        &format!("{tool:?}: {}", prompt.as_deref().unwrap_or("")),
                    );
                    transcript.push("tool_output", &format!("agent status={status:?}"));
                }
                ThreadItem::WebSearch(search) => {
                    transcript.push("tool_call", &format!("web search: {}", search.query))
                }
                // Match Guardian's exclusion of reasoning and contextual scaffolding.
                // Image/audio payloads, hook prompts and review policy are not copied.
                ThreadItem::Reasoning { .. }
                | ThreadItem::HookPrompt { .. }
                | ThreadItem::SubAgentActivity { .. }
                | ThreadItem::ImageView { .. }
                | ThreadItem::Sleep(_)
                | ThreadItem::ImageGeneration(_)
                | ThreadItem::EnteredReviewMode { .. }
                | ThreadItem::ExitedReviewMode { .. }
                | ThreadItem::ContextCompaction { .. } => transcript.omitted = true,
            }
        }
        transcript
    }
}

pub(crate) async fn snapshot(
    handle: &AppServerRequestHandle,
    thread_id: ThreadId,
    route_id: uuid::Uuid,
) -> JevTranscript {
    let request = handle.request_typed::<ThreadItemsListResponse>(ClientRequest::ThreadItemsList {
        request_id: RequestId::String(format!("jev-context-{route_id}")),
        params: crate::app_server_session::thread_items_page_params(
            thread_id, /*turn_id*/ None, /*cursor*/ None, /*limit*/ 32,
        ),
    });
    match tokio::time::timeout(Duration::from_secs(/*secs*/ 3), request).await {
        Ok(Ok(page)) => JevTranscript::from_page(page),
        // New/ephemeral threads and old servers may not expose item history.
        // Preserve the visible evidence and explicitly label the missing snapshot.
        Ok(Err(_)) | Err(_) => JevTranscript::unavailable(),
    }
}

#[cfg(test)]
#[path = "jev_context_tests.rs"]
mod tests;
