//! Integration tests for the llama.cpp streaming transport.
//!
//! A real axum server on 127.0.0.1 feeds SSE through the exact transport
//! path (bytes -> SSE parser -> chunk protocol -> `InferenceResult`).
//! Deterministic fragmentation is covered by the parser unit tests (in
//! `sse.rs`) and a proptest over random chunk sizes here; the HTTP tests
//! cover end-to-end semantics: text/reasoning/tool assembly, usage/timings,
//! error classification, cancellation, and endpoint policy.

use std::sync::Arc;
use std::time::Duration;

use spackle_core::agent::{
    FinishReason, InferenceRequest, InferenceTransport, SamplingParams, StreamEvent, ToolDefinition,
};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::{Message, ToolCall};
use spackle_llamacpp::chat_request_body;
use spackle_llamacpp::client::LlamaCppClient;
use spackle_llamacpp::sse::SseParser;
use spackle_llamacpp::transport::LlamaTransport;
use spackle_llamacpp::wire::{Chunk, ToolCallAssembler};
use spackle_llamacpp::wire::{TimingsWire, UsageWire, usage_from_wire};

// ---------------------------------------------------------------------------
// SSE parser (deterministic fragmentation on top of the unit tests)
// ---------------------------------------------------------------------------

#[test]
fn sse_parser_random_chunk_sizes_are_equivalent() {
    use proptest::prelude::*;
    proptest! {
        |(chunk in 1usize..40usize, salt in 0u32..1024u32)| {
            let payload = format!("data: {{\"n\":{salt},\"s\":\"héllo — 🦙\"}}\n\ndata: [DONE]\n\n");
            let bytes = payload.into_bytes();
            let mut whole = SseParser::new();
            let mut expected = whole.feed(&bytes).expect("feed");
            if let Some(final_payload) = whole.finish() {
                expected.push(final_payload);
            }
            let mut chunked = SseParser::new();
            let mut got = Vec::new();
            for piece in bytes.chunks(chunk) {
                got.extend(chunked.feed(piece).expect("feed"));
            }
            if let Some(final_payload) = chunked.finish() {
                got.push(final_payload);
            }
            prop_assert_eq!(got, expected);
        }
    }
}

#[test]
fn sse_parser_invalid_utf8_line_is_an_error() {
    let mut parser = SseParser::new();
    let err = parser.feed(b"data: \xff\xfe\n\n").expect_err("must fail");
    assert!(err.to_string().contains("UTF-8"), "{err}");
}

// ---------------------------------------------------------------------------
// Tool-call assembly (wire protocol facts)
// ---------------------------------------------------------------------------

fn apply_delta(assembler: &mut ToolCallAssembler, chunk_json: &str) {
    let chunk: Chunk = serde_json::from_str(chunk_json).unwrap();
    let choice = chunk.choices.unwrap().into_iter().next().unwrap();
    assembler.apply(&choice.delta);
}

#[test]
fn assembler_handles_fragmented_tool_calls_and_starts() {
    let mut assembler = ToolCallAssembler::new("seed".to_owned());
    apply_delta(
        &mut assembler,
        r#"{"choices": [{"delta": {"tool_calls": [{"id": "call-1", "function": {"name": "inspect_files", "arguments": "{\"paths\":["}}]}}]}"#,
    );
    apply_delta(
        &mut assembler,
        r#"{"choices": [{"delta": {"tool_calls": [{"function": {"arguments": "\"a\""}}]}}]}"#,
    );
    apply_delta(
        &mut assembler,
        r#"{"choices": [{"delta": {"tool_calls": [{"function": {"arguments": "]}"}}]}}]}"#,
    );
    apply_delta(
        &mut assembler,
        r#"{"choices": [{"delta": {"tool_calls": [{"id": "call-2", "function": {"name": "write_file", "arguments": "{}"}}]}}]}"#,
    );
    let calls = assembler.finish();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert_eq!(calls[0].id, "call-1");
    assert_eq!(calls[0].name, "inspect_files");
    assert_eq!(calls[0].arguments, serde_json::json!({"paths": ["a"]}));
    assert_eq!(calls[1].id, "call-2");
    assert_eq!(calls[1].name, "write_file");
    assert_eq!(calls[1].arguments, serde_json::json!({}));
}

