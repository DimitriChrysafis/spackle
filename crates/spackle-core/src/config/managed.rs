//! Managed llama-server supervision settings.

use serde::{Deserialize, Serialize};

use super::error::{ConfigError, Issue};

/// KV cache quantization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CacheType {
    F16,
    Bf16,
    F32,
    Q8_0,
    Q4_0,
}

/// GPU layer selection (`--n-gpu-layers` accepts `all`, `auto`, or a number).
#[derive(Debug, Clone, Copy, PartialEq, Eq, schemars::JsonSchema)]
pub enum GpuLayers {
    All,
    Auto,
    Count(u32),
}

impl Default for GpuLayers {
    fn default() -> Self {
        Self::All
    }
}

impl Serialize for GpuLayers {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All => serializer.serialize_str("all"),
            Self::Auto => serializer.serialize_str("auto"),
            Self::Count(n) => serializer.serialize_u64(u64::from(*n)),
        }
    }
}

impl<'de> Deserialize<'de> for GpuLayers {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = GpuLayers;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(r#""all", "auto", or a non-negative integer"#)
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<GpuLayers, E> {
                match v {
                    "all" => Ok(GpuLayers::All),
                    "auto" => Ok(GpuLayers::Auto),
                    other => Err(E::invalid_value(serde::de::Unexpected::Str(other), &self)),
                }
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<GpuLayers, E> {
                u32::try_from(v)
                    .map(GpuLayers::Count)
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Unsigned(v), &self))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<GpuLayers, E> {
                if v >= 0 {
                    self.visit_u64(v as u64)
                } else {
                    Err(E::invalid_value(serde::de::Unexpected::Signed(v), &self))
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// Reasoning output format passed to llama-server (`--reasoning-format`).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningFormat {
    /// Thoughts stay unparsed in `content` (Qwen template handles it).
    #[default]
    None,
    /// Thoughts are extracted into `reasoning_content`.
    Deepseek,
    /// Legacy tag style, also extracted into `reasoning_content`.
    DeepseekLegacy,
}

/// Flash attention switch (`--flash-attn`). Accepts `true`/`false` (mapped
/// to on/off) or `"off"`, `"auto"`, `"on"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, schemars::JsonSchema)]
pub enum FlashAttention {
    Off,
    #[default]
    Auto,
    On,
}

impl FlashAttention {
    fn as_token(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Auto => "auto",
            Self::On => "on",
        }
    }
}

impl Serialize for FlashAttention {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_token())
    }
}

