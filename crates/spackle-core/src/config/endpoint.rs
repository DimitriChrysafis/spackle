//! Endpoint policy: llama.cpp must be reachable on loopback by default, on a
//! private LAN only when explicitly allowed, and never on public endpoints.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use url::Url;

use super::error::{ConfigError, Issue};

/// Server connection mode.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EndpointMode {
    /// Connect to an already-running llama.cpp server. The server is never
    /// restarted or reconfigured in this mode.
    #[default]
    Attach,
    /// Launch and supervise a user-selected llama-server child process.
    Managed,
}

/// Endpoint and model selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct EndpointConfig {
    pub mode: EndpointMode,
    /// Base URL of the llama.cpp server, e.g. `http://127.0.0.1:8080`.
    pub base_url: String,
    /// Model name or alias served by llama.cpp.
    pub model: String,
    /// Permit private-LAN (non-loopback, non-public) endpoints.
    pub allow_private_lan: bool,
    /// Must remain false. Public inference endpoints are always rejected.
    pub allow_public_endpoint: bool,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            mode: EndpointMode::default(),
            base_url: super::DEFAULT_BASE_URL.to_owned(),
            model: super::DEFAULT_MODEL.to_owned(),
            allow_private_lan: false,
            allow_public_endpoint: false,
        }
    }
}

/// Where an endpoint host lives, relative to this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointClass {
    Loopback,
    PrivateLan,
    Public,
}

/// A parsed endpoint together with its policy class.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct EndpointInfo {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub class: EndpointClass,
}

impl EndpointInfo {
    /// Parse and classify `base_url`. Returns issues when it is not a usable
    /// local llama.cpp endpoint.
    pub fn parse(base_url: &str) -> Result<EndpointInfo, Issue> {
        let url = Url::parse(base_url).map_err(|err| {
            Issue::new("endpoint.base_url", format!("not a valid URL ({err})"))
                .with_hint("use an absolute URL such as http://127.0.0.1:8080")
        })?;
        match url.scheme() {
            "http" | "https" => {}
            other => {
                return Err(Issue::new(
                    "endpoint.base_url",
                    format!("unsupported scheme `{other}`"),
                )
                .with_hint("llama.cpp serves HTTP; use http:// (or https:// for TLS builds)"));
            }
        }
        let host = url
            .host_str()
            .ok_or_else(|| {
                Issue::new("endpoint.base_url", "missing host")
                    .with_hint("include a host, e.g. http://127.0.0.1:8080")
            })?
            .to_owned();
        let port = url
            .port()
            .unwrap_or(if url.scheme() == "https" { 443 } else { 80 });
        let class = classify_host(&host);
        Ok(EndpointInfo {
            url: base_url.to_owned(),
            host,
            port,
            class,
        })
    }
}

fn classify_host(host: &str) -> EndpointClass {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host == "localhost" {
        return EndpointClass::Loopback;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            if is_loopback_v4(ip) {
                EndpointClass::Loopback
            } else if is_private_v4(ip) {
                EndpointClass::PrivateLan
            } else {
                EndpointClass::Public
            }
        }
        Ok(IpAddr::V6(ip)) => {
            if ip.is_loopback() {
                EndpointClass::Loopback
            } else if is_private_v6(ip) {
                EndpointClass::PrivateLan
            } else {
                EndpointClass::Public
            }
        }
        Err(_) => EndpointClass::Public,
    }
}

fn is_loopback_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified()
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    (octets[0] == 10)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 169 && octets[1] == 254)
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let first = segments[0];
    (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
}

