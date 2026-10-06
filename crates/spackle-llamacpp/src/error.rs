//! Client errors with transport classification (transient vs fatal vs
//! capability) so the agent loop can retry or diagnose precisely.

use reqwest::StatusCode;
use spackle_core::agent::TransportError;
use thiserror::Error;

use crate::sse::SseError;

/// Errors from talking to a llama.cpp server.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The endpoint violates the local-only policy or is not a URL.
    #[error("invalid endpoint: {0}")]
    InvalidEndpoint(String),
    /// Connection-level failure (DNS, connect, TLS, body decoding).
    #[error("llama.cpp request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// The server answered with a non-2xx status.
    #[error("llama.cpp returned HTTP {status}: {message}")]
    Http {
        status: u16,
        message: String,
        retryable: bool,
        capability: Option<(String, String)>,
    },
    /// A complete SSE line was malformed.
    #[error("stream error: {0}")]
    Sse(#[from] SseError),
    /// A chunk payload could not be parsed as a completion chunk.
    #[error("malformed stream chunk: {0}")]
    Malformed(String),
    /// Cancellation was requested.
    #[error("cancelled by user")]
    Cancelled,
}

impl ClientError {
    /// Build a classified HTTP error from a status and response body.
    pub(crate) fn http(status: StatusCode, body: String) -> Self {
        let message = extract_error_message(&body).unwrap_or_else(|| {
            format!(
                "HTTP {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("error")
            )
        });
        let retryable = matches!(status.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504);
        let capability = looks_like_capability(&body).map(|feature| {
            (
                feature.to_owned(),
                "the attached llama.cpp build does not accept this parameter; adjust the generation profile or server build".to_owned(),
            )
        });
        Self::Http {
            status: status.as_u16(),
            message,
            retryable,
            capability,
        }
    }
}

/// Pull a human-readable message out of a llama.cpp error body.
fn extract_error_message(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        for key in ["error", "message", "detail"] {
            if let Some(message) = value
                .get(key)
                .or_else(|| value.get("error").and_then(|e| e.get("message")))
                && let Some(text) = message.as_str()
                && !text.is_empty()
            {
                return Some(text.to_owned());
            }
        }
        return None;
    }
    Some(trimmed.chars().take(300).collect())
}

/// Detect "server rejects a feature this profile relies on" bodies.
fn looks_like_capability(body: &str) -> Option<&str> {
    let lower = body.to_lowercase();
    for (needle, feature) in [
        ("chat_template_kwargs", "chat_template_kwargs"),
        ("reasoning_effort", "reasoning_effort"),
        ("stream_options", "stream_options.include_usage"),
        ("parse_tool_calls", "parse_tool_calls"),
        ("unknown field", "request field"),
        ("unrecognized field", "request field"),
        ("unsupported", "request field"),
    ] {
        if lower.contains(needle) {
            return Some(feature);
        }
    }
    None
}

impl From<ClientError> for TransportError {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::InvalidEndpoint(message) => TransportError::Fatal(message),
            ClientError::Request(err) => {
                if err.is_connect() || err.is_timeout() || err.is_request() {
                    TransportError::Transient(err.to_string())
                } else {
                    TransportError::Fatal(err.to_string())
                }
            }
            ClientError::Http {
                status,
                message,
                retryable,
                capability,
            } => match capability {
                Some((feature, hint)) => TransportError::Capability { feature, hint },
                None if retryable => TransportError::Transient(format!("HTTP {status}: {message}")),
                None => TransportError::Fatal(format!("HTTP {status}: {message}")),
            },
            ClientError::Sse(err) => TransportError::Fatal(err.to_string()),
            ClientError::Malformed(detail) => TransportError::Fatal(detail),
            ClientError::Cancelled => TransportError::Transient("cancelled by user".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llamacpp_error_bodies_extract_messages() {
        let err = ClientError::http(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"model not found","type":"invalid_request_error"}}"#.to_owned(),
        );
        let transport: TransportError = err.into();
        match transport {
            TransportError::Fatal(message) => {
                assert!(message.contains("model not found"), "{message}")
            }
            other => panic!("expected fatal, got {other:?}"),
        }
    }

    #[test]
    fn server_5xx_is_transient() {
        let err = ClientError::http(StatusCode::INTERNAL_SERVER_ERROR, "oops".to_owned());
        let transport: TransportError = err.into();
        assert!(matches!(transport, TransportError::Transient(_)));
    }

    #[test]
    fn unknown_field_is_a_capability_error() {
        let err = ClientError::http(
            StatusCode::BAD_REQUEST,
            r#"{"error":"unrecognized field `chat_template_kwargs`"}"#.to_owned(),
        );
        let transport: TransportError = err.into();
        match transport {
            TransportError::Capability { feature, .. } => {
                assert!(feature.contains("chat_template_kwargs"), "{feature}")
            }
            other => panic!("expected capability, got {other:?}"),
        }
    }
}
