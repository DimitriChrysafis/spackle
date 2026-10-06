//! Generation profiles for local models. Profiles map onto standard Chat
//! Completions sampling fields; Qwen-specific knobs travel through
//! `chat_template_kwargs` and request-level fields (handled by the wire layer).

use serde::{Deserialize, Serialize};

use super::error::{ConfigError, Issue};

/// Reasoning effort, passed through as `reasoning_effort`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    #[default]
    Medium,
    High,
    Xhigh,
}

/// One named sampling profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GenerationProfile {
    /// Qwen `enable_thinking` chat template keyword.
    pub enable_thinking: Option<bool>,
    /// Qwen `preserve_thinking` chat template keyword.
    pub preserve_thinking: Option<bool>,
    /// Qwen `reasoning_effort`; models without support must surface a clear
    /// capability error rather than silently ignoring this.
    pub reasoning_effort: Option<ReasoningEffort>,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
    pub presence_penalty: f32,
    pub repeat_penalty: f32,
    /// Maximum generated tokens per inference step.
    pub max_tokens: u32,
}

impl Default for GenerationProfile {
    fn default() -> Self {
        Self {
            enable_thinking: None,
            preserve_thinking: None,
            reasoning_effort: None,
            temperature: 1.0,
            top_p: 1.0,
            top_k: 40,
            min_p: 0.0,
            presence_penalty: 0.0,
            repeat_penalty: 1.0,
            max_tokens: 4096,
        }
    }
}

/// The default profile set shipped with spackle.
pub fn default_profiles() -> std::collections::BTreeMap<String, GenerationProfile> {
    let fast = GenerationProfile {
        enable_thinking: Some(false),
        preserve_thinking: Some(true),
        temperature: 0.7,
        top_p: 0.8,
        top_k: 20,
        min_p: 0.0,
        presence_penalty: 1.5,
        repeat_penalty: 1.0,
        max_tokens: 8192,
        reasoning_effort: None,
    };
    let balanced = GenerationProfile {
        enable_thinking: Some(true),
        preserve_thinking: Some(true),
        reasoning_effort: Some(ReasoningEffort::Medium),
        temperature: 1.0,
        top_p: 0.95,
        top_k: 20,
        min_p: 0.0,
        presence_penalty: 0.0,
        repeat_penalty: 1.0,
        max_tokens: 16384,
    };
    let deep = GenerationProfile {
        enable_thinking: Some(true),
        preserve_thinking: Some(true),
        reasoning_effort: Some(ReasoningEffort::Xhigh),
        temperature: 1.0,
        top_p: 0.95,
        top_k: 20,
        min_p: 0.0,
        presence_penalty: 0.0,
        repeat_penalty: 1.0,
        max_tokens: 32768,
    };
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("fast".to_owned(), fast);
    profiles.insert("balanced".to_owned(), balanced);
    profiles.insert("deep".to_owned(), deep);
    profiles
}

/// Generation settings: the active profile plus the named profile set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GenerationConfig {
    /// Name of the active profile within `profiles`.
    pub active_profile: String,
    /// Named profiles.
    pub profiles: std::collections::BTreeMap<String, GenerationProfile>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            active_profile: "balanced".to_owned(),
            profiles: default_profiles(),
        }
    }
}

impl GenerationConfig {
    /// Active profile or `None` when the active name is unknown.
    pub fn active(&self) -> Option<&GenerationProfile> {
        self.profiles.get(&self.active_profile)
    }
}

