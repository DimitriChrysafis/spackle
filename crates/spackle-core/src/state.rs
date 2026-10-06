//! Explicit agent states. Transitions are driven by [`crate::agent::run_turn`]
//! and recorded as events; there is no hidden recursive control flow.

use serde::{Deserialize, Serialize};

/// Agent lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// Waiting for a user turn.
    Idle,
    /// Validating context budget and assembling the request.
    Preparing,
    /// Streaming model output.
    Inferring,
    /// Executing approved tool calls.
    ExecutingTools,
    /// Waiting for a user approval decision.
    AwaitingApproval,
    /// Cancellation in progress; owned work is being dropped.
    Cancelling,
    /// Turn completed successfully.
    Completed,
    /// Turn ended in error.
    Error,
}

impl AgentState {
    /// All states (stable order for UI/tooling).
    #[must_use]
    pub fn all() -> [AgentState; 8] {
        [
            AgentState::Idle,
            AgentState::Preparing,
            AgentState::Inferring,
            AgentState::ExecutingTools,
            AgentState::AwaitingApproval,
            AgentState::Cancelling,
            AgentState::Completed,
            AgentState::Error,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_serialize_stably() {
        assert_eq!(
            serde_json::to_string(&AgentState::ExecutingTools).expect("serialize"),
            "\"executing_tools\""
        );
        let back: AgentState = serde_json::from_str("\"awaiting_approval\"").expect("deserialize");
        assert_eq!(back, AgentState::AwaitingApproval);
    }

    #[test]
    fn all_states_are_distinct() {
        let states = AgentState::all();
        for (i, a) in states.iter().enumerate() {
            for b in &states[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(states.len(), 8);
    }
}
