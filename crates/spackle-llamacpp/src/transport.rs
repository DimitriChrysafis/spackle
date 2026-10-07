//! `InferenceTransport` implementation over the streaming Chat Completions
//! API.
//!
//! Responsibilities:
//! - map the core `InferenceRequest` onto the wire body;
//! - consume SSE safely under arbitrary byte fragmentation;
//! - accumulate text / reasoning / tool-call fragments into one
//!   `InferenceResult`;
//! - forward live `StreamEvent`s to the UI channel;
//! - honour the shared `CancellationToken` promptly, dropping the stream;
//! - classify failures (transient / fatal / capability) for the loop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;

use spackle_core::agent::{
    FinishReason, InferenceRequest, InferenceResult, InferenceTransport, StreamEvent, Timings,
    TransportError, Usage,
};
use spackle_core::cancel::CancellationToken;

use crate::client::LlamaCppClient;
use crate::error::ClientError;
use crate::sse::SseParser;
use crate::wire::{Chunk, TimingsWire, ToolCallAssembler, chat_request_body, usage_from_wire};

/// One inference step over HTTP.
#[derive(Debug, Clone)]
pub struct LlamaTransport {
    client: LlamaCppClient,
    model: String,
}

impl LlamaTransport {
    /// Wrap a policy-checked client with the model to request.
    #[must_use]
    pub fn new(client: LlamaCppClient, model: String) -> Self {
        Self { client, model }
    }

    /// The underlying client (for probes and model listing).
    #[must_use]
    pub fn client(&self) -> &LlamaCppClient {
        &self.client
    }

    /// The model id this transport requests by default (request-level
    /// `model` wins when the caller supplies one).
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
}

fn timings_from_wire(t: &TimingsWire) -> Timings {
    Timings {
        prompt_ms: t.prompt_ms.map(|v| v as u64),
        predicted_ms: t.predicted_ms.map(|v| v as u64),
        prompt_per_second: t.prompt_per_second,
        predicted_per_second: t.predicted_per_second,
    }
}

fn finish_reason_from_wire(reason: Option<&str>, has_tool_calls: bool) -> FinishReason {
    match reason {
        Some("length") => FinishReason::Length,
        Some("tool_calls") => FinishReason::ToolCalls,
        Some("content_filter") => FinishReason::ContentFilter,
        Some("error") => FinishReason::Error("server reported an error finish".to_owned()),
        Some("stop") | None if has_tool_calls => FinishReason::ToolCalls,
        Some("stop") | None => FinishReason::Stop,
        Some(_) => FinishReason::Stop,
    }
}

/// Accumulator for one inference step.
struct StepAssembler {
    content: String,
    reasoning: String,
    calls: ToolCallAssembler,
    usage: Option<Usage>,
    timings: Option<Timings>,
    finish_reason: Option<String>,
    started: Instant,
    first_token: Option<Instant>,
}

impl StepAssembler {
    fn new(request_seed: String) -> Self {
        Self {
            content: String::new(),
            reasoning: String::new(),
            calls: ToolCallAssembler::new(request_seed),
            usage: None,
            timings: None,
            finish_reason: None,
            started: Instant::now(),
            first_token: None,
        }
    }

    fn apply_chunk(
        &mut self,
        chunk: &Chunk,
        events: &tokio::sync::mpsc::UnboundedSender<StreamEvent>,
    ) {
        if let Some(usage) = &chunk.usage {
            let mapped = usage_from_wire(usage);
            self.usage = Some(mapped.clone());
            let _ = events.send(StreamEvent::Usage(mapped));
        }
        if let Some(timings) = &chunk.timings {
            self.timings = Some(timings_from_wire(timings));
            let _ = events.send(StreamEvent::Timings(
                self.timings.clone().unwrap_or_default(),
            ));
        }
        for choice in chunk.choices.iter().flatten() {
            let delta = &choice.delta;
            if let Some(fragment) = &delta.reasoning_content
                && !fragment.is_empty()
            {
                self.mark_first_token();
                let _ = events.send(StreamEvent::ReasoningDelta {
                    text: fragment.clone(),
                });
                self.reasoning.push_str(fragment);
            }
            if let Some(fragment) = &delta.content
                && !fragment.is_empty()
            {
                self.mark_first_token();
                let _ = events.send(StreamEvent::TextDelta {
                    text: fragment.clone(),
                });
                self.content.push_str(fragment);
            }
            if let Some(items) = &delta.tool_calls
                && !items.is_empty()
            {
                self.mark_first_token();
                for (position, item) in items.iter().enumerate() {
                    let index = item.index.map_or(position, |i| i as usize);
                    let arguments = item
                        .function
                        .as_ref()
                        .and_then(|function| function.arguments.clone())
                        .unwrap_or_default();
                    let _ = events.send(StreamEvent::ToolCallDelta {
                        index,
                        id: item.id.clone(),
                        name: item
                            .function
                            .as_ref()
                            .and_then(|function| function.name.clone()),
                        arguments,
                    });
                }
                self.calls.apply(delta);
            }
            if let Some(reason) = &choice.finish_reason
                && !reason.is_empty()
                && reason != "null"
            {
                self.finish_reason = Some(reason.clone());
            }
        }
    }