#[test]
fn assembler_uses_fallback_ids_when_server_omits_them() {
    let mut assembler = ToolCallAssembler::new("seed".to_owned());
    apply_delta(
        &mut assembler,
        r#"{"choices": [{"delta": {"tool_calls": [{"function": {"name": "edit_file", "arguments": "{}"}}]}}]}"#,
    );
    let calls = assembler.finish();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].id.contains("seed"), "{}", calls[0].id);
}

// ---------------------------------------------------------------------------
// Request body shape (asserted against the wire)
// ---------------------------------------------------------------------------

fn base_request() -> InferenceRequest {
    let profile = spackle_core::config::generation::GenerationProfile::default();
    InferenceRequest {
        model: "qwen3.8-27b-local".to_owned(),
        system: "You are a careful coding agent.".to_owned(),
        messages: vec![Message::user("Say hi.")],
        tools: Vec::new(),
        sampling: SamplingParams::from(&profile),
    }
}

#[test]
fn request_body_carries_profile_and_parse_tool_calls() {
    let mut request = base_request();
    request.sampling.enable_thinking = Some(true);
    request.sampling.preserve_thinking = Some(true);
    request.sampling.reasoning_effort =
        Some(spackle_core::config::generation::ReasoningEffort::Medium);
    request.tools = vec![ToolDefinition {
        name: "inspect_files".into(),
        description: "read files".into(),
        parameters: serde_json::json!({"type": "object"}),
    }];
    let body = chat_request_body(
        &request.model,
        &request.system,
        &request.messages,
        &request.tools,
        &request.sampling,
        true,
    );
    assert_eq!(body["stream"], serde_json::Value::Bool(true));
    assert_eq!(body["parse_tool_calls"], serde_json::Value::Bool(true));
    assert_eq!(
        body["stream_options"]["include_usage"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        body["chat_template_kwargs"]["enable_thinking"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        body["chat_template_kwargs"]["preserve_thinking"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        body["reasoning_effort"],
        serde_json::Value::String("medium".into())
    );
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "Say hi.");
    assert_eq!(body["tools"][0]["function"]["name"], "inspect_files");
}

#[test]
fn usage_and_timings_wire_mapping_is_lossless() {
    let usage: UsageWire = serde_json::from_value(serde_json::json!({
        "prompt_tokens": 10,
        "completion_tokens": 4,
        "total_tokens": 14,
        "prompt_tokens_details": {"cached_tokens": 3},
        "completion_tokens_details": {"reasoning_tokens": 2}
    }))
    .unwrap();
    let mapped = usage_from_wire(&usage);
    assert_eq!(mapped.prompt_tokens, Some(10));
    assert_eq!(mapped.completion_tokens, Some(4));
    assert_eq!(mapped.cached_tokens, Some(3));
    assert_eq!(mapped.reasoning_tokens, Some(2));

    let timings: TimingsWire = serde_json::from_value(serde_json::json!({
        "prompt_ms": 5, "predicted_ms": 50,
        "prompt_per_second": 2.0, "predicted_per_second": 8.0
    }))
    .unwrap();
    assert_eq!(timings.prompt_per_second, Some(2.0));
}

// ---------------------------------------------------------------------------
// End-to-end over a real local server
// ---------------------------------------------------------------------------

async fn serve_script(script: Vec<Vec<u8>>) -> String {
    use axum::Router;
    use axum::body::{Body, Bytes};
    use axum::http::Response;
    use axum::routing::post;

    #[derive(Clone)]
    struct S {
        script: Arc<Vec<Vec<u8>>>,
    }

    async fn chat(state: axum::extract::State<S>) -> Response<Body> {
        let mut body: Vec<u8> = Vec::new();
        for batch in state.script.iter() {
            body.extend_from_slice(batch);
        }
        Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .body(Body::from(Bytes::from(body)))
            .unwrap()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(chat))
        .with_state(S {
            script: Arc::new(script),
        });
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

fn text_chunk(content: &str) -> Vec<u8> {
    format!(
        "data: {{\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":{}}},\"finish_reason\":null}}]}}\n\n",
        serde_json::json!(content)
    )
    .into_bytes()
}

async fn run_infer(
    base_url: &str,
    request: InferenceRequest,
    cancel: CancellationToken,
) -> Result<
    (spackle_core::agent::InferenceResult, Vec<StreamEvent>),
    spackle_core::agent::TransportError,
> {
    let client = LlamaCppClient::new(base_url).unwrap();
    let transport = LlamaTransport::new(client, "model".to_owned());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    let result = transport.infer(request, cancel, tx).await?;
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    Ok((result, events))
}

#[tokio::test]
async fn end_to_end_text_stream_with_usage_and_timings() {
    let script = vec![
        text_chunk("Hel"),
        text_chunk("lo wörld — 🦙"),
        text_chunk(""),
        b"data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":4,\"total_tokens\":14,\"prompt_tokens_details\":{\"cached_tokens\":3}},\"timings\":{\"prompt_ms\":5,\"predicted_ms\":50,\"prompt_per_second\":2.0,\"predicted_per_second\":8.0}}\n\n".to_vec(),
        b"data: [DONE]\n\n".to_vec(),
    ];
    let base_url = serve_script(script).await;
    let (result, events) = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.content, "Hello wörld — 🦙");
    assert!(result.tool_calls.is_empty());
    assert_eq!(result.usage.prompt_tokens, Some(10));
    assert_eq!(result.usage.completion_tokens, Some(4));
    assert_eq!(result.usage.cached_tokens, Some(3));
    assert_eq!(result.timings.prompt_ms, Some(5));
    assert_eq!(result.timings.predicted_per_second, Some(8.0));
    assert!(result.first_token_ms.is_some(), "first token must be timed");
    let text_events: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextDelta { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_events,
        vec!["Hel".to_owned(), "lo wörld — 🦙".to_owned()]
    );
    assert!(matches!(result.finish_reason, FinishReason::Stop));
}

