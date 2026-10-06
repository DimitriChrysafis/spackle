//! Context budgeting: keep llama.cpp's prefix/KV cache warm by reserving
//! headroom and compacting only between turns.

use serde::{Deserialize, Serialize};

use super::error::{ConfigError, Issue};

/// Context budget parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ContextConfig {
    /// Fraction of the server context at which compaction is triggered.
    pub compact_threshold: f32,
    /// Tokens reserved for in-flight tool results beyond the assistant
    /// response reserve (which follows the active profile's `max_tokens`).
    pub reserve_tool_tokens: u32,
    /// Maximum bytes kept per tool result before truncation with a marker.
    pub max_tool_result_bytes: Option<u64>,
    /// Whether reasoning content may be dropped first during compaction.
    pub drop_reasoning_first: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            compact_threshold: 0.85,
            reserve_tool_tokens: 512,
            max_tool_result_bytes: None,
            drop_reasoning_first: true,
        }
    }
}

/// Validate context budget parameters.
pub fn validate(config: &ContextConfig) -> Result<(), ConfigError> {
    let mut issues = Vec::new();
    if !(0.5..=1.0).contains(&config.compact_threshold) {
        issues.push(Issue::new(
            "context.compact_threshold",
            format!("{} must be in 0.5..=1.0", config.compact_threshold),
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
        assert!(validate(&ContextConfig::default()).is_ok());
    }

    fn with_threshold(threshold: f32) -> ContextConfig {
        ContextConfig {
            compact_threshold: threshold,
            ..Default::default()
        }
    }

    #[test]
    fn threshold_bounds_are_enforced() {
        assert!(validate(&with_threshold(0.4)).is_err());
        assert!(validate(&with_threshold(1.5)).is_err());
        assert!(validate(&with_threshold(0.9)).is_ok());
    }
}
