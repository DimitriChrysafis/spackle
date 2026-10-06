//! `spackle` — local llama.cpp coding-agent harness (CLI entry point).
//!
//! Phase 2 commands:
//! - `spackle doctor` — probe the attached server (non-mutating).
//! - `spackle ask` — single prompt against the model, streaming to the
//!   terminal, full config precedence and endpoint policy applied.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::Write;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use spackle_core::agent::{
    DenyAllGate, SamplingParams, StreamEvent, ToolDefinition, ToolRegistry, TurnContext,
};
use spackle_core::cancel::CancellationToken;
use spackle_core::config::Loader;
use spackle_core::config::generation::ReasoningEffort;
use spackle_core::message::Message;
use spackle_llamacpp::{LlamaCppClient, LlamaTransport};

#[derive(Debug, Parser)]
#[command(
    name = "spackle",
    version,
    about = "Local llama.cpp coding agent. Offline by design: no cloud, no telemetry.",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Probe the attached llama.cpp server without modifying it.
    Doctor(DoctorArgs),
    /// Ask the model a single question (streamed to the terminal).
    Ask(AskArgs),
}

#[derive(Debug, Parser)]
struct DoctorArgs {
    /// Endpoint base URL (defaults to config).
    #[arg(long, env = "SPACKLE_BASE_URL")]
    base_url: Option<String>,
    /// Permit a private-LAN endpoint (loopback is always allowed).
    #[arg(long)]
    allow_private_lan: bool,
}

#[derive(Debug, Parser)]
struct AskArgs {
    /// The question or instruction.
    prompt: Vec<String>,

