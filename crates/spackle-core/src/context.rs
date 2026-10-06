//! Context accounting against the server-reported context window.
//!
//! llama.cpp reports the *served* context (`n_ctx`), not the model's native
//! maximum; all accounting here must use the served value.

use serde::{Deserialize, Serialize};

use crate::config::context::ContextConfig;

/// A live context budget for one session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Server-reported served context (tokens).
    pub capacity: u32,
    /// Tokens reserved for the assistant response (profile `max_tokens`).
    pub reserve_assistant: u32,
    /// Tokens reserved for in-flight tool results.
    pub reserve_tool: u32,
    /// Tokens currently used by the prompt (system + transcript + tools).
    pub used: u32,
}

impl ContextBudget {
    /// Build a budget from the server context and configuration.
    pub fn new(capacity: u32, reserve_assistant: u32, config: &ContextConfig) -> Self {
        Self {
            capacity,
            reserve_assistant: reserve_assistant.min(capacity / 2),
            reserve_tool: config.reserve_tool_tokens.min(capacity / 2),
            used: 0,
        }
    }

    /// Tokens available for the prompt after reserves.
    #[must_use]
    pub fn free_for_prompt(&self) -> u32 {
        self.capacity
            .saturating_sub(self.reserve_assistant)
            .saturating_sub(self.reserve_tool)
    }

    /// Whether `tokens` fit into the prompt budget given current usage.
    #[must_use]
    pub fn fits(&self, tokens: u32) -> bool {
        self.used.saturating_add(tokens) <= self.free_for_prompt()
    }

    /// Record a message's token cost.
    pub fn add(&mut self, tokens: u32) {
        self.used = self.used.saturating_add(tokens);
    }

    /// Remove tokens (compaction).
    pub fn remove(&mut self, tokens: u32) {
        self.used = self.used.saturating_sub(tokens);
    }

    /// Whether compaction should trigger at the configured threshold.
    #[must_use]
    pub fn needs_compaction(&self, config: &ContextConfig) -> bool {
        self.capacity == 0
            || (self.used as f32) >= (self.capacity as f32) * config.compact_threshold
    }

    /// Fraction of capacity currently used (0.0..=1.0+).
    #[must_use]
    pub fn usage_ratio(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            self.used as f64 / self.capacity as f64
        }
    }

    /// Cache-hit percentage estimate from cached vs. prompt tokens.
    #[must_use]
    pub fn cache_hit_percent(cached_tokens: u64, prompt_tokens: u64) -> Option<f64> {
        if prompt_tokens == 0 {
            None
        } else {
            Some(cached_tokens as f64 / prompt_tokens as f64 * 100.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserves_cannot_exceed_half_capacity() {
        let config = ContextConfig::default();
        let budget = ContextBudget::new(1000, 9000, &config);
        assert!(budget.reserve_assistant <= 500);
        assert!(budget.reserve_tool <= 500);
    }

    #[test]
    fn free_for_prompt_never_underflows() {
        let config = ContextConfig::default();
        let budget = ContextBudget::new(100, 90, &config);
        assert_eq!(budget.free_for_prompt(), 0);
        assert!(!budget.fits(1));
    }

    #[test]
    fn fits_and_add_are_consistent() {
        let config = ContextConfig::default();
        let mut budget = ContextBudget::new(10_000, 1_000, &config);
        assert!(budget.fits(8_000));
        budget.add(8_000);
        assert!(!budget.fits(1_500));
        budget.remove(4_000);
        assert!(budget.fits(1_500));
    }

    #[test]
    fn compaction_threshold_respected() {
        let config = ContextConfig {
            compact_threshold: 0.8,
            ..Default::default()
        };
        let mut budget = ContextBudget::new(1_000, 100, &config);
        budget.add(799);
        assert!(!budget.needs_compaction(&config));
        budget.add(1);
        assert!(budget.needs_compaction(&config));
    }

    #[test]
    fn cache_hit_percent_handles_zero() {
        assert_eq!(ContextBudget::cache_hit_percent(50, 0), None);
        let percent = ContextBudget::cache_hit_percent(50, 100).expect("percent");
        assert!((percent - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn usage_ratio_is_bounded() {
        let config = ContextConfig::default();
        let mut budget = ContextBudget::new(100, 10, &config);
        assert_eq!(budget.usage_ratio(), 0.0);
        budget.add(150);
        assert!(budget.usage_ratio() > 1.0);
    }
}
