//! Configuration diagnostics with actionable, path-qualified messages.

/// A single configuration problem or warning.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Issue {
    /// Dotted configuration path, e.g. `managed_server.micro_batch_size`.
    pub path: String,
    /// What is wrong.
    pub message: String,
    /// Suggested fix, when one can be stated precisely.
    pub hint: Option<String>,
}

impl Issue {
    #[must_use]
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            hint: None,
        }
    }

    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Collected configuration failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    issues: Vec<Issue>,
}

impl ConfigError {
    #[must_use]
    pub fn new(issues: Vec<Issue>) -> Self {
        Self { issues }
    }

    #[must_use]
    pub fn single(issue: Issue) -> Self {
        Self {
            issues: vec![issue],
        }
    }

    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    #[must_use]
    pub fn into_issues(self) -> Vec<Issue> {
        self.issues
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, issue) in self.issues.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{}: {}", issue.path, issue.message)?;
            if let Some(hint) = &issue.hint {
                write!(f, "\n  hint: {hint}")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_path_message_and_hint() {
        let error = ConfigError::single(
            Issue::new("agent.max_steps", "must be at least 1")
                .with_hint("set agent.max_steps = 1 or higher"),
        );
        let text = error.to_string();
        assert!(text.contains("agent.max_steps"));
        assert!(text.contains("must be at least 1"));
        assert!(text.contains("hint:"));
    }
}
