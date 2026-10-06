//! Repeated tool-call detection.
//!
//! A fingerprint normalizes a tool call (name + canonical JSON arguments).
//! The detector tolerates a configurable number of identical calls but stops
//! the turn when the same call repeats without any *different* call in
//! between (i.e. no progress), which is the unproductive loop we must not
//! allow to burn inference.

use std::collections::HashMap;

use serde_json::Value;

use crate::message::ToolCall;

/// A canonical fingerprint of a normalized tool call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallFingerprint {
    /// Tool name.
    pub name: String,
    /// Canonical (key-sorted) JSON of the arguments.
    pub normalized: String,
}

impl CallFingerprint {
    /// Build from a tool call. Arguments are canonically serialized so that
    /// key order and whitespace do not affect equality.
    pub fn of(call: &ToolCall) -> Self {
        let normalized = canonical_json(&call.arguments);
        Self {
            name: call.name.clone(),
            normalized,
        }
    }

    /// Human-readable form for diagnostics.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{}({})", self.name, truncate(&self.normalized, 80))
    }
}

/// Serialize a JSON value with object keys sorted (canonical form).
fn canonical_json(value: &Value) -> String {
    // serde_json's Map already orders keys when the "preserve_order" feature
    // is disabled (the default), so `to_string` is canonical. We still route
    // through `canonicalize` to be explicit and robust.
    canonicalize(value).to_string()
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, val) in map {
                out.insert(key.clone(), canonicalize(val));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let truncated: String = text.chars().take(max).collect();
        format!("{truncated}…")
    }
}

/// Verdict from observing a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopVerdict {
    /// The call is acceptable for now.
    Ok,
    /// The same call has repeated `count` times with no progress; stop.
    RepeatedLoop {
        fingerprint: CallFingerprint,
        count: u32,
    },
}

/// Detects unproductive identical tool-call loops.
#[derive(Debug)]
pub struct RepeatedCallDetector {
    limit: u32,
    counts: HashMap<CallFingerprint, u32>,
}

impl RepeatedCallDetector {
    /// Create with a repetition limit (1 = stop after the first repeat).
    pub fn new(limit: u32) -> Self {
        Self {
            limit: limit.max(1),
            counts: HashMap::new(),
        }
    }

    /// The configured limit.
    #[must_use]
    pub fn limit(&self) -> u32 {
        self.limit
    }

    /// Observe a call. Any *different* call since the last observation of
    /// this fingerprint counts as progress and resets its counter.
    pub fn observe(&mut self, fingerprint: &CallFingerprint) -> LoopVerdict {
        let is_new_fingerprint = !self.counts.contains_key(fingerprint);
        // Progress: anything other than an immediate repeat resets all
        // counters. A brand-new fingerprint is always progress.
        if is_new_fingerprint {
            self.counts.clear();
        }
        let count = self.counts.entry(fingerprint.clone()).or_insert(0);
        *count += 1;
        if *count > self.limit {
            LoopVerdict::RepeatedLoop {
                fingerprint: fingerprint.clone(),
                count: *count,
            }
        } else {
            LoopVerdict::Ok
        }
    }

    /// Reset all counters (e.g. after a successful compaction).
    pub fn reset(&mut self) {
        self.counts.clear();
    }
}

impl Default for RepeatedCallDetector {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, arg: &str) -> ToolCall {
        let arguments: Value = serde_json::from_str(arg).expect("json");
        ToolCall {
            id: "id".to_owned(),
            name: name.to_owned(),
            arguments,
        }
    }

    #[test]
    fn identical_calls_within_limit_are_ok() {
        let mut detector = RepeatedCallDetector::new(3);
        let fp = CallFingerprint::of(&call("find_in_workspace", r#"{"mode":"files"}"#));
        for _ in 0..3 {
            assert!(matches!(detector.observe(&fp), LoopVerdict::Ok));
        }
    }

    #[test]
    fn exceeding_limit_reports_repeated_loop() {
        let mut detector = RepeatedCallDetector::new(2);
        let fp = CallFingerprint::of(&call("inspect_files", r#"{"paths":["a.rs"]}"#));
        detector.observe(&fp);
        detector.observe(&fp);
        let verdict = detector.observe(&fp);
        match verdict {
            LoopVerdict::RepeatedLoop { count, .. } => assert_eq!(count, 3),
            other => panic!("expected repeated loop, got {other:?}"),
        }
    }

    #[test]
    fn a_different_call_is_progress_and_resets() {
        let mut detector = RepeatedCallDetector::new(1);
        let a = CallFingerprint::of(&call("inspect_files", r#"{"paths":["a.rs"]}"#));
        let b = CallFingerprint::of(&call("inspect_files", r#"{"paths":["b.rs"]}"#));
        // Limit 1: a, a would loop. But a, b, a should be fine.
        assert!(matches!(detector.observe(&a), LoopVerdict::Ok));
        assert!(matches!(detector.observe(&b), LoopVerdict::Ok));
        assert!(matches!(detector.observe(&a), LoopVerdict::Ok));
    }

    #[test]
    fn argument_key_order_does_not_change_fingerprint() {
        let a = CallFingerprint::of(&call(
            "change_files",
            r#"{"path":"x","expected_revision":"r1"}"#,
        ));
        let b = CallFingerprint::of(&call(
            "change_files",
            r#"{"expected_revision":"r1","path":"x"}"#,
        ));
        assert_eq!(a, b, "canonical JSON should ignore key order");
    }

    #[test]
    fn distinct_arguments_are_distinct() {
        let a = CallFingerprint::of(&call(
            "process",
            r#"{"action":"start","argv":["cargo","test"]}"#,
        ));
        let b = CallFingerprint::of(&call(
            "process",
            r#"{"action":"start","argv":["cargo","build"]}"#,
        ));
        assert_ne!(a, b);
    }

    #[test]
    fn describe_truncates_long_args() {
        let fp = CallFingerprint {
            name: "x".to_owned(),
            normalized: "0123456789".repeat(20),
        };
        let text = fp.describe();
        assert!(text.len() < 200, "{text}");
        assert!(text.contains('…'));
        assert!(text.starts_with("x("));
    }
}