    /// Model name or alias served by llama.cpp.
    #[arg(long)]
    model: Option<String>,
    /// Endpoint base URL (defaults to config).
    #[arg(long, env = "SPACKLE_BASE_URL")]
    base_url: Option<String>,
    /// Generation profile name (e.g. fast, balanced, deep).
    #[arg(long)]
    profile: Option<String>,
    /// Reasoning effort override (none, low, medium, high, xhigh).
    #[arg(long, value_enum)]
    reasoning: Option<ReasoningArg>,
    /// Permit a private-LAN endpoint (loopback is always allowed).
    #[arg(long)]
    allow_private_lan: bool,
    /// Extra system prompt prepended to any configured instructions.
    #[arg(long)]
    system: Option<String>,
    /// Show reasoning/thinking output as it streams.
    #[arg(long, default_value_t = false)]
    show_reasoning: bool,
    /// Maximum generated tokens for this call.
    #[arg(long)]
    max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ReasoningArg {
    None,
    Low,
    Medium,
    High,
    Xhigh,
}

impl ReasoningArg {
    fn to_effort(self) -> Option<ReasoningEffort> {
        match self {
            Self::None => None,
            Self::Low => Some(ReasoningEffort::Low),
            Self::Medium => Some(ReasoningEffort::Medium),
            Self::High => Some(ReasoningEffort::High),
            Self::Xhigh => Some(ReasoningEffort::Xhigh),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Doctor(args) => doctor(args).await,
        Command::Ask(args) => ask(args).await,
    }
}

async fn doctor(args: DoctorArgs) -> Result<()> {
    let loaded = load_config(args.base_url.as_deref(), args.allow_private_lan)?;
    let client = build_client(&loaded.config, args.allow_private_lan)?;
    let report = client
        .probe()
        .await
        .with_context(|| format!("probe of {} failed", client.base_url()))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

async fn ask(args: AskArgs) -> Result<()> {
    if args.prompt.is_empty() {
        bail!("`spackle ask` needs a prompt; e.g. `spackle ask \"explain this file\" src/main.rs`");
    }
    let prompt = args.prompt.join(" ");
    let loaded = load_config(args.base_url.as_deref(), args.allow_private_lan)?;
    for warning in &loaded.warnings {
        eprintln!("warning: {warning}");
    }

    let model = args
        .model
        .unwrap_or_else(|| loaded.config.endpoint.model.clone());
    let client = build_client(&loaded.config, args.allow_private_lan)?;
    let transport = LlamaTransport::new(client, model.clone());

    // Resolve the active profile (config + CLI overrides).
    let profile_name = args
        .profile
        .clone()
        .unwrap_or_else(|| loaded.config.generation.active_profile.clone());
    let mut sampling = match loaded.config.generation.profiles.get(&profile_name) {
        Some(profile) => SamplingParams::from(profile),
        None => {
            bail!(
                "unknown generation profile `{profile_name}`; available: {}",
                loaded
                    .config
                    .generation
                    .profiles
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    };
    if let Some(effort) = args.reasoning.map(ReasoningArg::to_effort) {
        sampling.reasoning_effort = effort;
    }
    if let Some(max_tokens) = args.max_tokens {
        sampling.max_tokens = max_tokens;
    }

    // System prompt: explicit flag first, then project instructions.
    let mut system = args.system.clone().unwrap_or_default();
    if let Some(instructions) = &loaded.instructions {
        if system.is_empty() {
            system = instructions.clone();
        } else {
            system = format!("{system}\n\n{instructions}");
        }
    }
    if system.is_empty() {
        system = "You are spackle, a careful local coding agent.".to_owned();
    }

    let registry = ToolRegistry::new();
    let gate = DenyAllGate;
    let context = TurnContext {
        transport: &transport,
        tools: &registry,
        gate: &gate,
        model: model.clone(),
        system_prompt: system,
        tools_schema: Vec::<ToolDefinition>::new(),
        sampling,
        config: (&loaded.config.agent).into(),
    };

    let show_reasoning = args.show_reasoning;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();

    // Stream deltas to the terminal as they arrive.
    let printer = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let mut out = std::io::stdout();
            match event {
                StreamEvent::TextDelta { text } => {
                    let _ = out.write_all(text.as_bytes());
                    let _ = out.flush();
                }
                StreamEvent::ReasoningDelta { text } => {
                    if show_reasoning {
                        let _ = out.write_all(format!("\u{1b}[2m{text}\u{1b}[0m").as_bytes());
                        let _ = out.flush();
                    }
                }
                StreamEvent::ToolCallDelta { name, .. } => {
                    if let Some(name) = name {
                        eprintln!("\n[tool call: {name}]");
                    }
                }
                StreamEvent::Usage(_) | StreamEvent::Timings(_) => {}
            }
        }
    });

    let cancel = CancellationToken::new();
    let mut transcript: Vec<Message> = Vec::new();
    let user = Message::user(&prompt);
    let outcome =
        spackle_core::agent::run_turn(&context, "ask", &mut transcript, user, cancel, tx).await;

    printer.await.expect("printer task must not panic");

    match outcome {
        Ok(outcome) => {
            println!();
            eprintln!(
                "({} step{})",
                outcome.steps,
                if outcome.steps == 1 { "" } else { "s" }
            );
            Ok(())
        }
        Err(agent_error) => {
            if agent_error.state == spackle_core::state::AgentState::Cancelling {
                eprintln!("cancelled");
                return Ok(());
            }
            bail!("{:?}: {}", agent_error.state, agent_error.reason);
        }
    }
}

fn load_config(
    base_url: Option<&str>,
    allow_private_lan: bool,
) -> Result<spackle_core::config::LoadedConfig> {
    let mut cli_overrides: BTreeMap<String, toml::Value> = BTreeMap::new();
    if let Some(base_url) = base_url {
        cli_overrides.insert(
            "endpoint.base_url".to_owned(),
            toml::Value::String(base_url.to_owned()),
        );
    }
    if allow_private_lan {
        cli_overrides.insert(
            "endpoint.allow_private_lan".to_owned(),
            toml::Value::Boolean(true),
        );
    }
    let start_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut loader = Loader::standard(start_dir);
    loader.cli_overrides = cli_overrides;
    loader
        .load()
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

fn build_client(
    config: &spackle_core::config::Config,
    allow_private_lan: bool,
) -> Result<LlamaCppClient> {
    LlamaCppClient::with_policy(&config.endpoint.base_url, allow_private_lan)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}