impl<'de> Deserialize<'de> for FlashAttention {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = FlashAttention;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("true, false, \"off\", \"auto\", or \"on\"")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<FlashAttention, E> {
                Ok(if v {
                    FlashAttention::On
                } else {
                    FlashAttention::Off
                })
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<FlashAttention, E> {
                match v {
                    "off" => Ok(FlashAttention::Off),
                    "auto" => Ok(FlashAttention::Auto),
                    "on" | "true" => Ok(FlashAttention::On),
                    other => Err(E::invalid_value(serde::de::Unexpected::Str(other), &self)),
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// Speculative decoding. Disabled by default; each mode needs its own
/// prerequisites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SpeculativeConfig {
    pub mode: SpeculativeMode,
    /// Draft model path (draft mode).
    pub draft_model: Option<String>,
    /// Draft tokens to speculate (draft/mtp).
    pub draft_max: u32,
    /// n-gram context bounds (ngram mode).
    pub ngram_min: u32,
    pub ngram_max: u32,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            mode: SpeculativeMode::None,
            draft_model: None,
            draft_max: 0,
            ngram_min: 0,
            ngram_max: 0,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SpeculativeMode {
    #[default]
    None,
    Draft,
    Mtp,
    Ngram,
}

/// Settings for launching and supervising a llama-server child process.
/// Only used when `endpoint.mode = "managed"`; attach mode never touches
/// these values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ManagedServerConfig {
    /// Reserved; managed mode is opt-in through `endpoint.mode`.
    pub enabled: bool,
    /// llama-server binary path, or `"auto"` to search PATH and known local
    /// runtime locations.
    pub binary: String,
    pub model_path: String,
    pub mmproj_path: Option<String>,
    pub host: String,
    pub port: u16,
    /// 0 lets llama-server choose its own limit.
    pub context_size: u32,
    pub batch_size: u32,
    pub micro_batch_size: u32,
    pub cpu_threads: Option<u32>,
    pub batch_threads: Option<u32>,
    pub gpu_layers: GpuLayers,
    pub flash_attention: FlashAttention,
    pub cache_type_k: CacheType,
    pub cache_type_v: CacheType,
    pub parallel_slots: u32,
    pub mmap: bool,
    pub mlock: bool,
    pub reasoning_format: ReasoningFormat,
    pub metrics: bool,
    pub speculative: SpeculativeConfig,
    /// Advanced extra arguments appended verbatim (advanced, validated as
    /// shell words only).
    pub extra_args: Vec<String>,
}

impl Default for ManagedServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            binary: "auto".to_owned(),
            model_path: String::new(),
            mmproj_path: None,
            host: "127.0.0.1".to_owned(),
            port: 8080,
            context_size: 0,
            batch_size: 512,
            micro_batch_size: 128,
            cpu_threads: None,
            batch_threads: None,
            gpu_layers: GpuLayers::All,
            flash_attention: FlashAttention::Auto,
            cache_type_k: CacheType::F16,
            cache_type_v: CacheType::F16,
            parallel_slots: 1,
            mmap: true,
            mlock: false,
            reasoning_format: ReasoningFormat::default(),
            metrics: false,
            speculative: SpeculativeConfig::default(),
            extra_args: Vec::new(),
        }
    }
}

impl ManagedServerConfig {
    /// Build the exact command line that would be executed. Returned as
    /// resolved words so the UI can preview it verbatim.
    pub fn command(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        if !self.model_path.is_empty() {
            args.push("--model".to_owned());
            args.push(self.model_path.clone());
        }
        if let Some(mmproj) = &self.mmproj_path {
            args.push("--mmproj".to_owned());
            args.push(mmproj.clone());
        }
        args.push("--host".to_owned());
        args.push(self.host.clone());
        args.push("--port".to_owned());
        args.push(self.port.to_string());
        if self.context_size > 0 {
            args.push("--ctx-size".to_owned());
            args.push(self.context_size.to_string());
        }
        args.push("--n-gpu-layers".to_owned());
        match self.gpu_layers {
            GpuLayers::All => args.push("all".to_owned()),
            GpuLayers::Auto => args.push("auto".to_owned()),
            GpuLayers::Count(n) => args.push(n.to_string()),
        }
        args.push("--batch-size".to_owned());
        args.push(self.batch_size.to_string());
        args.push("--ubatch-size".to_owned());
        args.push(self.micro_batch_size.to_string());
        if let Some(threads) = self.cpu_threads {
            args.push("--threads".to_owned());
            args.push(threads.to_string());
        }
        if let Some(threads) = self.batch_threads {
            args.push("--threads-batch".to_owned());
            args.push(threads.to_string());
        }
        args.push("--flash-attn".to_owned());
        args.push(self.flash_attention.as_token().to_owned());
        args.push("--cache-type-k".to_owned());
        args.push(self.cache_type_k.as_arg().to_owned());
        args.push("--cache-type-v".to_owned());
        args.push(self.cache_type_v.as_arg().to_owned());
        if self.parallel_slots > 1 {
            args.push("--parallel".to_owned());
            args.push(self.parallel_slots.to_string());
        }
        if !self.mmap {
            args.push("--no-mmap".to_owned());
        }
        if self.mlock {
            args.push("--mlock".to_owned());
        }
        if self.reasoning_format != ReasoningFormat::None {
            args.push("--reasoning-format".to_owned());
            args.push(self.reasoning_format.as_arg().to_owned());
        }
        if self.metrics {
            args.push("--metrics".to_owned());
        }
        match self.speculative.mode {
            SpeculativeMode::None => {}
            SpeculativeMode::Draft => {
                args.push("--spec-type".to_owned());
                args.push("draft-simple".to_owned());
                if let Some(model) = &self.speculative.draft_model {
                    args.push("--spec-draft-model".to_owned());
                    args.push(model.clone());
                }
                if self.speculative.draft_max > 0 {
                    args.push("--spec-draft-n-max".to_owned());
                    args.push(self.speculative.draft_max.to_string());
                }
            }
            SpeculativeMode::Mtp => {
                args.push("--spec-type".to_owned());
                args.push("draft-mtp".to_owned());
                if let Some(model) = &self.speculative.draft_model {
                    args.push("--spec-draft-model".to_owned());
                    args.push(model.clone());
                }
                if self.speculative.draft_max > 0 {
                    args.push("--spec-draft-n-max".to_owned());
                    args.push(self.speculative.draft_max.to_string());
                }
            }
            SpeculativeMode::Ngram => {
                args.push("--spec-type".to_owned());
                args.push("ngram-mod".to_owned());
                if self.speculative.ngram_min > 0 {
                    args.push("--spec-ngram-mod-n-min".to_owned());
                    args.push(self.speculative.ngram_min.to_string());
                }
                if self.speculative.ngram_max > 0 {
                    args.push("--spec-ngram-mod-n-max".to_owned());
                    args.push(self.speculative.ngram_max.to_string());
                }
            }
        }
        args.extend(self.extra_args.iter().cloned());
        args
    }
}

