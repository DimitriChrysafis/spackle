//! Agent state machine and loop.
//!
//! Explicit states, no recursion:
//!
//! ```text
//! Idle -> Preparing -> Inferring -> ExecutingTools -> Inferring -> Completed
//!                                      -> AwaitingApproval
//! Any state -> Cancelling -> Idle/Error
//! ```
//!
//! The loop appends messages to the transcript exactly in wire order,
//! preserves assistant tool-call messages verbatim, bounds tool output,
//! detects unproductive repeated-call loops, and honours the shared
//! cancellation token.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::cancel::CancellationToken;
use crate::config::agent::AgentConfig;
use crate::config::generation::{GenerationProfile, ReasoningEffort};
use crate::event::AgentEvent;
use crate::loopdetect::{CallFingerprint, LoopVerdict, RepeatedCallDetector};
use crate::message::{Message, ToolCall};
use crate::state::AgentState;

/// Sampling parameters for one inference step (from a generation profile).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SamplingParams {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
    pub presence_penalty: f32,
    pub repeat_penalty: f32,
    pub max_tokens: u32,
    /// Qwen `enable_thinking` (chat template keyword).
    pub enable_thinking: Option<bool>,
    /// Qwen `preserve_thinking` (chat template keyword).
    pub preserve_thinking: Option<bool>,
    /// Qwen `reasoning_effort` (request level).
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl From<&GenerationProfile> for SamplingParams {
    fn from(profile: &GenerationProfile) -> Self {
        Self {
            temperature: profile.temperature,
            top_p: profile.top_p,
            top_k: profile.top_k,
            min_p: profile.min_p,
            presence_penalty: profile.presence_penalty,
            repeat_penalty: profile.repeat_penalty,
            max_tokens: profile.max_tokens,
            enable_thinking: profile.enable_thinking,
            preserve_thinking: profile.preserve_thinking,
            reasoning_effort: profile.reasoning_effort,
        }
    }
}

/// A tool advertised to the model (compact, batch-oriented schemas).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema object for the arguments.
    pub parameters: serde_json::Value,
}

/// One streaming delta from the model. The transcript is built from the
/// final [`InferenceResult`]; deltas feed the live UI only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Assistant text delta.
    TextDelta { text: String },
    /// Reasoning delta (hidden unless display is enabled).
    ReasoningDelta { text: String },
    /// Incremental tool-call data.
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        /// Partial JSON argument fragment.
        arguments: String,
    },
    /// Usage information (authoritative when present).
    Usage(Usage),
    /// Server timings (authoritative).
    Timings(Timings),
}

/// Token usage as reported by the server.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Usage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
}

/// Server-reported timings (`timings` object).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Timings {
    pub prompt_ms: Option<u64>,
    pub predicted_ms: Option<u64>,
    /// Authoritative prompt (prefill) throughput.
    pub prompt_per_second: Option<f64>,
    /// Authoritative decode throughput.
    pub predicted_per_second: Option<f64>,
}

/// Why the model stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    /// Server-side error surfaced in the stream.
    Error(String),
}

impl FinishReason {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stop => "stop",
            Self::ToolCalls => "tool_calls",
            Self::Length => "length",
            Self::ContentFilter => "content_filter",
            Self::Error(_) => "error",
        }
    }
}

/// The complete result of one inference step.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// Final assistant text.
    pub content: String,
    /// Final reasoning content (empty when the model produced none).
    pub reasoning: String,
    /// Tool calls in wire order, with stable ids.
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
    pub usage: Usage,
    pub timings: Timings,
    /// Time to first streamed token, when measurable.
    pub first_token_ms: Option<u64>,
}

/// A full inference request (wire mapping happens in spackle-llamacpp).
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub model: String,
    /// Rendered runtime system prompt (byte-stable).
    pub system: String,
    /// Conversation so far (user/assistant/tool), excluding the system
    /// message.
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub sampling: SamplingParams,
}

