//! Opt-in footer metadata for the current widget's latest acknowledged Jev route.
use super::jev_auto::JevAutoState;
use crate::jev_route::JevRouteStats;
use crate::status::format_tokens_compact;
use std::time::Duration;

#[derive(Default)]
pub(super) enum JevRouteStatus {
    #[default]
    Ready,
    Complete {
        elapsed: Duration,
        stats: Option<JevRouteStats>,
    },
    Failed,
}

impl JevAutoState {
    pub(super) fn stats_display(&self) -> String {
        if !self.enabled {
            return "Jev off".to_string();
        }
        if self.pending.is_some() {
            return "Jev routing…".to_string();
        }
        match &self.status {
            JevRouteStatus::Ready => "Jev ready".to_string(),
            JevRouteStatus::Failed => "Jev failed".to_string(),
            JevRouteStatus::Complete { elapsed, stats } => {
                let timing = format!("Jev {:.1}s", elapsed.as_secs_f64());
                let Some(stats) = stats else {
                    return timing;
                };
                format!(
                    "{timing} M{:.0}% E{:.0}% {}/{} tok",
                    stats.model_confidence * 100.0,
                    stats.effort_confidence * 100.0,
                    compact_tokens(stats.input_tokens),
                    compact_tokens(stats.output_tokens),
                )
            }
        }
    }
}

fn compact_tokens(tokens: u64) -> String {
    format_tokens_compact(i64::try_from(tokens).unwrap_or(i64::MAX))
}
