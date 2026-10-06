//! Telemetry: server-authoritative values plus clearly-labelled estimates.

use serde::{Deserialize, Serialize};

/// Authoritative per-turn telemetry from the server where available.
///
/// `prompt_tokens_per_second` and `predicted_tokens_per_second` come from
/// the response `timings` object and are authoritative even when `/metrics`
/// is unavailable (HTTP 501). Locally derived numbers must be presented as
/// estimates.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TurnTelemetry {
    pub prompt_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub predicted_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub prompt_tokens_per_second: Option<f64>,
    pub predicted_tokens_per_second: Option<f64>,
    pub time_to_first_token_ms: Option<u64>,
    pub request_duration_ms: Option<u64>,
    /// Slot busy state observed during the turn.
    pub slot_busy: Option<bool>,
    /// Speculative acceptance rate (only when genuinely reported).
    pub speculative_acceptance_rate: Option<f64>,
}

impl TurnTelemetry {
    /// Merge in a strictly-more-authoritative update (server results win
    /// over estimates).
    pub fn merge(&mut self, other: &Self) {
        macro_rules! take_if_some {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field;
                }
            };
        }
        take_if_some!(prompt_tokens);
        take_if_some!(cached_tokens);
        take_if_some!(predicted_tokens);
        take_if_some!(reasoning_tokens);
        take_if_some!(prompt_tokens_per_second);
        take_if_some!(predicted_tokens_per_second);
        take_if_some!(time_to_first_token_ms);
        take_if_some!(request_duration_ms);
        take_if_some!(slot_busy);
        take_if_some!(speculative_acceptance_rate);
    }
}

/// A live, bounded telemetry sample for the TUI status line.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TelemetrySample {
    pub tokens_per_second: Option<f64>,
    pub tokens_per_second_is_estimate: bool,
    pub time_to_first_token_ms: Option<u64>,
    pub context_used: Option<u64>,
    pub context_capacity: Option<u64>,
    pub cache_hit_percent: Option<f64>,
    pub step: u32,
    pub tool_duration_ms: Option<u64>,
    pub server: Option<String>,
    pub model: Option<String>,
    pub quantization: Option<String>,
    pub profile: Option<String>,
    pub slot_busy: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_prefers_authoritative_values() {
        let mut base = TurnTelemetry {
            prompt_tokens: Some(100),
            predicted_tokens_per_second: Some(12.5),
            ..Default::default()
        };
        base.merge(&TurnTelemetry {
            cached_tokens: Some(80),
            ..Default::default()
        });
        assert_eq!(base.prompt_tokens, Some(100));
        assert_eq!(base.cached_tokens, Some(80));
    }

    #[test]
    fn merge_does_not_overwrite_with_none() {
        let mut base = TurnTelemetry {
            prompt_tokens: Some(100),
            ..Default::default()
        };
        base.merge(&TurnTelemetry::default());
        assert_eq!(base.prompt_tokens, Some(100));
    }
}