/// Transport failures. `Transient` may be retried within the step budget;
/// `Capability` must produce an actionable diagnostic; `Fatal` stops the
/// turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Retryable (connection reset, HTTP 5xx, slot busy).
    Transient(String),
    /// Non-retryable.
    Fatal(String),
    /// The server lacks a feature this profile relies on.
    Capability { feature: String, hint: String },
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transient(message) => write!(f, "transient model error: {message}"),
            Self::Fatal(message) => write!(f, "model error: {message}"),
            Self::Capability { feature, hint } => {
                write!(f, "server does not support {feature}: {hint}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// Inference transport (implemented by spackle-llamacpp for HTTP; faked in
/// tests).
#[async_trait]
pub trait InferenceTransport: Send + Sync {
    /// Run one inference step. Stream deltas on `events`; return the
    /// complete result. Must respect `cancel` by dropping in-flight work.
    async fn infer(
        &self,
        request: InferenceRequest,
        cancel: CancellationToken,
        events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<InferenceResult, TransportError>;
}

/// Output of a tool execution.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Bounded, human/model-readable output.
    pub text: String,
    /// True when the tool itself reported failure (recorded as an error
    /// result, not a loop abort).
    pub is_error: bool,
}

/// Tool failures. `Cancelled` aborts the turn; the others are recorded in
/// the transcript so the model can adapt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    /// Recorded as a tool error result; the loop continues.
    Failed(String),
    /// Refused by policy; recorded as a denial result; the loop continues.
    Denied(String),
    /// Cancellation requested; the turn is aborted.
    Cancelled,
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(message) => f.write_str(message),
            Self::Denied(message) => write!(f, "denied: {message}"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

/// One tool implementation (implemented by spackle-tools).
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Tool name as advertised in the schema.
    fn name(&self) -> &str;

    /// True for independent, side-effect-free calls that may run
    /// concurrently. Mutating tools must return false (serialized).
    fn read_only(&self) -> bool {
        false
    }

    /// True when the approval gate must be consulted before execution.
    /// Receives the call so tools can gate selectively (e.g. reads only
    /// when the target looks like a secret file).
    fn requires_approval(&self, _call: &ToolCall) -> bool {
        false
    }

    /// Human-readable description of the pending action (approval detail).
    fn describe(&self, call: &ToolCall) -> String {
        format!("{} with {}", self.name(), call.arguments)
    }

    /// Execute the call. Must respect `cancel`.
    async fn execute(
        &self,
        call: &ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolOutput, ToolError>;
}

/// Named lookup for tool executors.
#[derive(Default)]
pub struct ToolRegistry {
    executors: Vec<Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an executor (last registration wins for a name).
    pub fn register(&mut self, executor: Box<dyn ToolExecutor>) {
        self.executors
            .retain(|existing| existing.name() != executor.name());
        self.executors.push(executor);
    }

    /// Look up by tool name.
    pub fn get(&self, name: &str) -> Option<&dyn ToolExecutor> {
        self.executors.iter().find_map(|executor| {
            let executor = executor.as_ref();
            (executor.name() == name).then_some(executor)
        })
    }

    /// All registered names, sorted (stable schema ordering).
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .executors
            .iter()
            .map(|executor| executor.name().to_owned())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.names())
            .finish()
    }
}

/// Approval decision for a tool that requires approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny,
}

/// Approval gate (TUI confirms interactively; headless denies by default).
pub trait ApprovalGate: Send + Sync {
    fn decide(&self, tool: &str, detail: &str) -> ApprovalDecision;
}

/// Deny every approval request (safe headless default).
pub struct DenyAllGate;

impl ApprovalGate for DenyAllGate {
    fn decide(&self, _tool: &str, _detail: &str) -> ApprovalDecision {
        ApprovalDecision::Deny
    }
}

/// Allow every approval request (only for tests and explicit opt-in).
pub struct AllowAllGate;

impl ApprovalGate for AllowAllGate {
    fn decide(&self, _tool: &str, _detail: &str) -> ApprovalDecision {
        ApprovalDecision::Allow
    }
}

/// Loop limits.
#[derive(Debug, Clone, Copy)]
pub struct LoopConfig {
    pub max_steps: u32,
    pub repeated_call_limit: u32,
    pub max_tool_output_bytes: u64,
    pub max_model_retries: u32,
    /// 0 disables the wall-clock bound.
    pub max_turn_seconds: u64,
    /// Maximum concurrent read-only tool executions.
    pub tool_concurrency: usize,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_steps: 24,
            repeated_call_limit: 3,
            max_tool_output_bytes: 128 * 1024,
            max_model_retries: 1,
            max_turn_seconds: 0,
            tool_concurrency: 4,
        }
    }
}

impl From<&AgentConfig> for LoopConfig {
    fn from(config: &AgentConfig) -> Self {
        Self {
            max_steps: config.max_steps,
            repeated_call_limit: config.repeated_call_limit,
            max_tool_output_bytes: config.max_tool_output_bytes,
            max_model_retries: config.max_model_retries,
            max_turn_seconds: config.max_turn_seconds,
            tool_concurrency: 4,
        }
    }
}

/// Everything the loop needs for one turn.
pub struct TurnContext<'a> {
    pub transport: &'a dyn InferenceTransport,
    pub tools: &'a ToolRegistry,
    pub gate: &'a dyn ApprovalGate,
    pub model: String,
    pub system_prompt: String,
    pub tools_schema: Vec<ToolDefinition>,
    pub sampling: SamplingParams,
    pub config: LoopConfig,
}

/// Successful turn outcome.
#[derive(Debug, Clone)]
pub struct TurnOutcome {
    pub final_text: String,
    pub steps: u32,
    /// Deterministic event sequence for the journal.
    pub events: Vec<AgentEvent>,
}

/// Turn failure with the events recorded so far (for the journal).
#[derive(Debug, Clone)]
pub struct AgentError {
    pub state: AgentState,
    pub reason: String,
    pub events: Vec<AgentEvent>,
    /// Transcript state at failure (already appended messages).
    pub transcript: Vec<Message>,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.state, self.reason)
    }
}

impl std::error::Error for AgentError {}

