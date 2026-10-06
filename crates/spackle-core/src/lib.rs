//! spackle-core: agent state machine, domain events, messages, context
//! accounting, sessions, cancellation, repeated-call detection, runtime
//! prompt assembly, configuration, and transport/tool traits.
//!
//! This crate has no terminal, HTTP, or model-provider dependencies; the
//! TUI and llama.cpp client are built on it, not the other way around.

#![forbid(unsafe_code)]

pub mod agent;
pub mod cancel;
pub mod config;
pub mod context;
pub mod event;
pub mod loopdetect;
pub mod message;
pub mod prompt;
pub mod session;
pub mod state;
pub mod telemetry;

pub const PRODUCT_NAME: &str = "spackle";
pub const DEFAULT_MODEL: &str = "qwen3.8-27b-local";
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";

// Backward-compatible re-exports (the initial skeleton defined these here).
pub use event::{AgentEvent, EventKind};
pub use state::AgentState;
pub use telemetry::TurnTelemetry;
