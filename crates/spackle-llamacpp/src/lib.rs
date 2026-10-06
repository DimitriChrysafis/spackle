//! `spackle-llamacpp` — the wire adapter for llama.cpp servers.
//!
//! This crate maps the wire-agnostic core (`spackle_core`) onto the exact
//! HTTP surface a local `llama-server` exposes:
//!
//! - byte-safe incremental SSE parsing (`sse`),
//! - llama.cpp 2.28 Chat Completions request/chunk shape (`wire`),
//! - a policy-checked endpoint client (`client`),
//! - the `InferenceTransport` implementation (`transport`).
//!
//! Endpoint discipline (inherited from the project rules): loopback by
//! default, private LAN only with an explicit flag, public endpoints
//! always rejected. No telemetry, no auto-update, offline first.

#![forbid(unsafe_code)]

pub mod client;
pub mod error;
pub mod sse;
pub mod transport;
pub mod wire;

pub use client::{EndpointProbe, LlamaCppClient, ModelInfo, PinBoxStream, ProbeReport};
pub use error::ClientError;
pub use sse::{SseError, SseParser};
pub use transport::LlamaTransport;
pub use wire::{
    Chunk, Delta, DeltaFunction, DeltaToolCall, StreamError, TimingsWire, TokenDetails,
    ToolCallAssembler, UsageWire, chat_request_body, reasoning_effort_wire,
};

use std::sync::atomic::{AtomicU64, Ordering};

/// Per-process request counter used to make fallback tool-call ids stable
/// within one request.
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Next request sequence number.
#[must_use]
pub fn next_request_counter() -> u64 {
    REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}
