//! Bounded subprocess boundary for the bundled Jev classifier. No prompts in diagnostics.
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JevDecision {
    pub(crate) model: String,
    pub(crate) effort: ReasoningEffort,
    // Older bundled helpers can still route, without inventing missing metrics.
    #[serde(default)]
    pub(crate) stats: Option<JevRouteStats>,
    #[serde(skip)]
    pub(crate) elapsed: Duration,
}

/// Validated classifier metadata only; no task, policy, credentials or response text.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JevRouteStats {
    pub(crate) model_confidence: f64,
    pub(crate) effort_confidence: f64,
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
}

fn parse_decision(output: &[u8]) -> Result<JevDecision, String> {
    let decision: JevDecision =
        serde_json::from_slice(output).map_err(|_| "Invalid Jev decision.")?;
    if !matches!(
        decision.model.as_str(),
        "gpt-6.1-sol" | "gpt-6.1-terra" | "gpt-6-astra" | "gpt-6-luna"
    ) {
        return Err("Jev selected an unsupported model.".to_string());
    }
    if !matches!(
        decision.effort,
        ReasoningEffort::Low
            | ReasoningEffort::Medium
            | ReasoningEffort::High
            | ReasoningEffort::XHigh
            | ReasoningEffort::Max
    ) {
        return Err("Jev selected an unsupported effort.".to_string());
    }
    if decision.stats.as_ref().is_some_and(|stats| {
        [stats.model_confidence, stats.effort_confidence]
            .into_iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    }) {
        return Err("Invalid Jev routing stats.".to_string());
    }
    Ok(decision)
}

/// A prefix of visible evidence; truncation never claims to be a complete summary.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct JevExcerpt {
    text: String,
    truncated: bool,
}

impl JevExcerpt {
    pub(crate) fn new(text: &str) -> Self {
        let end = text.floor_char_boundary(text.len().min(768));
        Self {
            text: text[..end].to_string(),
            truncated: end < text.len(),
        }
    }
}

/// Classifier-only evidence. Never injected into the execution model's history.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct JevContext {
    source: &'static str,
    partial: bool,
    recent_requests: Vec<JevExcerpt>,
    pub(crate) recent_response: Option<JevExcerpt>,
    pub(crate) last_turn_status: Option<&'static str>,
    pub(crate) active_goal: Option<JevExcerpt>,
    pub(crate) images_attached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transcript: Option<crate::jev_context::JevTranscript>,
}

impl Default for JevContext {
    fn default() -> Self {
        Self {
            source: "current_widget_visible_history",
            partial: true,
            recent_requests: Vec::new(),
            recent_response: None,
            last_turn_status: None,
            active_goal: None,
            images_attached: false,
            transcript: None,
        }
    }
}

impl JevContext {
    pub(crate) fn record_request(&mut self, text: &str) {
        if self.recent_requests.len() == 3 {
            self.recent_requests.remove(0);
        }
        self.recent_requests.push(JevExcerpt::new(text));
        self.recent_response = None;
        self.last_turn_status = None;
    }
}

pub(crate) async fn choose(task: String, context: JevContext) -> Result<JevDecision, String> {
    if task.len() > 24_000 || (task.trim().is_empty() && !context.images_attached) {
        return Err("Jev needs text or an image attachment, with text under 24 KB.".to_string());
    }
    let started = Instant::now();
    let operation = async {
        let helper = std::env::current_exe()
            .map_err(|_| "Cannot locate the Codex Jev installation.")?
            .with_file_name("jev-route.py");
        if !helper.is_file() {
            return Err("Jev helper missing. Run tools/fork/install.sh.".to_string());
        }
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
        let payload = serde_json::to_vec(&serde_json::json!({"task": task, "context": context}))
            .map_err(|_| "Cannot encode Jev request.")?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|_| "Cannot send Jev request.")?;
        drop(stdin);
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .ok_or("Jev stdout unavailable.")?
            .take(4097)
            .read_to_end(&mut output)
            .await
            .map_err(|_| "Cannot read Jev decision.")?;
        if output.len() > 4096 {
            return Err("Jev helper output exceeded its limit.".to_string());
        }
        if !child
            .wait()
            .await
            .map_err(|_| "Jev helper failed.")?
            .success()
        {
            // Only the bundled helper's safe error reaches the UI.
            let value: serde_json::Value = serde_json::from_slice(&output)
                .map_err(|_| "Jev routing failed. Your message was not sent.")?;
            return Err(value["error"]
                .as_str()
                .unwrap_or("Jev routing failed.")
                .to_string());
        }
        let mut decision = parse_decision(&output)?;
        decision.elapsed = started.elapsed();
        Ok(decision)
    };
    tokio::time::timeout(Duration::from_secs(35), operation)
        .await
        .map_err(|_| "Jev routing timed out. Your message was not sent.".to_string())?
}

#[cfg(test)]
#[path = "jev_route_tests.rs"]
mod tests;
