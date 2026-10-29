//! Jev selects a concrete effort once per sampling step; retries reuse that selection.

use super::session::Session;
use super::step_context::StepContext;
use crate::context_manager::is_user_turn_boundary;
use codex_async_utils::OrCancelExt;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::SessionSource;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

const MAX_TASK_BYTES: usize = 2048;
const MAX_ENTRY_BYTES: usize = 1536;
const MAX_TRANSCRIPT_BYTES: usize = 16_000;
const MAX_ENTRIES: usize = 24;
const MAX_HISTORY_ITEMS: usize = 32;
const MAX_HELPER_OUTPUT_BYTES: usize = 4096;

pub(super) async fn resolve(
    session: &Session,
    step: &mut Arc<StepContext>,
    cancellation: &CancellationToken,
) -> CodexResult<()> {
    if !matches!(
        step.settings.selected_collaboration_mode().settings.reasoning_effort.as_ref(),
        Some(ReasoningEffort::Custom(value)) if value == "jev"
    ) {
        return Ok(());
    }
    let model = &step.settings.model_info;
    if step.turn.config.model_provider_id != "openai"
        || !matches!(
            model.slug.as_str(),
            "gpt-6-luna" | "gpt-6.1-terra" | "gpt-6.1-sol" | "gpt-6-astra"
        )
        || matches!(
            &step.turn.session_source,
            SessionSource::SubAgent(_) | SessionSource::Internal(_)
        )
        || crate::guardian::is_basic_session_source(&step.turn.session_source)
    {
        return Err(CodexErr::Fatal(
            "Jev effort mode requires a supported OpenAI model in a root thread.".to_string(),
        ));
    }
    let allowed_efforts: Vec<_> = model
        .supported_reasoning_levels
        .iter()
        .map(|preset| &preset.effort)
        .filter(|effort| {
            matches!(
                effort,
                ReasoningEffort::Low
                    | ReasoningEffort::Medium
                    | ReasoningEffort::High
                    | ReasoningEffort::XHigh
                    | ReasoningEffort::Max
            ) && (model.slug != "gpt-6-luna"
                || matches!(effort, ReasoningEffort::Low | ReasoningEffort::Medium))
        })
        .cloned()
        .collect();
    if allowed_efforts.is_empty() {
        return Err(CodexErr::Fatal(
            "The current model has no supported Jev effort levels.".to_string(),
        ));
    }
    let history = session
        .clone_history()
        .await
        .for_prompt(&model.input_modalities);
    let payload = request(&history, &model.slug, &allowed_efforts);
    let helper = std::env::current_exe()
        .map_err(|_| CodexErr::Fatal("Cannot locate the Codex Jev installation.".to_string()))?
        .with_file_name("jev-route.py");
    let effort = choose(&helper, &payload, &model.slug, &allowed_efforts)
        .or_cancel(cancellation)
        .await?
        .map_err(CodexErr::Fatal)?;
    Arc::make_mut(&mut Arc::make_mut(step).settings).jev_effort = Some(effort);
    Ok(())
}

fn bounded_parts<'a>(parts: impl Iterator<Item = &'a str>, limit: usize) -> (String, bool) {
    let mut text = String::new();
    let mut truncated = false;
    for part in parts {
        let (part, part_truncated) = bounded(part, limit);
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&part);
        let (next, next_truncated) = bounded(&text, limit);
        text = next;
        truncated |= part_truncated || next_truncated;
    }
    (text, truncated)
}

fn bounded(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_string(), false);
    }
    let marker = "\n[... omitted ...]\n";
    let head = text.floor_char_boundary((limit - marker.len()) / 2);
    let tail = text.ceil_char_boundary(text.len() - (limit - marker.len() - head));
    (format!("{}{marker}{}", &text[..head], &text[tail..]), true)
}

fn message_text(content: &[ContentItem], limit: usize) -> (String, bool) {
    bounded_parts(
        content.iter().filter_map(|part| match part {
            ContentItem::InputText { text } | ContentItem::OutputText { text, .. } => {
                Some(text.as_str())
            }
            ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => None,
        }),
        limit,
    )
}

#[derive(Serialize)]
struct Transcript {
    source: &'static str,
    availability: &'static str,
    omitted: bool,
    entries: Vec<Value>,
}

