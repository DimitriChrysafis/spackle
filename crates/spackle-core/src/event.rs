//! Deterministic domain events. Events are the journal currency: the same
//! state transitions always produce the same event sequence, which makes
//! resume, audit, and TUI replay possible.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The set of domain events the agent emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A user turn started.
    TurnStarted,
    /// A turn completed with a final assistant response.
    TurnCompleted,
    /// A turn ended in error.
    TurnFailed,
    /// A turn was cancelled by the user.
    TurnCancelled,
    /// A model inference step began.
    InferenceStarted,
    /// A model inference step finished.
    InferenceFinished,
    /// A tool call began.
    ToolCallStarted,
    /// A tool call finished (successfully or with a recorded error).
    ToolCallFinished,
    /// An approval decision was requested from the user.
    ApprovalRequested,
    /// The user decided an approval request.
    ApprovalDecided,
    /// Context compaction began (between turns only).
    CompactionStarted,
    /// Context compaction finished.
    CompactionFinished,
    /// Telemetry update (usage, timings, cache).
    Telemetry,
    /// A session was restored from its journal.
    SessionRestored,
    /// A managed server action (start/stop/status) completed.
    ServerAction,
}

impl EventKind {
    /// Stable wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TurnStarted => "turn_started",
            Self::TurnCompleted => "turn_completed",
            Self::TurnFailed => "turn_failed",
            Self::TurnCancelled => "turn_cancelled",
            Self::InferenceStarted => "inference_started",
            Self::InferenceFinished => "inference_finished",
            Self::ToolCallStarted => "tool_call_started",
            Self::ToolCallFinished => "tool_call_finished",
            Self::ApprovalRequested => "approval_requested",
            Self::ApprovalDecided => "approval_decided",
            Self::CompactionStarted => "compaction_started",
            Self::CompactionFinished => "compaction_finished",
            Self::Telemetry => "telemetry",
            Self::SessionRestored => "session_restored",
            Self::ServerAction => "server_action",
        }
    }
}

/// One recorded domain event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub kind: EventKind,
    pub payload: serde_json::Value,
}

impl AgentEvent {
    /// Build an event with canonical payload fields.
    #[must_use]
    pub fn new(kind: EventKind, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            kind,
            payload,
        }
    }

    #[must_use]
    pub fn turn_started(session: &str, turn: u32) -> Self {
        Self::new(
            EventKind::TurnStarted,
            serde_json::json!({ "session": session, "turn": turn }),
        )
    }

    #[must_use]
    pub fn turn_completed(session: &str, steps: u32) -> Self {
        Self::new(
            EventKind::TurnCompleted,
            serde_json::json!({ "session": session, "steps": steps }),
        )
    }

    #[must_use]
    pub fn turn_failed(session: &str, reason: &str) -> Self {
        Self::new(
            EventKind::TurnFailed,
            serde_json::json!({ "session": session, "reason": reason }),
        )
    }

    #[must_use]
    pub fn turn_cancelled(session: &str, step: u32) -> Self {
        Self::new(
            EventKind::TurnCancelled,
            serde_json::json!({ "session": session, "step": step }),
        )
    }

    #[must_use]
    pub fn inference_started(step: u32) -> Self {
        Self::new(
            EventKind::InferenceStarted,
            serde_json::json!({ "step": step }),
        )
    }

    #[must_use]
    pub fn inference_finished(step: u32, finish_reason: &str) -> Self {
        Self::new(
            EventKind::InferenceFinished,
            serde_json::json!({ "step": step, "finish_reason": finish_reason }),
        )
    }

    #[must_use]
    pub fn tool_call_started(id: &str, name: &str) -> Self {
        Self::new(
            EventKind::ToolCallStarted,
            serde_json::json!({ "id": id, "name": name }),
        )
    }

    #[must_use]
    pub fn tool_call_finished(id: &str, name: &str, is_error: bool, ms: u64) -> Self {
        Self::new(
            EventKind::ToolCallFinished,
            serde_json::json!({ "id": id, "name": name, "is_error": is_error, "duration_ms": ms }),
        )
    }

    #[must_use]
    pub fn telemetry(payload: serde_json::Value) -> Self {
        Self::new(EventKind::Telemetry, payload)
    }

    #[must_use]
    pub fn session_restored(session: &str, events: u64) -> Self {
        Self::new(
            EventKind::SessionRestored,
            serde_json::json!({ "session": session, "events": events }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_json_round_trips() {
        let event = AgentEvent::turn_started("ws-abc", 3);
        let text = serde_json::to_string(&event).expect("serialize");
        assert!(text.contains("\"turn_started\""));
        let back: AgentEvent = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(back.kind, EventKind::TurnStarted);
        assert_eq!(back.payload["turn"], 3);
    }

    #[test]
    fn all_kinds_have_stable_names() {
        let kinds = [
            EventKind::TurnStarted,
            EventKind::TurnCompleted,
            EventKind::TurnFailed,
            EventKind::TurnCancelled,
            EventKind::InferenceStarted,
            EventKind::InferenceFinished,
            EventKind::ToolCallStarted,
            EventKind::ToolCallFinished,
            EventKind::ApprovalRequested,
            EventKind::ApprovalDecided,
            EventKind::CompactionStarted,
            EventKind::CompactionFinished,
            EventKind::Telemetry,
            EventKind::SessionRestored,
            EventKind::ServerAction,
        ];
        let names: Vec<&str> = kinds.iter().map(|kind| kind.as_str()).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(names.len(), unique.len(), "names must be unique");
        for name in &names {
            assert!(
                name.contains('_') || *name == "telemetry",
                "{name} snake_case"
            );
        }
    }
}