/// Validate endpoint policy for one configuration.
pub fn validate(config: &EndpointConfig) -> Result<(), ConfigError> {
    let mut issues = Vec::new();
    let endpoint = EndpointInfo::parse(&config.base_url);
    if let Err(issue) = &endpoint {
        issues.push(issue.clone());
    }
    if config.allow_public_endpoint {
        issues.push(
            Issue::new(
                "endpoint.allow_public_endpoint",
                "public inference endpoints are not permitted",
            )
            .with_hint("remove allow_public_endpoint; spackle only talks to local servers"),
        );
    }
    if let Ok(info) = &endpoint {
        match (info.class, config.allow_private_lan) {
            (EndpointClass::Public, _) => issues.push(
                Issue::new(
                    "endpoint.base_url",
                    format!("{} resolves to a public endpoint", info.host),
                )
                .with_hint(
                    "point spackle at a loopback server (http://127.0.0.1:PORT) or \
                     enable a private-LAN endpoint with endpoint.allow_private_lan = true",
                ),
            ),
            (EndpointClass::PrivateLan, false) => issues.push(
                Issue::new(
                    "endpoint.base_url",
                    format!("{} is a private-LAN endpoint", info.host),
                )
                .with_hint("set endpoint.allow_private_lan = true to permit this endpoint"),
            ),
            _ => {}
        }
    }
    if config.model.trim().is_empty() {
        issues.push(
            Issue::new("endpoint.model", "must not be empty")
                .with_hint("set the model alias reported by GET /v1/models"),
        );
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(ConfigError::new(issues))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(base_url: &str) -> EndpointConfig {
        EndpointConfig {
            base_url: base_url.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn loopback_is_allowed_by_default() {
        for url in [
            "http://127.0.0.1:8080",
            "http://localhost:8080",
            "http://[::1]:8080",
            "http://127.5.6.7:8080",
        ] {
            let config = endpoint(url);
            assert!(
                validate(&config).is_ok(),
                "{url} should be allowed: {:?}",
                validate(&config).err()
            );
        }
    }

    #[test]
    fn private_lan_requires_explicit_opt_in() {
        let config = endpoint("http://192.168.1.20:8080");
        let error = validate(&config).expect_err("LAN endpoint must need opt-in");
        assert!(error.to_string().contains("allow_private_lan"));

        let mut allowed = config.clone();
        allowed.allow_private_lan = true;
        assert!(validate(&allowed).is_ok());
    }

    #[test]
    fn lan_ranges_are_recognized() {
        for url in [
            "http://10.0.0.5:8080",
            "http://172.16.0.1:8080",
            "http://172.31.255.255:8080",
            "http://192.168.0.1:8080",
            "http://169.254.10.1:8080",
            "http://[fd12:3456::1]:8080",
            "http://[fe80::1]:8080",
        ] {
            let config = endpoint(url);
            let error = validate(&config).expect_err("must require opt-in");
            assert!(
                error.to_string().contains("private-LAN"),
                "{url} should classify as private LAN, got: {error}"
            );
        }
        // Just outside the ranges.
        for url in [
            "http://172.15.0.1:8080",
            "http://172.32.0.1:8080",
            "http://9.0.0.1:8080",
        ] {
            let info = EndpointInfo::parse(url).expect("parses");
            assert_eq!(info.class, EndpointClass::Public, "{url}");
        }
    }

    #[test]
    fn public_endpoints_are_always_rejected() {
        for url in [
            "http://203.0.113.10:8080",
            "http://model-host.example.com:8080",
            "http://[2001:db8::1]:8080",
        ] {
            let mut config = endpoint(url);
            let error = validate(&config).expect_err("public endpoint must be rejected");
            assert!(error.to_string().contains("public"), "{url}: {error}");
            config.allow_private_lan = true;
            assert!(
                validate(&config).is_err(),
                "allow_private_lan must not unlock public endpoints"
            );
        }
    }

    #[test]
    fn allow_public_endpoint_flag_is_rejected() {
        let mut config = endpoint("http://127.0.0.1:8080");
        config.allow_public_endpoint = true;
        let error = validate(&config).expect_err("flag must be rejected");
        assert!(error.to_string().contains("allow_public_endpoint"));
    }

    #[test]
    fn scheme_and_host_are_validated() {
        let error = validate(&endpoint("ftp://127.0.0.1")).expect_err("bad scheme");
        assert!(error.to_string().contains("scheme"));
        let error = validate(&endpoint("not a url")).expect_err("bad url");
        assert!(error.to_string().contains("endpoint.base_url"));
    }
}
