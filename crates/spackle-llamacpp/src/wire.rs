//! Mapping between the core domain model and llama.cpp's OpenAI-compatible
//! Chat Completions wire protocol (verified against the vendored llama.cpp
//! 2.28.2 server sources).
//!
//! Wire facts implemented here (from `offline/reference-src`):
//! - Request: standard sampling fields plus `chat_template_kwargs`
//!   (object of values; `enable_thinking` is parsed server-side),
//!   `reasoning_effort` (string; "none" disables), and
//!   `stream_options.include_usage`.
//! - Stream delta: `content` (string), `reasoning_content` (string),
//!   `tool_calls` (at most one entry per delta: optional `id`,
//!   `function.name`, `function.arguments` as a *string fragment*; there is
//!   no index field — a new `id`/`name` starts a new call).
//! - Final chunk: `choices: []` plus `usage`
//!   (`prompt_tokens`, `completion_tokens`,
//!   `prompt_tokens_details.cached_tokens`) and `timings`
//!   (`prompt_ms`, `predicted_ms`, `prompt_per_second`,
//!   `predicted_per_second`).

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use spackle_core::agent::{SamplingParams, ToolDefinition, Usage};
use spackle_core::config::generation::ReasoningEffort;
use spackle_core::message::{ContentBlock, Message, ToolCall};

/// Build the `POST /v1/chat/completions` request body.
#[must_use]
pub fn chat_request_body(
    model: &str,
    system: &str,
    messages: &[Message],
    tools: &[ToolDefinition],
    sampling: &SamplingParams,
    stream: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": message_wire(system, messages),
        "stream": stream,
        "temperature": sampling.temperature,
        "top_p": sampling.top_p,
        "top_k": sampling.top_k,
        "min_p": sampling.min_p,
        "presence_penalty": sampling.presence_penalty,
        "repeat_penalty": sampling.repeat_penalty,
        "max_tokens": sampling.max_tokens,
    });

    if stream {
        body["stream_options"] = json!({ "include_usage": true });
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        }
                    })
                })
                .collect(),
        );
        body["parse_tool_calls"] = json!(true);
    }

    // Qwen3.8 thinking controls travel through the chat template keywords.
    let mut kwargs: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(flag) = sampling.enable_thinking {
        kwargs.insert("enable_thinking".to_owned(), Value::Bool(flag));
    }
    if let Some(flag) = sampling.preserve_thinking {
        kwargs.insert("preserve_thinking".to_owned(), Value::Bool(flag));
    }
    if !kwargs.is_empty() {
        body["chat_template_kwargs"] = Value::Object(
            kwargs
                .into_iter()
                .collect::<serde_json::Map<String, Value>>(),
        );
    }
    if let Some(effort) = sampling.reasoning_effort {
        body["reasoning_effort"] = Value::String(reasoning_effort_wire(effort).to_owned());
    }
    body
}

/// The string the llama.cpp server accepts for `reasoning_effort`.
#[must_use]
pub fn reasoning_effort_wire(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::Xhigh => "xhigh",
    }
}

/// Render core messages to wire messages, with the system prompt first.
fn message_wire(system: &str, messages: &[Message]) -> Value {
    let mut out: Vec<Value> = Vec::with_capacity(messages.len() + 1);
    if !system.trim().is_empty() {
        out.push(json!({ "role": "system", "content": system }));
    }
    for message in messages {
        match message.role {
            spackle_core::message::Role::System => {
                out.push(json!({ "role": "system", "content": message.text() }));
            }
            spackle_core::message::Role::User => {
                out.push(json!({ "role": "user", "content": message.text() }));
            }
            spackle_core::message::Role::Assistant => {
                let mut item = json!({
                    "role": "assistant",
                    "content": message.text(),
                });
                if let Some(reasoning) =
                    (!message.reasoning().is_empty()).then(|| message.reasoning())
                {
                    item["reasoning_content"] = Value::String(reasoning);
                }
                let calls = message.tool_calls();
                if !calls.is_empty() {
                    item["tool_calls"] = Value::Array(
                        calls
                            .iter()
                            .map(|call| {
                                json!({
                                    "type": "function",
                                    "id": call.id,
                                    "function": {
                                        "name": call.name,
                                        "arguments": call.arguments.to_string(),
                                    }
                                })
                            })
                            .collect(),
                    );
                }
                out.push(item);
            }
            spackle_core::message::Role::Tool => {
                // The tool result message carries exactly one ToolResult block.
                let (id, name, output) = message
                    .blocks
                    .iter()
                    .find_map(|block| match block {
                        ContentBlock::ToolResult(result) => Some((
                            result.id.clone(),
                            result.name.clone(),
                            result.output.clone(),
                        )),
                        _ => None,
                    })
                    .unwrap_or_default();
                let mut item = json!({
                    "role": "tool",
                    "tool_call_id": id,
                    "content": output,
                });
                if !name.is_empty() {
                    item["name"] = Value::String(name);
                }
                out.push(item);
            }
        }
    }
    Value::Array(out)
}