#[tokio::test]
async fn end_to_end_reasoning_then_text() {
    let mut script = vec![
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"think \"},\"finish_reason\":null}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"again\"},\"finish_reason\":null}]}\n\n".to_vec(),
    ];
    script.push(text_chunk("answer"));
    script.push(b"data: [DONE]\n\n".to_vec());
    let base_url = serve_script(script).await;
    let (result, events) = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.reasoning, "think again");
    assert_eq!(result.content, "answer");
    let reasoning_events = events
        .iter()
        .filter(|event| matches!(event, StreamEvent::ReasoningDelta { .. }))
        .count();
    assert_eq!(reasoning_events, 2);
}

#[tokio::test]
async fn end_to_end_tool_call_streaming_assembles() {
    let script = vec![
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call-9\",\"function\":{\"name\":\"inspect_files\",\"arguments\":\"{\"}}]},\"finish_reason\":null}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"}\"}}]},\"finish_reason\":null}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n".to_vec(),
        b"data: [DONE]\n\n".to_vec(),
    ];
    let base_url = serve_script(script).await;
    let (result, events) = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap();
    assert!(result.content.is_empty());
    assert_eq!(result.tool_calls.len(), 1, "{:?}", result.tool_calls);
    let call: &ToolCall = &result.tool_calls[0];
    assert_eq!(call.id, "call-9");
    assert_eq!(call.name, "inspect_files");
    assert_eq!(call.arguments, serde_json::json!({}));
    assert!(matches!(result.finish_reason, FinishReason::ToolCalls));
    let deltas = events
        .iter()
        .filter(|event| matches!(event, StreamEvent::ToolCallDelta { .. }))
        .count();
    assert_eq!(deltas, 3);
}