fn bounded_output(text: String, limit_bytes: u64) -> String {
    let bytes = text.as_bytes();
    if bytes.len() as u64 <= limit_bytes {
        return text;
    }
    // Cut on a char boundary, then label the truncation so the model knows
    // output was elided (never silently).
    let mut end = limit_bytes as usize;
    while end > 0 && !bytes[end].is_ascii() {
        end -= 1;
    }
    let mut head = String::from_utf8_lossy(&bytes[..end]).into_owned();
    head.push_str("\n…[output truncated by spackle; re-run with a narrower range or filters]");
    head
}

fn agent_error(
    state: AgentState,
    reason: String,
    events: Vec<AgentEvent>,
    transcript: &[Message],
) -> AgentError {
    AgentError {
        state,
        reason,
        events,
        transcript: transcript.to_vec(),
    }
}

/// Run one turn to completion, failure, or cancellation.
///
/// `transcript` holds prior conversation (excluding the system prompt); the
/// loop appends the user message, assistant steps, and tool results in wire
/// order, and hands the updated transcript back on both success and error.
pub async fn run_turn(
    context: &TurnContext<'_>,
    session_id: &str,
    transcript: &mut Vec<Message>,
    user_message: Message,
    cancel: CancellationToken,
    stream_events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
) -> Result<TurnOutcome, AgentError> {
    let mut events: Vec<AgentEvent> = Vec::new();
    let mut detector = RepeatedCallDetector::new(context.config.repeated_call_limit);
    let mut retries_left = context.config.max_model_retries;
    let turn_started_at = Instant::now();
    let deadline = (context.config.max_turn_seconds > 0)
        .then(|| turn_started_at + Duration::from_secs(context.config.max_turn_seconds));

    events.push(AgentEvent::turn_started(session_id, 1));
    transcript.push(user_message);

    let mut step: u32 = 0;
    loop {
        if cancel.is_cancelled() {
            events.push(AgentEvent::turn_cancelled(session_id, step));
            return Err(agent_error(
                AgentState::Cancelling,
                "cancelled by user".to_owned(),
                events,
                transcript,
            ));
        }
        if let Some(limit) = deadline
            && Instant::now() >= limit
        {
            events.push(AgentEvent::turn_failed(
                session_id,
                "turn exceeded its time budget",
            ));
            return Err(agent_error(
                AgentState::Error,
                "turn exceeded its time budget".to_owned(),
                events,
                transcript,
            ));
        }
        if step >= context.config.max_steps {
            let reason = format!(
                "stopped after {} steps (max_steps)",
                context.config.max_steps
            );
            events.push(AgentEvent::turn_failed(session_id, &reason));
            return Err(agent_error(AgentState::Error, reason, events, transcript));
        }

        // --- Inferring -----------------------------------------------------
        events.push(AgentEvent::inference_started(step));
        let request = InferenceRequest {
            model: context.model.clone(),
            system: context.system_prompt.clone(),
            messages: transcript.clone(),
            tools: context.tools_schema.clone(),
            sampling: context.sampling.clone(),
        };
        let result = loop {
            match context
                .transport
                .infer(request.clone(), cancel.clone(), stream_events.clone())
                .await
            {
                Ok(result) => break result,
                Err(TransportError::Transient(message)) if retries_left > 0 => {
                    retries_left -= 1;
                    tracing::warn!(error = %message, "transient inference error; retrying");
                    continue;
                }
                Err(error) => {
                    if cancel.is_cancelled() {
                        let reason = error.to_string();
                        events.push(AgentEvent::turn_cancelled(session_id, step));
                        return Err(agent_error(
                            AgentState::Cancelling,
                            reason,
                            events,
                            transcript,
                        ));
                    }
                    let reason = error.to_string();
                    events.push(AgentEvent::turn_failed(session_id, &reason));
                    return Err(agent_error(AgentState::Error, reason, events, transcript));
                }
            }
        };
        let tool_calls = result.tool_calls.clone();
        let assistant = Message::assistant(
            result.content.clone(),
            (!result.reasoning.is_empty()).then(|| result.reasoning.clone()),
            tool_calls.clone(),
        );
        transcript.push(assistant);
        events.push(AgentEvent::inference_finished(
            step,
            result.finish_reason.as_str(),
        ));
        if let Some(first_token_ms) = result.first_token_ms {
            let usage = result.usage.clone();
            events.push(AgentEvent::telemetry(serde_json::json!({
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "cached_tokens": usage.cached_tokens,
                "reasoning_tokens": usage.reasoning_tokens,
                "first_token_ms": first_token_ms,
                "prompt_per_second": result.timings.prompt_per_second,
                "predicted_per_second": result.timings.predicted_per_second,
            })));
        }

        if tool_calls.is_empty() {
            events.push(AgentEvent::turn_completed(session_id, step + 1));
            return Ok(TurnOutcome {
                final_text: result.content,
                steps: step + 1,
                events,
            });
        }

        // --- ExecutingTools -------------------------------------------------
        let started = Instant::now();
        let per_call = match execute_tool_calls(
            context,
            &tool_calls,
            &mut detector,
            &mut events,
            &cancel,
            started,
        )
        .await
        {
            Ok(per_call) => per_call,
            Err(reason) => {
                let state = if cancel.is_cancelled() {
                    AgentState::Cancelling
                } else {
                    AgentState::Error
                };
                let event = if cancel.is_cancelled() {
                    AgentEvent::turn_cancelled(session_id, step)
                } else {
                    AgentEvent::turn_failed(session_id, &reason)
                };
                events.push(event);
                return Err(agent_error(state, reason, events, transcript));
            }
        };
        for (call, (text, is_error)) in tool_calls.iter().zip(per_call) {
            let bounded = bounded_output(text, context.config.max_tool_output_bytes);
            transcript.push(Message::tool_result(
                call.id.clone(),
                call.name.clone(),
                bounded,
                is_error,
            ));
        }
        step += 1;
    }
}