    fn mark_first_token(&mut self) {
        if self.first_token.is_none() {
            self.first_token = Some(self.started);
        }
    }

    fn into_result(self) -> InferenceResult {
        let tool_calls = self.calls.finish();
        InferenceResult {
            content: self.content,
            reasoning: self.reasoning,
            finish_reason: finish_reason_from_wire(
                self.finish_reason.as_deref(),
                !tool_calls.is_empty(),
            ),
            tool_calls,
            usage: self.usage.unwrap_or_default(),
            timings: self.timings.unwrap_or_default(),
            first_token_ms: self.first_token.map(|at| at.elapsed().as_millis() as u64),
        }
    }
}

#[async_trait]
impl InferenceTransport for LlamaTransport {
    async fn infer(
        &self,
        request: InferenceRequest,
        cancel: CancellationToken,
        events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<InferenceResult, TransportError> {
        let mut body = chat_request_body(
            &request.model,
            &request.system,
            &request.messages,
            &request.tools,
            &request.sampling,
            true,
            self.client.api(),
        );
        let seed = format!("{}-{}", std::process::id(), crate::next_request_counter());
        let mut assembler = StepAssembler::new(seed);
        // Non-reasoning models on generic OpenAI endpoints reject
        // `reasoning_effort` outright; degrade once instead of failing.
        let mut stream = match self.client.stream_chat(body.clone()).await {
            Ok(stream) => stream,
            Err(err) => {
                let transport: TransportError = err.into();
                match transport {
                    TransportError::Capability { ref feature, .. }
                        if feature.contains("reasoning_effort")
                            && body.get("reasoning_effort").is_some() =>
                    {
                        body.as_object_mut().map(|m| m.remove("reasoning_effort"));
                        self.client.stream_chat(body).await?
                    }
                    other => return Err(other),
                }
            }
        };
        let mut parser = SseParser::new();

        // Bridge the std condvar cancellation token into the async select.
        // The wait must live on the blocking pool (a regular task would
        // park a current-thread executor) and must exit when the stream
        // ends even without a cancel: a parked condvar waiter would
        // otherwise stall runtime shutdown, which waits on the pool.
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let finished = Arc::new(AtomicBool::new(false));
        let cancel_handle = {
            let token = cancel.clone();
            let finished = finished.clone();
            tokio::task::spawn_blocking(move || {
                while !finished.load(Ordering::Acquire) {
                    if token.wait_timeout(Duration::from_millis(25)) {
                        let _ = cancel_tx.send(());
                        return;
                    }
                }
            })
        };
        struct MarkFinished(Arc<AtomicBool>);
        impl Drop for MarkFinished {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let _finished_guard = MarkFinished(finished);

        'stream: loop {
            tokio::select! {
                biased;
                _ = &mut cancel_rx => {
                    cancel_handle.abort();
                    return Err(ClientError::Cancelled.into());
                }
                next = stream.next() => match next {
                    Some(Ok(bytes)) => {
                        if cancel.is_cancelled() {
                            cancel_handle.abort();
                            return Err(ClientError::Cancelled.into());
                        }
                        for payload in parser.feed(&bytes).map_err(ClientError::Sse)? {
                            if payload == "[DONE]" {
                                break 'stream;
                            }
                            let chunk = match serde_json::from_str::<Chunk>(&payload) {
                                Ok(chunk) => chunk,
                                Err(err) => {
                                    cancel_handle.abort();
                                    return Err(ClientError::Malformed(format!(
                                        "unparseable SSE payload `{payload}`: {err}"
                                    ))
                                    .into());
                                }
                            };
                            if let Some(stream_error) = &chunk.error {
                                cancel_handle.abort();
                                return Err(TransportError::Fatal(
                                    stream_error
                                        .message
                                        .clone()
                                        .unwrap_or_else(|| "stream error from server".to_owned()),
                                ));
                            }
                            assembler.apply_chunk(&chunk, &events);
                        }
                    }
                    Some(Err(err)) => {
                        cancel_handle.abort();
                        return Err(err.into());
                    }
                    None => break,
                },
            }
        }

        // Flush a final event that lacked a trailing blank line.
        if let Some(payload) = parser.finish()
            && payload != "[DONE]"
        {
            match serde_json::from_str::<Chunk>(&payload) {
                Ok(chunk) => {
                    if let Some(stream_error) = &chunk.error {
                        let message = stream_error
                            .message
                            .clone()
                            .unwrap_or_else(|| "stream error from server".to_owned());
                        cancel_handle.abort();
                        return Err(TransportError::Fatal(message));
                    }
                    assembler.apply_chunk(&chunk, &events);
                }
                Err(err) => {
                    // An unterminated trailing payload means the connection
                    // was cut mid-chunk — retryable, unlike a complete SSE
                    // event with bad JSON (a real protocol violation).
                    cancel_handle.abort();
                    return Err(TransportError::Transient(format!(
                        "stream ended mid-payload: {err}"
                    )));
                }
            }
        }

        cancel_handle.abort();
        Ok(assembler.into_result())
    }
}