impl CacheType {
    fn as_arg(self) -> &'static str {
        match self {
            Self::F16 => "f16",
            Self::Bf16 => "bf16",
            Self::F32 => "f32",
            Self::Q8_0 => "q8_0",
            Self::Q4_0 => "q4_0",
        }
    }
}

impl ReasoningFormat {
    fn as_arg(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Deepseek => "deepseek",
            Self::DeepseekLegacy => "deepseek-legacy",
        }
    }
}

/// Validate managed-server settings and relational constraints.
pub fn validate(config: &ManagedServerConfig) -> Result<(), ConfigError> {
    let mut issues = Vec::new();
    if config.model_path.trim().is_empty() {
        issues.push(
            Issue::new(
                "managed_server.model_path",
                "model path is required for managed mode",
            )
            .with_hint("point at a local GGUF file"),
        );
    }
    if config.port == 0 {
        issues.push(Issue::new("managed_server.port", "port must be 1..=65535"));
    }
    if config.batch_size == 0 {
        issues.push(Issue::new(
            "managed_server.batch_size",
            "batch size must be at least 1",
        ));
    }
    if config.micro_batch_size == 0 {
        issues.push(Issue::new(
            "managed_server.micro_batch_size",
            "micro-batch size must be at least 1",
        ));
    }
    if config.micro_batch_size > config.batch_size {
        issues.push(
            Issue::new(
                "managed_server.micro_batch_size",
                format!(
                    "micro-batch {} exceeds batch {}",
                    config.micro_batch_size, config.batch_size
                ),
            )
            .with_hint("micro-batch (ubatch) must be <= batch size"),
        );
    }
    if config.parallel_slots == 0 {
        issues.push(Issue::new(
            "managed_server.parallel_slots",
            "parallel slots must be at least 1",
        ));
    }
    if let GpuLayers::Count(n) = config.gpu_layers
        && n > 512
    {
        issues.push(Issue::new(
            "managed_server.gpu_layers",
            format!("layer count {n} exceeds 512"),
        ));
    }
    let spec = &config.speculative;
    if spec.mode == SpeculativeMode::Draft && spec.draft_model.is_none() {
        issues.push(
            Issue::new(
                "managed_server.speculative.draft_model",
                "draft-simple mode requires a draft model path",
            )
            .with_hint("set managed_server.speculative.draft_model or use mode = \"none\""),
        );
    }
    if spec.mode != SpeculativeMode::None && spec.draft_max > 256 {
        issues.push(Issue::new(
            "managed_server.speculative.draft_max",
            format!("{} exceeds the 256 draft-token bound", spec.draft_max),
        ));
    }
    if spec.mode == SpeculativeMode::Ngram && (spec.ngram_min > 1024 || spec.ngram_max > 1024) {
        issues.push(Issue::new(
            "managed_server.speculative.ngram_min",
            "ngram bounds must be within 0..=1024",
        ));
    }
    if spec.mode == SpeculativeMode::Ngram && spec.ngram_max < spec.ngram_min {
        issues.push(Issue::new(
            "managed_server.speculative.ngram_max",
            format!(
                "ngram_max {} is below ngram_min {}",
                spec.ngram_max, spec.ngram_min
            ),
        ));
    }
    for (index, arg) in config.extra_args.iter().enumerate() {
        if arg.trim().is_empty() {
            issues.push(Issue::new(
                format!("managed_server.extra_args[{index}]"),
                "empty extra argument",
            ));
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

    fn with_model() -> ManagedServerConfig {
        ManagedServerConfig {
            model_path: "/tmp/model.gguf".to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn defaults_validate() {
        assert!(validate(&with_model()).is_ok());
    }

    #[test]
    fn micro_batch_must_not_exceed_batch() {
        let config = ManagedServerConfig {
            model_path: "/tmp/model.gguf".to_owned(),
            batch_size: 128,
            micro_batch_size: 256,
            ..Default::default()
        };
        let error = validate(&config).expect_err("must fail");
        assert!(error.to_string().contains("micro_batch_size"));
        assert!(error.to_string().contains("hint:"));
    }

    #[test]
    fn draft_mode_requires_draft_model() {
        let config = ManagedServerConfig {
            model_path: "/tmp/model.gguf".to_owned(),
            speculative: SpeculativeConfig {
                mode: SpeculativeMode::Draft,
                ..Default::default()
            },
            ..Default::default()
        };
        let error = validate(&config).expect_err("must fail");
        assert!(error.to_string().contains("draft_model"));
    }

    #[test]
    fn command_includes_core_flags_and_previewable() {
        let config = ManagedServerConfig {
            model_path: "/tmp/model.gguf".to_owned(),
            batch_size: 2048,
            micro_batch_size: 512,
            flash_attention: FlashAttention::On,
            cache_type_k: CacheType::Q8_0,
            cache_type_v: CacheType::Q8_0,
            speculative: SpeculativeConfig {
                mode: SpeculativeMode::Ngram,
                ngram_min: 4,
                ngram_max: 8,
                ..Default::default()
            },
            ..Default::default()
        };
        let command = config.command();
        let joined = command.join(" ");
        assert!(joined.contains("--model /tmp/model.gguf"));
        assert!(joined.contains("--n-gpu-layers all"));
        assert!(joined.contains("--flash-attn on"));
        assert!(joined.contains("--cache-type-k q8_0"));
        assert!(joined.contains("--batch-size 2048"));
        assert!(joined.contains("--ubatch-size 512"));
        assert!(joined.contains("--spec-type ngram-mod"));
        assert!(joined.contains("--spec-ngram-mod-n-min 4"));
        assert!(joined.contains("--spec-ngram-mod-n-max 8"));
    }

    #[test]
    fn flash_attention_accepts_bool_and_tokens() {
        let toml = r#"
            flash_attention = true
        "#;
        let config: ManagedServerConfig = toml::from_str(toml).expect("bool form");
        assert_eq!(config.flash_attention, FlashAttention::On);
        let toml = r#"
            flash_attention = "auto"
        "#;
        let config: ManagedServerConfig = toml::from_str(toml).expect("token form");
        assert_eq!(config.flash_attention, FlashAttention::Auto);
        let toml = "flash_attention = \"bogus\"";
        assert!(toml::from_str::<ManagedServerConfig>(toml).is_err());
    }
}