fn valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// Validate profile parameters and relational constraints.
pub fn validate(config: &GenerationConfig) -> Result<(), ConfigError> {
    let mut issues = Vec::new();
    if !valid_profile_name(&config.active_profile) {
        issues.push(
            Issue::new(
                "generation.active_profile",
                format!("`{}` is not a valid profile name", config.active_profile),
            )
            .with_hint("use lowercase letters, digits, and dashes"),
        );
    }
    if config.profiles.is_empty() {
        issues.push(
            Issue::new("generation.profiles", "at least one profile is required")
                .with_hint("define [generation.profiles.name] sections"),
        );
    }
    if !config.profiles.contains_key(&config.active_profile) {
        issues.push(
            Issue::new(
                "generation.active_profile",
                format!("profile `{}` is not defined", config.active_profile),
            )
            .with_hint(format!(
                "defined profiles: {}",
                config
                    .profiles
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        );
    }
    for (name, profile) in &config.profiles {
        let path = |field: &str| format!("generation.profiles.{name}.{field}");
        if !valid_profile_name(name) {
            issues.push(Issue::new(
                format!("generation.profiles.{name}"),
                "profile name must use lowercase letters, digits, and dashes",
            ));
        }
        if !(0.0..=3.0).contains(&profile.temperature) {
            issues.push(Issue::new(
                path("temperature"),
                format!(
                    "temperature {} is out of range 0.0..=3.0",
                    profile.temperature
                ),
            ));
        }
        if !(0.0..=1.0).contains(&profile.top_p) || profile.top_p == 0.0 {
            issues.push(Issue::new(
                path("top_p"),
                format!("top_p {} must be in (0.0, 1.0]", profile.top_p),
            ));
        }
        if profile.top_k > 1000 {
            issues.push(Issue::new(
                path("top_k"),
                format!("top_k {} exceeds llama.cpp maximum 1000", profile.top_k),
            ));
        }
        if !(0.0..=1.0).contains(&profile.min_p) {
            issues.push(Issue::new(
                path("min_p"),
                format!("min_p {} must be in 0.0..=1.0", profile.min_p),
            ));
        }
        if !(-2.0..=2.0).contains(&profile.presence_penalty) {
            issues.push(Issue::new(
                path("presence_penalty"),
                format!(
                    "presence_penalty {} is out of range -2.0..=2.0",
                    profile.presence_penalty
                ),
            ));
        }
        if !(0.0..=2.0).contains(&profile.repeat_penalty) {
            issues.push(Issue::new(
                path("repeat_penalty"),
                format!(
                    "repeat_penalty {} is out of range 0.0..=2.0 (1.0 disables it)",
                    profile.repeat_penalty
                ),
            ));
        }
        if profile.max_tokens < 16 {
            issues.push(
                Issue::new(
                    path("max_tokens"),
                    format!("max_tokens {} is too small", profile.max_tokens),
                )
                .with_hint("use at least 16 tokens per step"),
            );
        }
        if profile.max_tokens > 131_072 {
            issues.push(Issue::new(
                path("max_tokens"),
                format!("max_tokens {} exceeds 131072", profile.max_tokens),
            ));
        }
        if profile.enable_thinking == Some(false) && profile.reasoning_effort.is_some() {
            issues.push(
                Issue::new(
                    path("reasoning_effort"),
                    "reasoning_effort is set while enable_thinking is false",
                )
                .with_hint("either enable thinking or clear reasoning_effort for this profile"),
            );
        }
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
    fn defaults_match_spec_profiles() {
        let config = GenerationConfig::default();
        assert_eq!(config.active_profile, "balanced");
        let fast = &config.profiles["fast"];
        assert_eq!(fast.enable_thinking, Some(false));
        assert!((fast.temperature - 0.7).abs() < f32::EPSILON);
        assert!((fast.presence_penalty - 1.5).abs() < f32::EPSILON);
        assert_eq!(fast.max_tokens, 8192);
        let balanced = &config.profiles["balanced"];
        assert_eq!(balanced.reasoning_effort, Some(ReasoningEffort::Medium));
        assert_eq!(balanced.max_tokens, 16384);
        let deep = &config.profiles["deep"];
        assert_eq!(deep.reasoning_effort, Some(ReasoningEffort::Xhigh));
        assert_eq!(deep.max_tokens, 32768);
    }

    #[test]
    fn unknown_active_profile_is_reported() {
        let config = GenerationConfig {
            active_profile: "turbo".to_owned(),
            ..Default::default()
        };
        let error = validate(&config).expect_err("must fail");
        let text = error.to_string();
        assert!(text.contains("generation.active_profile"));
        assert!(text.contains("turbo"));
        assert!(text.contains("balanced"));
    }

    #[test]
    fn out_of_range_parameters_are_reported_with_paths() {
        let mut config = GenerationConfig::default();
        let profile = config.profiles.get_mut("fast").unwrap();
        profile.top_p = 0.0;
        profile.temperature = 4.0;
        profile.max_tokens = 4;
        let error = validate(&config).expect_err("must fail");
        let text = error.to_string();
        assert!(text.contains("generation.profiles.fast.top_p"));
        assert!(text.contains("generation.profiles.fast.temperature"));
        assert!(text.contains("generation.profiles.fast.max_tokens"));
    }

    #[test]
    fn thinking_and_effort_conflict_is_flagged() {
        let mut config = GenerationConfig::default();
        let profile = config.profiles.get_mut("fast").unwrap();
        profile.reasoning_effort = Some(ReasoningEffort::High);
        let error = validate(&config).expect_err("must fail");
        assert!(error.to_string().contains("enable_thinking"));
    }

    #[test]
    fn all_defaults_validate() {
        assert!(validate(&GenerationConfig::default()).is_ok());
    }
}