fn request(history: &[ResponseItem], model: &str, efforts: &[ReasoningEffort]) -> Value {
    let latest_user = history.iter().rev().find_map(|item| match item {
        ResponseItem::Message { role, content, .. }
            if role == "user" && is_user_turn_boundary(item) =>
        {
            let (text, _) = message_text(content, MAX_TASK_BYTES);
            let images = content
                .iter()
                .any(|part| matches!(part, ContentItem::InputImage { .. }));
            Some((text, images))
        }
        _ => None,
    });
    let (mut task, mut images_attached) = latest_user.unwrap_or_default();
    if task.trim().is_empty() && !images_attached {
        task = "Continue the current task using the latest tool results.".to_string();
    }
    let mut entries = Vec::new();
    let mut omitted = history.len() > MAX_HISTORY_ITEMS;
    for item in history.iter().rev().take(MAX_HISTORY_ITEMS) {
        let (role, text, truncated) = match item {
            ResponseItem::Message { role, content, .. }
                if role == "user" || role == "assistant" =>
            {
                images_attached |= content
                    .iter()
                    .any(|part| matches!(part, ContentItem::InputImage { .. }));
                let (text, truncated) = message_text(content, MAX_ENTRY_BYTES);
                (role.as_str(), text, truncated)
            }
            ResponseItem::AgentMessage { content, .. } => {
                omitted |= content
                    .iter()
                    .any(|part| matches!(part, AgentMessageInputContent::EncryptedContent { .. }));
                let (text, truncated) = bounded_parts(
                    content.iter().filter_map(|part| match part {
                        AgentMessageInputContent::InputText { text } => Some(text.as_str()),
                        AgentMessageInputContent::EncryptedContent { .. } => None,
                    }),
                    MAX_ENTRY_BYTES,
                );
                ("assistant", text, truncated)
            }
            ResponseItem::FunctionCall {
                name, arguments, ..
            }
            | ResponseItem::CustomToolCall {
                name,
                input: arguments,
                ..
            } => {
                let (text, truncated) = bounded_parts(
                    [name.as_str(), arguments.as_str()].into_iter(),
                    MAX_ENTRY_BYTES,
                );
                ("tool_call", text, truncated)
            }
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                let (text, truncated) = match &output.body {
                    FunctionCallOutputBody::Text(text) => bounded(text, MAX_ENTRY_BYTES),
                    FunctionCallOutputBody::ContentItems(items) => bounded_parts(
                        items.iter().filter_map(|item| match item {
                            FunctionCallOutputContentItem::InputText { text } => {
                                Some(text.as_str())
                            }
                            FunctionCallOutputContentItem::InputImage { .. }
                            | FunctionCallOutputContentItem::InputAudio { .. }
                            | FunctionCallOutputContentItem::EncryptedContent { .. } => None,
                        }),
                        MAX_ENTRY_BYTES,
                    ),
                };
                ("tool_output", text, truncated)
            }
            _ => {
                omitted = true;
                continue;
            }
        };
        if text.trim().is_empty() || entries.len() == MAX_ENTRIES {
            omitted = true;
            continue;
        }
        entries.push(json!({"role": role, "text": text, "truncated": truncated}));
    }
    entries.reverse();
    let mut transcript = Transcript {
        source: "core_step_history",
        availability: "available",
        omitted,
        entries,
    };
    while serde_json::to_vec(&transcript).is_ok_and(|value| value.len() > MAX_TRANSCRIPT_BYTES) {
        transcript.entries.remove(/*index*/ 0);
        transcript.omitted = true;
    }
    json!({
        "mode": "effort_only", "model": model, "allowed_efforts": efforts, "task": task,
        "context": {
            "source": "core_step_history", "partial": true, "recent_requests": [],
            "recent_response": null, "last_turn_status": "in_progress", "active_goal": null,
            "images_attached": images_attached, "transcript": transcript,
        },
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    model: String,
    effort: ReasoningEffort,
    #[serde(default, rename = "stats")]
    _stats: Option<serde::de::IgnoredAny>,
}

fn parse_decision(
    output: &[u8],
    model: &str,
    allowed_efforts: &[ReasoningEffort],
) -> Result<ReasoningEffort, String> {
    let decision: Decision =
        serde_json::from_slice(output).map_err(|_| "Invalid Jev effort decision.")?;
    if decision.model != model || !allowed_efforts.contains(&decision.effort) {
        return Err("Jev selected an unsupported model or effort.".to_string());
    }
    Ok(decision.effort)
}

async fn choose(
    helper: &Path,
    payload: &Value,
    model: &str,
    allowed_efforts: &[ReasoningEffort],
) -> Result<ReasoningEffort, String> {
    if !helper.is_file() {
        return Err("Jev helper missing. Install the matching codex-jev build.".to_string());
    }
    let operation = async {
        let mut child = tokio::process::Command::new("python3")
            .arg("-I")
            .arg(helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| "Cannot start the Jev helper; Python 3 is required.")?;
        let mut stdin = child.stdin.take().ok_or("Jev stdin unavailable.")?;
        let input = serde_json::to_vec(payload).map_err(|_| "Cannot encode Jev request.")?;
        stdin
            .write_all(&input)
            .await
            .map_err(|_| "Cannot send Jev effort request.")?;
        drop(stdin);
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .ok_or("Jev stdout unavailable.")?
            .take((MAX_HELPER_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut output)
            .await
            .map_err(|_| "Cannot read Jev effort decision.")?;
        if output.len() > MAX_HELPER_OUTPUT_BYTES {
            return Err("Jev helper output exceeded its limit.".to_string());
        }
        if !child
            .wait()
            .await
            .map_err(|_| "Jev helper failed.")?
            .success()
        {
            return Err(serde_json::from_slice::<Value>(&output)
                .ok()
                .and_then(|value| value["error"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "Jev effort routing failed.".to_string()));
        }
        parse_decision(&output, model, allowed_efforts)
    };
    tokio::time::timeout(Duration::from_secs(/*secs*/ 35), operation)
        .await
        .map_err(|_| "Jev effort routing timed out.".to_string())?
}

#[cfg(test)]
#[path = "jev_effort_tests.rs"]
mod tests;
