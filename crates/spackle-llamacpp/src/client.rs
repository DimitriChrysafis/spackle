//! The llama.cpp HTTP client: endpoint policy, probes, model listing, and
//! streaming chat completions.
//!
//! Network discipline: endpoints are classified before any request —
//! loopback by default, private LAN only with an explicit flag, public
//! endpoints always rejected. Streaming requests carry no global timeout
//! (local inference is slow), but connect and per-read windows are bounded.

use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde::Serialize;
use serde_json::Value;
use url::Url;

use spackle_core::config::endpoint::{EndpointClass, EndpointInfo};

impl From<url::ParseError> for ClientError {
    fn from(err: url::ParseError) -> Self {
        Self::InvalidEndpoint(err.to_string())
    }
}

use crate::error::ClientError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Idle window between streamed bytes; a local server that stalls for this
/// long is treated as broken rather than waited on forever.
const READ_TIMEOUT: Duration = Duration::from_secs(300);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// A llama.cpp endpoint client.
#[derive(Debug, Clone)]
pub struct LlamaCppClient {
    base_url: Url,
    chat: reqwest::Client,
    probe: reqwest::Client,
}

/// One served model (subset of `GET /v1/models`).
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub n_ctx: Option<u64>,
}

impl LlamaCppClient {
    /// Create a client for `endpoint`, enforcing the default local-only
    /// policy (loopback only).
    pub fn new(endpoint: &str) -> Result<Self, ClientError> {
        Self::with_policy(endpoint, false)
    }

    /// Create a client, optionally permitting private-LAN endpoints.
    /// Public endpoints are always rejected.
    pub fn with_policy(endpoint: &str, allow_private_lan: bool) -> Result<Self, ClientError> {
        let info = EndpointInfo::parse(endpoint).map_err(|issue| {
            ClientError::InvalidEndpoint(format!("{}: {}", issue.path, issue.message))
        })?;
        match info.class {
            EndpointClass::Loopback => {}
            EndpointClass::PrivateLan if allow_private_lan => {}
            EndpointClass::PrivateLan => {
                return Err(ClientError::InvalidEndpoint(format!(
                    "{} {} is a private-LAN endpoint and was not explicitly allowed",
                    info.host, info.port
                )));
            }
            EndpointClass::Public => {
                return Err(ClientError::InvalidEndpoint(format!(
                    "{} is a public endpoint; llama only talks to local llama.cpp servers",
                    info.host
                )));
            }
        }
        let mut base_url = match Url::parse(&info.url) {
            Ok(url) => url,
            Err(err) => return Err(ClientError::InvalidEndpoint(err.to_string())),
        };
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let chat = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()?;
        let probe = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(PROBE_TIMEOUT)
            .build()?;
        Ok(Self {
            base_url,
            chat,
            probe,
        })
    }

    /// The resolved base URL.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Stream a chat completion. Returns the raw byte stream; callers
    /// parse SSE (see `crate::sse`) and the chunk protocol (`crate::wire`).
    pub async fn stream_chat(&self, body: Value) -> Result<PinBoxStream, ClientError> {
        let url = self.base_url.join("v1/chat/completions")?;
        let response = self
            .chat
            .post(url)
            .header("accept", "text/event-stream")
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ClientError::http(status, text));
        }
        let stream = response
            .bytes_stream()
            .map(|result| result.map_err(ClientError::from));
        Ok(Box::pin(stream))
    }

    /// Non-streaming chat completion (used by tests and one-shot calls).
    pub async fn chat(&self, body: Value) -> Result<Value, ClientError> {
        let url = self.base_url.join("v1/chat/completions")?;
        let response = self.chat.post(url).json(&body).send().await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ClientError::http(status, text));
        }
        Ok(response.json().await?)
    }

    /// List served models and aliases.
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, ClientError> {
        let url = self.base_url.join("v1/models")?;
        let response = self.probe.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ClientError::http(status, text));
        }
        let body: Value = response.json().await?;
        let data = body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for item in data {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let aliases = item
                .get("aliases")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let n_ctx = item
                .get("meta")
                .and_then(|meta| meta.get("n_ctx"))
                .and_then(Value::as_u64)
                .or_else(|| item.get("n_ctx").and_then(Value::as_u64));
            out.push(ModelInfo { id, aliases, n_ctx });
        }
        Ok(out)
    }

    /// Server properties (`GET /props`), when supported.
    pub async fn props(&self) -> Result<Option<Value>, ClientError> {
        let url = self.base_url.join("props")?;
        let response = self.probe.get(url).send().await?;
        let status = response.status();
        if matches!(status.as_u16(), 404 | 501) {
            return Ok(None);
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ClientError::http(status, text));
        }
        Ok(Some(response.json().await?))
    }

    async fn probe_path(&self, path: &str) -> Result<EndpointProbe, ClientError> {
        let url = self.base_url.join(path.trim_start_matches('/'))?;
        let response = self.probe.get(url).send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        let body = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
        Ok(EndpointProbe {
            path: path.to_owned(),
            supported: !matches!(
                status,
                reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NOT_IMPLEMENTED
            ),
            status: status.as_u16(),
            body,
        })
    }

    /// Probe the attached server without modifying it.
    pub async fn probe(&self) -> Result<ProbeReport, ClientError> {
        let (health, models, props, slots, metrics) = tokio::join!(
            self.probe_path("health"),
            self.probe_path("v1/models"),
            self.probe_path("props"),
            self.probe_path("slots"),
            self.probe_path("metrics"),
        );
        Ok(ProbeReport {
            base_url: self.base_url.to_string(),
            health: health?,
            models: models?,
            props: props?,
            slots: slots?,
            metrics: metrics?,
        })
    }
}

/// A pinned byte stream (kept nameable for `impl Trait` ergonomics).
pub type PinBoxStream = std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, ClientError>> + Send>>;

#[derive(Debug, Clone, Serialize)]
pub struct EndpointProbe {
    pub path: String,
    pub supported: bool,
    pub status: u16,
    pub body: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeReport {
    pub base_url: String,
    pub health: EndpointProbe,
    pub models: EndpointProbe,
    pub props: EndpointProbe,
    pub slots: EndpointProbe,
    pub metrics: EndpointProbe,
}
