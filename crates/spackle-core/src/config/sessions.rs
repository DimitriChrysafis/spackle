//! Session journaling parameters.

use serde::{Deserialize, Serialize};

use super::error::{ConfigError, Issue};

/// Session journal settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SessionsConfig {
    /// Retain journals for this many days (0 keeps everything).
    pub retention_days: u32,
    /// Keep at most this many journals per workspace (0 keeps everything).
    pub max_journals: u32,
    /// Checkpoint the transcript every N events (0 disables checkpoints).
    pub checkpoint_every_events: u32,
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            max_journals: 20,
            checkpoint_every_events: 256,
        }
    }
}

/// Validate session settings.
pub fn validate(config: &SessionsConfig) -> Result<(), ConfigError> {
    let mut issues = Vec::new();
    if config.retention_days > 3650 {
        issues.push(Issue::new(
            "sessions.retention_days",
            format!("{} exceeds the 3650 day bound", config.retention_days),
        ));
    }
    if config.max_journals > 10_000 {
        issues.push(Issue::new(
            "sessions.max_journals",
            format!("{} exceeds the 10000 bound", config.max_journals),
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
        assert!(validate(&SessionsConfig::default()).is_ok());
    }
}