/// One stream chunk (a `data:` payload, minus the `[DONE]` sentinel).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Chunk {
    pub id: Option<String>,
    pub choices: Option<Vec<Choice>>,
    pub usage: Option<UsageWire>,
    pub timings: Option<TimingsWire>,
    /// Server-side error surfaced inside the stream.
    pub error: Option<StreamError>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Choice {
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Delta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeltaToolCall {
    pub id: Option<String>,
    pub function: Option<DeltaFunction>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeltaFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UsageWire {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub prompt_tokens_details: Option<TokenDetails>,
    pub completion_tokens_details: Option<TokenDetails>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TokenDetails {
    pub cached_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TimingsWire {
    pub prompt_ms: Option<f64>,
    pub predicted_ms: Option<f64>,
    pub prompt_per_second: Option<f64>,
    pub predicted_per_second: Option<f64>,
    pub prompt_n: Option<u64>,
    pub predicted_n: Option<u64>,
    pub cache_n: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StreamError {
    pub message: Option<String>,
    pub code: Option<Value>,
}

/// Map a wire usage object onto the core `Usage`.
#[must_use]
pub fn usage_from_wire(wire: &UsageWire) -> Usage {
    Usage {
        prompt_tokens: wire.prompt_tokens,
        completion_tokens: wire.completion_tokens,
        cached_tokens: wire
            .prompt_tokens_details
            .as_ref()
            .and_then(|details| details.cached_tokens)
            .or_else(|| {
                wire.completion_tokens_details
                    .as_ref()
                    .and_then(|d| d.cached_tokens)
            }),
        reasoning_tokens: wire
            .completion_tokens_details
            .as_ref()
            .and_then(|details| details.reasoning_tokens),
    }
}

/// Assemble streamed tool-call deltas into complete `ToolCall`s.
///
/// llama.cpp sends at most one tool-call delta per chunk: the first chunk
/// for a call carries `id`/`name`; later chunks carry only an `arguments`
/// fragment. A delta with an `id` or `name` starts a new call; a delta
/// with neither appends to the currently open call.
pub struct ToolCallAssembler {
    calls: Vec<ToolCall>,
    /// Open call currently receiving argument fragments, if any.
    open: Option<usize>,
    fallback_seed: String,
}

impl ToolCallAssembler {
    #[must_use]
    pub fn new(request_seed: String) -> Self {
        Self {
            calls: Vec::new(),
            open: None,
            fallback_seed: request_seed,
        }
    }

    /// Apply one delta's tool-call data (if any).
    pub fn apply(&mut self, delta: &Delta) {
        for (position, item) in delta.tool_calls.iter().flatten().enumerate() {
            let has_identity = item.id.is_some()
                || item
                    .function
                    .as_ref()
                    .is_some_and(|function| function.name.is_some());
            let index = if has_identity {
                let id = item
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("call-{}-{}", self.fallback_seed, self.calls.len()));
                let name = item
                    .function
                    .as_ref()
                    .and_then(|function| function.name.clone())
                    .unwrap_or_default();
                self.calls.push(ToolCall {
                    id,
                    name,
                    arguments: Value::Null,
                });
                let index = self.calls.len() - 1;
                self.open = Some(index);
                index
            } else {
                match self.open {
                    Some(index) => index,
                    None => {
                        // Defensive: an arguments-only delta with no open
                        // call (should not happen from llama.cpp).
                        let id = format!("call-{}-{}", self.fallback_seed, self.calls.len());
                        self.calls.push(ToolCall {
                            id,
                            name: String::new(),
                            arguments: Value::Null,
                        });
                        let index = self.calls.len() - 1;
                        self.open = Some(index);
                        index
                    }
                }
            };
            if let Some(fragment) = &item
                .function
                .as_ref()
                .and_then(|function| function.arguments.clone())
                && !fragment.is_empty()
            {
                let call = &mut self.calls[index];
                let accumulated = match &mut call.arguments {
                    Value::Null => fragment.clone(),
                    existing => {
                        let mut text = existing.as_str().unwrap_or("").to_owned();
                        text.push_str(fragment);
                        text
                    }
                };
                // Arguments arrive as a JSON *string* of the object;
                // parse eagerly when complete, keeping the raw string
                // otherwise (never fail mid-stream).
                if let Ok(parsed) = serde_json::from_str::<Value>(&accumulated) {
                    call.arguments = parsed;
                } else {
                    call.arguments = Value::String(accumulated);
                }
            }
            let _ = position;
        }
    }

    /// Finalize: normalize any call whose arguments stayed a raw string.
    pub fn finish(self) -> Vec<ToolCall> {
        self.calls
            .into_iter()
            .map(|mut call| {
                if let Value::String(raw) = &call.arguments {
                    call.arguments = serde_json::from_str::<Value>(raw)
                        .unwrap_or(Value::Object(Default::default()));
                }
                if call.arguments == Value::Null {
                    call.arguments = Value::Object(Default::default());
                }
                call
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spackle_core::agent::ToolDefinition as CoreTool;

    #[test]
    fn request_body_carries_profile_and_stream_options() {
        let sampling = SamplingParams {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 20,
            min_p: 0.0,
            presence_penalty: 1.5,
            repeat_penalty: 1.0,
            max_tokens: 8192,
            enable_thinking: Some(false),
            preserve_thinking: Some(true),
            reasoning_effort: Some(ReasoningEffort::Xhigh),
        };
        let tools = vec![CoreTool {
            name: "inspect_files".to_owned(),
            description: "Read files".to_owned(),
            parameters: json!({"type": "object"}),
        }];
        let body = chat_request_body(
            "qwen3.8-27b-local",
            "system prompt",
            &[spackle_core::message::Message::user("hi")],
            &tools,
            &sampling,
            true,
        );
        assert_eq!(body["model"], "qwen3.8-27b-local");
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert_eq!(body["chat_template_kwargs"]["enable_thinking"], false);
        assert_eq!(body["chat_template_kwargs"]["preserve_thinking"], true);
        assert_eq!(body["reasoning_effort"], "xhigh");
        assert_eq!(body["tools"][0]["function"]["name"], "inspect_files");
        assert_eq!(body["parse_tool_calls"], true);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["max_tokens"], 8192);
        assert_eq!(body["presence_penalty"], 1.5);
    }

    #[test]
    fn assistant_tool_call_round_trips_into_wire() {
        let message = spackle_core::message::Message::assistant(
            "checking".to_owned(),
            Some("thought".to_owned()),
            vec![ToolCall {
                id: "call_1".to_owned(),
                name: "inspect_files".to_owned(),
                arguments: json!({"paths": ["a.rs"]}),
            }],
        );
        let messages = message_wire("sys", &[message]);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["reasoning_content"], "thought");
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["name"],
            "inspect_files"
        );
        let args = messages[1]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .expect("wire arguments are a string");
        assert!(args.contains("a.rs"), "{args}");
    }

    #[test]
    fn tool_result_wire_message_uses_tool_call_id() {
        let message = spackle_core::message::Message::tool_result(
            "call_9".to_owned(),
            "echo".to_owned(),
            "out".to_owned(),
            false,
        );
        let messages = message_wire("", &[message]);
        assert_eq!(messages[0]["role"], "tool");
        assert_eq!(messages[0]["tool_call_id"], "call_9");
        assert_eq!(messages[0]["content"], "out");
    }

    #[test]
    fn chunk_deserializes_llamacpp_shape() {
        let raw = r#"{
            "id": "cmpl-1",
            "choices": [{
                "delta": {
                    "content": "he",
                    "reasoning_content": "th",
                    "tool_calls": [{
                        "id": "call_a",
                        "function": { "name": "echo", "arguments": "{\"" }
                    }]
                }
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 4,
                "total_tokens": 14,
                "prompt_tokens_details": { "cached_tokens": 8 }
            },
            "timings": {
                "prompt_ms": 50.0,
                "predicted_ms": 100.0,
                "prompt_per_second": 200.0,
                "predicted_per_second": 40.0
            }
        }"#;
        let chunk: Chunk = serde_json::from_str(raw).expect("chunk");
        let choices = chunk.choices.expect("choices");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].delta.content.as_deref(), Some("he"));
        let usage = usage_from_wire(&chunk.usage.expect("usage"));
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.cached_tokens, Some(8));
        let timings = chunk.timings.expect("timings");
        assert_eq!(timings.prompt_per_second, Some(200.0));
    }

    #[test]
    fn assembler_assembles_fragments_across_deltas() {
        let mut assembler = ToolCallAssembler::new("seed".to_owned());
        let first: Delta = serde_json::from_value(json!({
            "tool_calls": [{
                "id": "call_1",
                "function": { "name": "echo", "arguments": "{\"v\":" }
            }]
        }))
        .expect("delta");
        assembler.apply(&first);
        let second: Delta = serde_json::from_value(json!({
            "tool_calls": [{
                "function": { "arguments": "1 }" }
            }]
        }))
        .expect("delta");
        assembler.apply(&second);
        let third: Delta = serde_json::from_value(json!({
            "tool_calls": [{
                "id": "call_2",
                "function": { "name": "other", "arguments": "{}" }
            }]
        }))
        .expect("delta");
        assembler.apply(&third);
        let calls = assembler.finish();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "echo");
        assert_eq!(calls[0].arguments, json!({"v": 1}));
        assert_eq!(calls[1].id, "call_2");
        assert_eq!(calls[1].arguments, json!({}));
    }

    #[test]
    fn assembler_fallback_id_when_server_omits_it() {
        let mut assembler = ToolCallAssembler::new("req-7".to_owned());
        let delta: Delta = serde_json::from_value(json!({
            "tool_calls": [{
                "function": { "name": "echo", "arguments": "{}" }
            }]
        }))
        .expect("delta");
        assembler.apply(&delta);
        let calls = assembler.finish();
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0].id.contains("req-7"),
            "fallback id must be stable per request: {}",
            calls[0].id
        );
    }
}