/// Execute a batch of tool calls: read-only calls concurrently (bounded),
/// mutating calls serialized. Returns per-call results in call order, or
/// `Err(reason)` for cancellation/loop abort.
///
/// Journal events are emitted per call *in call order* after the batch
/// completes, keeping the recorded event sequence deterministic even when
/// concurrent calls finish in different orders.
async fn execute_tool_calls(
    context: &TurnContext<'_>,
    tool_calls: &[ToolCall],
    detector: &mut RepeatedCallDetector,
    events: &mut Vec<AgentEvent>,
    cancel: &CancellationToken,
    started: Instant,
) -> std::result::Result<Vec<(String, bool)>, String> {
    let mut results: Vec<Option<(String, bool)>> = vec![None; tool_calls.len()];

    // Validate the whole batch against the loop detector first: a repeated
    // loop aborts before any side effects.
    for call in tool_calls {
        let fingerprint = CallFingerprint::of(call);
        match detector.observe(&fingerprint) {
            LoopVerdict::Ok => {}
            LoopVerdict::RepeatedLoop { fingerprint, count } => {
                return Err(format!(
                    "repeated identical tool call detected ({} times): {}; stopping to avoid an unproductive loop",
                    count,
                    fingerprint.describe()
                ));
            }
        }
    }

    let execute_one = |index: usize| {
        let call = tool_calls[index].clone();
        let executor = context.tools.get(&call.name);
        let cancel = cancel.clone();
        async move {
            let (text, is_error) = match executor {
                None => (format!("unknown tool `{}`", call.name), true),
                Some(executor) => {
                    if executor.requires_approval(&call) {
                        let detail = executor.describe(&call);
                        let decision = context.gate.decide(&call.name, &detail);
                        match decision {
                            ApprovalDecision::Allow => {}
                            ApprovalDecision::Deny => {
                                return (
                                    index,
                                    format!(
                                        "denied by approval policy: {detail}. Do not retry this exact action; choose a different approach or ask the user."
                                    ),
                                    true,
                                );
                            }
                        }
                    }
                    match executor.execute(&call, cancel).await {
                        Ok(output) => (output.text, output.is_error),
                        Err(ToolError::Failed(message)) => (message, true),
                        Err(ToolError::Denied(message)) => (message, true),
                        Err(ToolError::Cancelled) => ("cancelled by user".to_owned(), true),
                    }
                }
            };
            (index, text, is_error)
        }
    };

    let mut readonly_indices: Vec<usize> = Vec::new();
    let mut mutating_indices: Vec<usize> = Vec::new();
    for (index, call) in tool_calls.iter().enumerate() {
        let executor = context.tools.get(&call.name);
        if executor.is_some_and(|executor| executor.read_only()) {
            readonly_indices.push(index);
        } else {
            mutating_indices.push(index);
        }
    }

    // Read-only calls run concurrently with bounded overlap.
    {
        let mut wave: futures_util::stream::FuturesUnordered<_> =
            futures_util::stream::FuturesUnordered::new();
        let mut pending: std::collections::VecDeque<usize> =
            readonly_indices.iter().copied().collect();
        while let Some(index) = pending.pop_front() {
            if wave.len() >= context.config.tool_concurrency.max(1)
                && let Some(finished) = wave.next().await
            {
                let (idx, text, is_error) = finished;
                results[idx] = Some((text, is_error));
                if cancel.is_cancelled() {
                    return Err("cancelled during tool execution".to_owned());
                }
            }
            wave.push(execute_one(index));
        }
        while let Some((idx, text, is_error)) = wave.next().await {
            results[idx] = Some((text, is_error));
            if cancel.is_cancelled() {
                return Err("cancelled during tool execution".to_owned());
            }
        }
    }

    // Mutating calls are serialized, in order.
    for index in mutating_indices {
        if cancel.is_cancelled() {
            return Err("cancelled during tool execution".to_owned());
        }
        let (index, text, is_error) = execute_one(index).await;
        results[index] = Some((text, is_error));
    }

    let mut finished = Vec::new();
    for (index, slot) in results.iter().enumerate() {
        if let Some((text, is_error)) = slot {
            events.push(AgentEvent::tool_call_started(
                &tool_calls[index].id,
                &tool_calls[index].name,
            ));
            events.push(AgentEvent::tool_call_finished(
                &tool_calls[index].id,
                &tool_calls[index].name,
                *is_error,
                started.elapsed().as_millis() as u64,
            ));
            finished.push((text.clone(), *is_error));
        }
    }
    Ok(finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventKind;

    struct FixedText {
        text: String,
    }

    #[async_trait]
    impl InferenceTransport for FixedText {
        async fn infer(
            &self,
            _request: InferenceRequest,
            cancel: CancellationToken,
            events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
        ) -> Result<InferenceResult, TransportError> {
            if cancel.is_cancelled() {
                return Err(TransportError::Fatal("cancelled".to_owned()));
            }
            let _ = events.send(StreamEvent::TextDelta {
                text: self.text.clone(),
            });
            Ok(InferenceResult {
                content: self.text.clone(),
                reasoning: String::new(),
                tool_calls: Vec::new(),
                finish_reason: FinishReason::Stop,
                usage: Usage {
                    prompt_tokens: Some(10),
                    completion_tokens: Some(5),
                    cached_tokens: Some(8),
                    reasoning_tokens: None,
                },
                timings: Timings {
                    prompt_ms: Some(50),
                    predicted_ms: Some(100),
                    prompt_per_second: Some(200.0),
                    predicted_per_second: Some(50.0),
                },
                first_token_ms: Some(30),
            })
        }
    }

    struct Scripted {
        steps: std::sync::Mutex<std::vec::Vec<InferenceResult>>,
        requests: std::sync::Mutex<std::vec::Vec<InferenceRequest>>,
    }

    impl Scripted {
        fn new(results: Vec<InferenceResult>) -> Self {
            Self {
                steps: std::sync::Mutex::new(results),
                requests: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<InferenceRequest> {
            self.requests.lock().expect("lock").clone()
        }
    }

    #[async_trait]
    impl InferenceTransport for Scripted {
        async fn infer(
            &self,
            request: InferenceRequest,
            _cancel: CancellationToken,
            _events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
        ) -> Result<InferenceResult, TransportError> {
            let mut steps = self.steps.lock().expect("lock");
            let result = match steps.first().cloned() {
                Some(result) => {
                    steps.remove(0);
                    result
                }
                None => {
                    drop(steps);
                    self.requests.lock().expect("lock").push(request);
                    return Err(TransportError::Fatal(
                        "scripted transport exhausted: the test scripted fewer model turns than the loop requested".to_owned(),
                    ));
                }
            };
            drop(steps);
            self.requests.lock().expect("lock").push(request);
            Ok(result)
        }
    }

    fn result_stop(text: &str) -> InferenceResult {
        InferenceResult {
            content: text.to_owned(),
            reasoning: String::new(),
            tool_calls: Vec::new(),
            finish_reason: FinishReason::Stop,
            usage: Usage::default(),
            timings: Timings::default(),
            first_token_ms: None,
        }
    }

    fn result_tools(calls: Vec<ToolCall>) -> InferenceResult {
        InferenceResult {
            content: String::new(),
            reasoning: String::new(),
            tool_calls: calls,
            finish_reason: FinishReason::ToolCalls,
            usage: Usage::default(),
            timings: Timings::default(),
            first_token_ms: None,
        }
    }

    fn call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: format!("call_{name}"),
            name: name.to_owned(),
            arguments: args,
        }
    }

    struct EchoTool {
        name: &'static str,
        read_only: bool,
    }

    #[async_trait]
    impl ToolExecutor for EchoTool {
        fn name(&self) -> &str {
            self.name
        }
        fn read_only(&self) -> bool {
            self.read_only
        }
        async fn execute(
            &self,
            call: &ToolCall,
            cancel: CancellationToken,
        ) -> Result<ToolOutput, ToolError> {
            if cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            Ok(ToolOutput {
                text: format!("{}:{}", self.name, call.arguments),
                is_error: false,
            })
        }
    }

    struct FailingTool;

    #[async_trait]
    impl ToolExecutor for FailingTool {
        fn name(&self) -> &str {
            "failing"
        }
        async fn execute(
            &self,
            _call: &ToolCall,
            _cancel: CancellationToken,
        ) -> Result<ToolOutput, ToolError> {
            Err(ToolError::Failed("boom".to_owned()))
        }
    }

    struct ApprovingTool;

    #[async_trait]
    impl ToolExecutor for ApprovingTool {
        fn name(&self) -> &str {
            "approving"
        }
        fn requires_approval(&self, _call: &ToolCall) -> bool {
            true
        }
        async fn execute(
            &self,
            call: &ToolCall,
            _cancel: CancellationToken,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput {
                text: format!("ran {}", call.arguments),
                is_error: false,
            })
        }
    }

    struct AlwaysAllow;

    impl ApprovalGate for AlwaysAllow {
        fn decide(&self, _tool: &str, _detail: &str) -> ApprovalDecision {
            ApprovalDecision::Allow
        }
    }

    /// A stream-event sink for tests: the receiver is dropped immediately
    /// so sends become no-ops.
    fn sink() -> (
        tokio::sync::mpsc::UnboundedSender<StreamEvent>,
        tokio::sync::mpsc::UnboundedReceiver<StreamEvent>,
    ) {
        tokio::sync::mpsc::unbounded_channel()
    }

    fn context<'a>(
        transport: &'a dyn InferenceTransport,
        tools: &'a ToolRegistry,
        gate: &'a dyn ApprovalGate,
        config: LoopConfig,
    ) -> TurnContext<'a> {
        TurnContext {
            transport,
            tools,
            gate,
            model: "test-model".to_owned(),
            system_prompt: "system".to_owned(),
            tools_schema: Vec::new(),
            sampling: SamplingParams {
                temperature: 1.0,
                top_p: 1.0,
                top_k: 40,
                min_p: 0.0,
                presence_penalty: 0.0,
                repeat_penalty: 1.0,
                max_tokens: 1024,
                enable_thinking: None,
                preserve_thinking: None,
                reasoning_effort: None,
            },
            config,
        }
    }

    #[tokio::test]
    async fn single_step_text_turn_completes_with_deterministic_events() {
        let transport = FixedText {
            text: "Done.".to_owned(),
        };
        let registry = ToolRegistry::new();
        let gate = DenyAllGate;
        let ctx = context(&transport, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let outcome = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("hello"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("turn completes");
        assert_eq!(outcome.final_text, "Done.");
        assert_eq!(outcome.steps, 1);
        let kinds: Vec<EventKind> = outcome.events.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EventKind::TurnStarted,
                EventKind::InferenceStarted,
                EventKind::InferenceFinished,
                EventKind::Telemetry,
                EventKind::TurnCompleted,
            ]
        );
        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript[1].text(), "Done.");
    }

    #[tokio::test]
    async fn tool_loop_appends_results_and_completes() {
        let scripted = Scripted::new(vec![
            result_tools(vec![call("echo", serde_json::json!({"v": 1}))]),
            result_stop("All good."),
        ]);
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool {
            name: "echo",
            read_only: true,
        }));
        let gate = DenyAllGate;
        let ctx = context(&scripted, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let outcome = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("use the tool"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("completes");
        assert_eq!(outcome.steps, 2);
        assert_eq!(
            transcript.len(),
            4,
            "user, assistant+tool, result, assistant"
        );
        let assistant = &transcript[1];
        assert_eq!(assistant.tool_calls()[0].name, "echo");
        assert_eq!(
            assistant.tool_calls()[0].arguments,
            serde_json::json!({"v": 1})
        );
        let result_msg = &transcript[2];
        assert_eq!(result_msg.role, crate::message::Role::Tool);
        assert!(result_msg.text().contains("echo"));
        let requests = scripted.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].messages.len(), 3);
    }

    #[tokio::test]
    async fn repeated_identical_calls_stop_the_turn() {
        // limit = 2: two identical calls are tolerated, the third is blocked
        // before execution.
        let same = result_tools(vec![call("echo", serde_json::json!({"v": 1}))]);
        let scripted = Scripted::new(vec![same.clone(), same.clone(), same]);
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool {
            name: "echo",
            read_only: true,
        }));
        let gate = DenyAllGate;
        let config = LoopConfig {
            repeated_call_limit: 2,
            ..Default::default()
        };
        let ctx = context(&scripted, &registry, &gate, config);
        let mut transcript: Vec<Message> = Vec::new();
        let error = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("loop"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect_err("loop must stop");
        assert!(error.reason.contains("repeated identical tool call"));
        assert!(error.reason.contains("echo"));
        assert!(
            error
                .events
                .iter()
                .any(|event| event.kind == EventKind::TurnFailed)
        );
    }

    #[tokio::test]
    async fn max_steps_bounds_the_turn() {
        let scripted = Scripted::new(vec![
            result_tools(vec![call("echo", serde_json::json!({"v": 1}))]),
            result_tools(vec![call("echo2", serde_json::json!({"v": 2}))]),
            result_tools(vec![call("echo3", serde_json::json!({"v": 3}))]),
        ]);
        let mut registry = ToolRegistry::new();
        for name in ["echo", "echo2", "echo3"] {
            registry.register(Box::new(EchoTool {
                name,
                read_only: true,
            }));
        }
        let gate = DenyAllGate;
        let config = LoopConfig {
            max_steps: 2,
            ..Default::default()
        };
        let ctx = context(&scripted, &registry, &gate, config);
        let mut transcript: Vec<Message> = Vec::new();
        let error = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("run three"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect_err("max steps");
        assert!(error.reason.contains("max_steps"), "{}", error.reason);
        assert_eq!(scripted.requests().len(), 2, "exactly max_steps requests");
    }

    #[tokio::test]
    async fn transient_errors_are_retried_within_budget() {
        struct Flaky {
            calls: std::sync::atomic::AtomicUsize,
        }
        #[async_trait]
        impl InferenceTransport for Flaky {
            async fn infer(
                &self,
                _request: InferenceRequest,
                _cancel: CancellationToken,
                _events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
            ) -> Result<InferenceResult, TransportError> {
                let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n == 0 {
                    Err(TransportError::Transient("connection reset".to_owned()))
                } else {
                    Ok(result_stop("recovered"))
                }
            }
        }
        let transport = Flaky {
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let registry = ToolRegistry::new();
        let gate = DenyAllGate;
        let ctx = context(&transport, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let outcome = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("flaky"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("retries then completes");
        assert_eq!(outcome.final_text, "recovered");
        assert_eq!(transport.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn capability_errors_are_fatal_with_diagnostics() {
        struct Capability;
        #[async_trait]
        impl InferenceTransport for Capability {
            async fn infer(
                &self,
                _request: InferenceRequest,
                _cancel: CancellationToken,
                _events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
            ) -> Result<InferenceResult, TransportError> {
                Err(TransportError::Capability {
                    feature: "reasoning_effort".to_owned(),
                    hint: "this model does not support reasoning_effort; use a Qwen3.x profile or clear the field"
                        .to_owned(),
                })
            }
        }
        let transport = Capability;
        let registry = ToolRegistry::new();
        let gate = DenyAllGate;
        let ctx = context(&transport, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let error = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("x"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect_err("capability must fail");
        assert!(
            error.reason.contains("reasoning_effort"),
            "{}",
            error.reason
        );
        assert!(error.reason.contains("support"));
    }

    #[tokio::test]
    async fn unknown_tool_is_recorded_and_loop_continues() {
        let scripted = Scripted::new(vec![
            result_tools(vec![call("missing", serde_json::json!({}))]),
            result_stop("ok"),
        ]);
        let registry = ToolRegistry::new();
        let gate = DenyAllGate;
        let ctx = context(&scripted, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let outcome = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("missing"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("continues after unknown tool");
        assert_eq!(outcome.final_text, "ok");
        let result_msg = &transcript[2];
        assert!(result_msg.text().contains("unknown tool `missing`"));
    }

    #[tokio::test]
    async fn failing_tool_result_is_recorded() {
        let scripted = Scripted::new(vec![
            result_tools(vec![call("failing", serde_json::json!({}))]),
            result_stop("noted the failure"),
        ]);
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(FailingTool));
        let gate = DenyAllGate;
        let ctx = context(&scripted, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("fail"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("loop continues");
        assert!(transcript[2].text().contains("boom"));
    }

    #[tokio::test]
    async fn approval_deny_is_recorded_not_executed() {
        let scripted = Scripted::new(vec![
            result_tools(vec![call("approving", serde_json::json!({}))]),
            result_stop("switching approach"),
        ]);
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(ApprovingTool));
        let gate = DenyAllGate;
        let ctx = context(&scripted, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("approve me"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("loop continues");
        let text = transcript[2].text();
        assert!(text.contains("denied by approval policy"), "{text}");
    }

    #[tokio::test]
    async fn approval_allow_executes() {
        let scripted = Scripted::new(vec![
            result_tools(vec![call("approving", serde_json::json!({"x": 1}))]),
            result_stop("done"),
        ]);
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(ApprovingTool));
        let gate = AlwaysAllow;
        let ctx = context(&scripted, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let outcome = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("approve me"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("completes");
        assert_eq!(outcome.final_text, "done");
        assert!(
            transcript[2].text().contains("ran "),
            "{}",
            transcript[2].text()
        );
    }

    #[tokio::test]
    async fn concurrent_readonly_tools_respect_order_in_transcript() {
        let scripted = Scripted::new(vec![
            result_tools(vec![
                call("r1", serde_json::json!({"i": 1})),
                call("r2", serde_json::json!({"i": 2})),
                call("r3", serde_json::json!({"i": 3})),
            ]),
            result_stop("batched"),
        ]);
        let mut registry = ToolRegistry::new();
        for name in ["r1", "r2", "r3"] {
            registry.register(Box::new(EchoTool {
                name,
                read_only: true,
            }));
        }
        let gate = DenyAllGate;
        let ctx = context(&scripted, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let outcome = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("batch"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("completes");
        assert_eq!(outcome.final_text, "batched");
        let results: Vec<String> = transcript
            .iter()
            .filter(|m| m.role == crate::message::Role::Tool)
            .map(|m| m.text())
            .collect();
        assert_eq!(results.len(), 3);
        assert!(results[0].contains("r1"), "{results:?}");
        assert!(results[1].contains("r2"), "{results:?}");
        assert!(results[2].contains("r3"), "{results:?}");
    }

    #[tokio::test]
    async fn mutating_tools_are_serialized() {
        struct SerialTool {
            name: &'static str,
            order: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
        }
        #[async_trait]
        impl ToolExecutor for SerialTool {
            fn name(&self) -> &str {
                self.name
            }
            fn read_only(&self) -> bool {
                false
            }
            async fn execute(
                &self,
                _call: &ToolCall,
                _cancel: CancellationToken,
            ) -> Result<ToolOutput, ToolError> {
                // Simulate work that must not interleave with other writes.
                tokio::task::yield_now().await;
                self.order.lock().expect("lock").push(self.name);
                Ok(ToolOutput {
                    text: self.name.to_owned(),
                    is_error: false,
                })
            }
        }
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let scripted = Scripted::new(vec![
            result_tools(vec![
                call("w1", serde_json::json!({})),
                call("w2", serde_json::json!({})),
            ]),
            result_stop("serial"),
        ]);
        let mut registry = ToolRegistry::new();
        for name in ["w1", "w2"] {
            registry.register(Box::new(SerialTool {
                name,
                order: order.clone(),
            }));
        }
        let gate = DenyAllGate;
        let ctx = context(&scripted, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let outcome = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("write both"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("completes");
        assert_eq!(outcome.final_text, "serial");
        let recorded = order.lock().expect("lock").clone();
        assert_eq!(
            recorded,
            vec!["w1", "w2"],
            "mutating tools must be serialized"
        );
    }

    #[tokio::test]
    async fn tool_output_is_bounded() {
        struct BigTool;
        #[async_trait]
        impl ToolExecutor for BigTool {
            fn name(&self) -> &str {
                "big"
            }
            fn read_only(&self) -> bool {
                true
            }
            async fn execute(
                &self,
                _call: &ToolCall,
                _cancel: CancellationToken,
            ) -> Result<ToolOutput, ToolError> {
                Ok(ToolOutput {
                    text: "x".repeat(10_000),
                    is_error: false,
                })
            }
        }
        let scripted = Scripted::new(vec![
            result_tools(vec![call("big", serde_json::json!({}))]),
            result_stop("bounded"),
        ]);
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(BigTool));
        let gate = DenyAllGate;
        let config = LoopConfig {
            max_tool_output_bytes: 256,
            ..Default::default()
        };
        let ctx = context(&scripted, &registry, &gate, config);
        let mut transcript: Vec<Message> = Vec::new();
        run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("big"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect("completes");
        let text = transcript[2].text();
        assert!(text.len() < 400, "len={}", text.len());
        assert!(text.contains("truncated"), "{text}");
    }

    #[tokio::test]
    async fn cancellation_during_inference_aborts() {
        /// A transport that parks on the shared cancel token, exposing it to
        /// a watchdog std thread.
        struct Slow {
            parked: std::sync::Arc<std::sync::Mutex<Option<CancellationToken>>>,
        }
        #[async_trait]
        impl InferenceTransport for Slow {
            async fn infer(
                &self,
                _request: InferenceRequest,
                cancel: CancellationToken,
                _events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
            ) -> Result<InferenceResult, TransportError> {
                *self.parked.lock().expect("lock") = Some(cancel.clone());
                let deadline = Instant::now() + Duration::from_secs(5);
                while !cancel.is_cancelled() {
                    if Instant::now() >= deadline {
                        return Ok(result_stop("too late"));
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                Err(TransportError::Fatal("cancelled".to_owned()))
            }
        }
        let parked = std::sync::Arc::new(std::sync::Mutex::new(None));
        let transport = Slow {
            parked: parked.clone(),
        };
        let watchdog = std::thread::spawn(move || {
            loop {
                if let Some(token) = parked.lock().expect("lock").clone() {
                    std::thread::sleep(Duration::from_millis(20));
                    token.cancel();
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        let registry = ToolRegistry::new();
        let gate = DenyAllGate;
        let ctx = context(&transport, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let error = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("slow"),
            CancellationToken::new(),
            sink().0,
        )
        .await
        .expect_err("cancelled");
        assert!(error.reason.contains("cancelled"), "{}", error.reason);
        assert!(
            error
                .events
                .iter()
                .any(|event| event.kind == EventKind::TurnCancelled)
        );
        watchdog.join().expect("watchdog");
    }

    #[tokio::test]
    async fn cancellation_between_steps_aborts() {
        let token = CancellationToken::new();
        struct CancellationWatcher {
            token: CancellationToken,
            called: std::sync::atomic::AtomicBool,
        }
        #[async_trait]
        impl InferenceTransport for CancellationWatcher {
            async fn infer(
                &self,
                _request: InferenceRequest,
                _cancel: CancellationToken,
                _events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
            ) -> Result<InferenceResult, TransportError> {
                if self.called.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    // Second step: cancel now, before doing work.
                    self.token.cancel();
                    Err(TransportError::Fatal("cancelled".to_owned()))
                } else {
                    Ok(result_tools(vec![call(
                        "echo",
                        serde_json::json!({"v": 1}),
                    )]))
                }
            }
        }
        let transport = CancellationWatcher {
            token: token.clone(),
            called: std::sync::atomic::AtomicBool::new(false),
        };
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool {
            name: "echo",
            read_only: true,
        }));
        let gate = DenyAllGate;
        let ctx = context(&transport, &registry, &gate, LoopConfig::default());
        let mut transcript: Vec<Message> = Vec::new();
        let error = run_turn(
            &ctx,
            "ws-test",
            &mut transcript,
            Message::user("cancel"),
            token,
            sink().0,
        )
        .await
        .expect_err("cancelled between steps");
        assert!(error.reason.contains("cancelled"), "{}", error.reason);
        assert!(
            error
                .events
                .iter()
                .any(|event| event.kind == EventKind::TurnCancelled)
        );
    }
}
