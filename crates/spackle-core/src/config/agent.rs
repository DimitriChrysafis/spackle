//! Agent loop limits and confirmation policy.

use serde::{Deserialize, Serialize};

use super::error::{ConfigError, Issue};

/// Whether a class of actions may run without asking.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationMode {
    /// Never ask; the action is refused.
    Deny,
    /// Ask the user before each action.
    #[default]
    Ask,
    /// Allow without asking (still recorded in the audit trail).
    Allow,
}

/// Confirmation policy per action class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ConfirmationPolicy {
    /// File changes and other workspace mutations.
    pub writes: ConfirmationMode,
    /// Destructive commands (removals, force operations).
    pub destructive: ConfirmationMode,
    /// Commands that touch the network.
    pub network_commands: ConfirmationMode,
    /// Reads/writes of likely secret files.
    pub secret_files: ConfirmationMode,
}

impl Default for ConfirmationPolicy {
    fn default() -> Self {
        Self {
            writes: ConfirmationMode::Ask,
            destructive: ConfirmationMode::Deny,
            network_commands: ConfirmationMode::Deny,
            secret_files: ConfirmationMode::Ask,
        }
    }
}

/// Limits that keep agent turns bounded and auditable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    /// Maximum inference/tool steps per turn.
    pub max_steps: u32,
    /// Transient model retries per step (connection resets, HTTP 5xx).
    pub max_model_retries: u32,
    /// How many identical normalized tool calls (with no progress in
    /// between) are tolerated before the turn stops with a diagnostic.
    pub repeated_call_limit: u32,
    /// Maximum bytes kept from a single tool result.
    pub max_tool_output_bytes: u64,
    /// Maximum wall-clock duration of one turn in seconds (0 disables).
    pub max_turn_seconds: u64,
    /// Explicit workspace override; normally derived from the project
    /// directory or the current directory.
    pub workspace: Option<String>,
    /// Confirmation policy for tool actions.
    pub confirmations: ConfirmationPolicy,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 24,
            max_model_retries: 1,
            repeated_call_limit: 3,
            max_tool_output_bytes: 128 * 1024,
            max_turn_seconds: 0,
            workspace: None,
            confirmations: ConfirmationPolicy::default(),
        }
    }
}

/// Validate agent limits.
pub fn validate(config: &AgentConfig) -> Result<(), ConfigError> {
    let mut issues = Vec::new();
    if config.max_steps == 0 {
        issues.push(
            Issue::new("agent.max_steps", "must be at least 1")
                .with_hint("set agent.max_steps = 1 or higher"),
        );
    }
    if config.max_steps > 256 {
        issues.push(Issue::new(
            "agent.max_steps",
            format!("{} exceeds the 256 sanity bound", config.max_steps),
        ));
    }
    if config.max_model_retries > 5 {
        issues.push(Issue::new(
            "agent.max_model_retries",
            format!(
                "{} is more than 5; retrying a local model rarely helps",
                config.max_model_retries
            ),
        ));
    }
    if config.repeated_call_limit == 0 {
        issues.push(
            Issue::new("agent.repeated_call_limit", "must be at least 1")
                .with_hint("set it to 1 to stop after the first repeated call"),
        );
    }
    if config.max_tool_output_bytes < 1024 {
        issues.push(Issue::new(
            "agent.max_tool_output_bytes",
            format!(
                "{} is below the 1024 byte floor",
                config.max_tool_output_bytes
            ),
        ));
    }
    if config.max_tool_output_bytes > 16 * 1024 * 1024 {
        issues.push(Issue::new(
            "agent.max_tool_output_bytes",
            format!(
                "{} exceeds the 16 MiB ceiling",
                config.max_tool_output_bytes
            ),
        ));
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(ConfigError::new(issues))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate() {
        assert!(validate(&AgentConfig::default()).is_ok());
    }

    #[test]
    fn zero_limits_are_rejected_with_hints() {
        let config = AgentConfig {
            max_steps: 0,
            repeated_call_limit: 0,
            ..Default::default()
        };
        let error = validate(&config).expect_err("must fail");
        let text = error.to_string();
        assert!(text.contains("agent.max_steps"));
        assert!(text.contains("hint:"));
        assert!(text.contains("agent.repeated_call_limit"));
    }

    #[test]
    fn destructive_and_network_default_to_deny() {
        let policy = ConfirmationPolicy::default();
        assert_eq!(policy.destructive, ConfirmationMode::Deny);
        assert_eq!(policy.network_commands, ConfirmationMode::Deny);
        assert_eq!(policy.writes, ConfirmationMode::Ask);
    }
}