async fn serve_status(status: u16, body: &str) -> String {
    use axum::Router;
    use axum::body::{Body, Bytes};
    use axum::extract::State;
    use axum::http::Response;
    use axum::routing::post;

    #[derive(Clone)]
    struct S {
        status: u16,
        body: Vec<u8>,
    }

    async fn chat(state: State<S>) -> Response<Body> {
        Response::builder()
            .status(state.status)
            .header("content-type", "application/json")
            .body(Body::from(Bytes::from(state.body.clone())))
            .unwrap()
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(chat))
        .with_state(S {
            status,
            body: body.as_bytes().to_vec(),
        });
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn http_400_is_fatal_with_server_message() {
    let base_url = serve_status(400, r#"{"error":{"message":"model not found"}}"#).await;
    let err = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap_err();
    match err {
        spackle_core::agent::TransportError::Fatal(message) => {
            assert!(message.contains("model not found"), "{message}")
        }
        other => panic!("expected fatal, got {other:?}"),
    }
}

#[tokio::test]
async fn http_429_is_transient_for_retry() {
    let base_url = serve_status(429, r#"{"error":"rate limited"}"#).await;
    let err = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        matches!(err, spackle_core::agent::TransportError::Transient(_)),
        "{err:?}"
    );
}

#[tokio::test]
async fn unknown_field_400_is_a_capability_error() {
    let base_url = serve_status(
        400,
        r#"{"error":"unrecognized field `chat_template_kwargs`"}"#,
    )
    .await;
    let err = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap_err();
    match err {
        spackle_core::agent::TransportError::Capability { feature, .. } => {
            assert!(feature.contains("chat_template_kwargs"), "{feature}")
        }
        other => panic!("expected capability, got {other:?}"),
    }
}

#[tokio::test]
async fn server_error_chunk_mid_stream_is_fatal() {
    let script = vec![
        text_chunk("partial"),
        b"data: {\"error\":{\"message\":\"server exploded\"}}\n\n".to_vec(),
    ];
    let base_url = serve_script(script).await;
    let err = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap_err();
    match err {
        spackle_core::agent::TransportError::Fatal(message) => {
            assert!(message.contains("server exploded"), "{message}")
        }
        other => panic!("expected fatal, got {other:?}"),
    }
}

#[tokio::test]
async fn cancellation_mid_stream_returns_promptly() {
    use axum::Router;
    use axum::body::Body;
    use axum::http::Response;
    use axum::routing::post;
    use bytes::Bytes;

    async fn slow() -> Response<Body> {
        let pieces = vec![
            Bytes::from(
                "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"one \"},\"finish_reason\":null}]}\n\n",
            ),
            Bytes::from(
                "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"two\"},\"finish_reason\":null}]}\n\n",
            ),
            Bytes::from("data: [DONE]\n\n"),
        ];
        let stream = futures_util::stream::unfold(
            (pieces, 0usize),
            |(pieces, i): (Vec<Bytes>, usize)| async move {
                if i >= pieces.len() {
                    return None;
                }
                let piece = pieces[i].clone();
                if i + 1 == pieces.len() {
                    // Hold the final event: a client that waits would hang.
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                Some((Ok(piece) as Result<Bytes, std::io::Error>, (pieces, i + 1)))
            },
        );
        Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap()
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/v1/chat/completions", post(slow));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let base_url = format!("http://{addr}");

    let client = LlamaCppClient::new(&base_url).unwrap();
    let transport = LlamaTransport::new(client, "model".to_owned());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    let token = CancellationToken::new();
    let token_for_task = token.clone();
    let task =
        tokio::spawn(async move { transport.infer(base_request(), token_for_task, tx).await });

    // Wait for the first streamed delta, then cancel.
    assert!(rx.recv().await.is_some(), "expected at least one delta");
    token.cancel();
    let started = std::time::Instant::now();
    let outcome = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("cancellation must return promptly")
        .expect("infer task must not panic");
    let elapsed = started.elapsed();
    let err = outcome.unwrap_err();
    assert!(
        matches!(err, spackle_core::agent::TransportError::Transient(_)),
        "{err:?}"
    );
    assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");
}

#[tokio::test]
async fn connection_refused_is_transient() {
    // Bind then drop a listener to get a port that refuses connections.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let base_url = format!("http://{addr}");
    let err = run_infer(&base_url, base_request(), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        matches!(err, spackle_core::agent::TransportError::Transient(_)),
        "{err:?}"
    );
}

// ---------------------------------------------------------------------------
// Endpoint policy
// ---------------------------------------------------------------------------

#[test]
fn endpoint_policy_enforced_at_construction() {
    assert!(LlamaCppClient::new("http://127.0.0.1:8080").is_ok());
    assert!(LlamaCppClient::new("http://[::1]:8080").is_ok());
    let lan = LlamaCppClient::new("http://192.168.1.20:8080");
    assert!(lan.is_err(), "private LAN must be rejected by default");
    assert!(LlamaCppClient::with_policy("http://192.168.1.20:8080", true).is_ok());
    let public = LlamaCppClient::with_policy("http://llama.example.com:8080", true);
    assert!(public.is_err(), "public endpoints are always rejected");
}
