//! Bounded subprocess boundary for the bundled Jev classifier. No prompts in diagnostics.
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JevDecision {
    pub(crate) model: String,
    pub(crate) effort: ReasoningEffort,
}

pub(crate) async fn choose(task: String) -> Result<JevDecision, String> {
    if task.trim().is_empty() || task.len() > 24_000 {
        return Err("Jev needs a nonempty text message under 24 KB.".to_string());
    }
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
        let payload = serde_json::to_vec(&serde_json::json!({"task": task}))
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
        let decision: JevDecision =
            serde_json::from_slice(&output).map_err(|_| "Invalid Jev decision.")?;
        if !matches!(decision.model.as_str(), "gpt-6.1-sol" | "gpt-6-luna") {
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
        Ok(decision)
    };
    tokio::time::timeout(Duration::from_secs(35), operation)
        .await
        .map_err(|_| "Jev routing timed out. Your message was not sent.".to_string())?
}
