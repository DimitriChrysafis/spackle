//! `spackle` — local llama.cpp coding-agent harness (CLI entry point).
//!
//! Commands:
//! - `spackle doctor` — probe the attached server (non-mutating).
//! - `spackle ask` — run one agent turn with the workspace tools, streaming
//!   to the terminal, with approval prompts and a session journal.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use spackle_core::agent::{
    AllowAllGate, ApprovalDecision, ApprovalGate, DenyAllGate, SamplingParams, StreamEvent,
    ToolDefinition, ToolRegistry, TurnContext, run_turn,
};
use spackle_core::cancel::CancellationToken;
use spackle_core::config::Loader;
use spackle_core::config::agent::{ConfirmationMode, ConfirmationPolicy};
use spackle_core::config::generation::ReasoningEffort;
use spackle_core::event::AgentEvent;
use spackle_core::message::{ContentBlock, Message};
use spackle_core::prompt::{SystemPrompt, SystemPromptInput};
use spackle_core::session::{
    SessionId, SessionPaths, SessionRecord, SessionRecordKind, SessionStore,
};
use spackle_llamacpp::{LlamaCppClient, LlamaTransport};
use spackle_tools::{WorkspaceRoot, standard_registry, standard_schemas};

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
pub(crate) struct EndpointFlags {
    /// Endpoint base URL (defaults to config).
    #[arg(long, env = "SPACKLE_BASE_URL")]
    base_url: Option<String>,
    /// Request dialect: llamacpp extensions or plain OpenAI fields.
    #[arg(long, value_enum)]
    api: Option<ApiArg>,
    /// Environment variable holding a bearer token for the endpoint.
    #[arg(long)]
    api_key_env: Option<String>,
    /// Permit a private-LAN endpoint (loopback is always allowed).
    #[arg(long)]
    allow_private_lan: bool,
    /// Permit a public https endpoint (off by default: spackle is local-first).
    #[arg(long)]
    allow_public_endpoint: bool,
}

impl EndpointFlags {
    fn overrides(&self) -> BTreeMap<String, toml::Value> {
        let mut out: BTreeMap<String, toml::Value> = BTreeMap::new();
        if let Some(base_url) = &self.base_url {
            out.insert(
                "endpoint.base_url".to_owned(),
                toml::Value::String(base_url.clone()),
            );
        }
        if let Some(api) = self.api {
            out.insert(
                "endpoint.api".to_owned(),
                toml::Value::String(api.as_config_str().to_owned()),
            );
        }
        if let Some(var) = &self.api_key_env {
            out.insert(
                "endpoint.api_key_env".to_owned(),
                toml::Value::String(var.clone()),
            );
        }
        if self.allow_private_lan {
            out.insert(
                "endpoint.allow_private_lan".to_owned(),
                toml::Value::Boolean(true),
            );
        }
        if self.allow_public_endpoint {
            out.insert(
                "endpoint.allow_public_endpoint".to_owned(),
                toml::Value::Boolean(true),
            );
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ApiArg {
    Llamacpp,
    Openai,
}

impl ApiArg {
    fn as_config_str(self) -> &'static str {
        match self {
            Self::Llamacpp => "llamacpp",
            Self::Openai => "openai",
        }
    }
}

#[derive(Debug, Parser)]
struct DoctorArgs {
    #[command(flatten)]
    endpoint: EndpointFlags,
}

#[derive(Debug, Parser)]
struct AskArgs {
    /// The question or instruction.
    prompt: Vec<String>,

    #[command(flatten)]
    endpoint: EndpointFlags,
    /// Workspace directory to sandbox tools into (default: config or cwd).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Model name or alias served by the endpoint.
    #[arg(long)]
    model: Option<String>,
    /// Generation profile name (e.g. fast, balanced, deep).
    #[arg(long)]
    profile: Option<String>,
    /// Reasoning effort override (none, low, medium, high, xhigh).
    #[arg(long, value_enum)]
    reasoning: Option<ReasoningArg>,
    /// Extra system prompt prepended to any configured instructions.
    #[arg(long)]
    system: Option<String>,
    /// Show reasoning/thinking output as it streams.
    #[arg(long, default_value_t = false)]
    show_reasoning: bool,
    /// Maximum generated tokens for this call.
    #[arg(long)]
    max_tokens: Option<u32>,
    /// Approve every tool action without prompting (use with care).
    #[arg(long)]
    yes: bool,
    /// Disable tools entirely (plain chat).
    #[arg(long)]
    no_tools: bool,
    /// Do not write a session journal.
    #[arg(long)]
    no_journal: bool,
    /// Continue the existing session journal for this workspace.
    #[arg(long)]
    resume: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ReasoningArg {
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
    let loaded = load_config(&args.endpoint)?;
    let client = build_client(&loaded.config)?;
    let report = client
        .probe()
        .await
        .with_context(|| format!("probe of {} failed", client.base_url()))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

/// Approval gate that maps the tool `describe` prefix onto the configured
/// confirmation policy and asks on the terminal when the mode is `ask`.
struct PolicyGate {
    policy: ConfirmationPolicy,
    interactive: bool,
}

impl PolicyGate {
    fn mode_for(&self, detail: &str) -> ConfirmationMode {
        let kind = detail.split(':').next().unwrap_or("");
        match kind {
            "secret" => self.policy.secret_files,
            "command-destructive" => self.policy.destructive,
            "command-network" => self.policy.network_commands,
            _ => self.policy.writes,
        }
    }
}

impl ApprovalGate for PolicyGate {
    fn decide(&self, tool: &str, detail: &str) -> ApprovalDecision {
        match self.mode_for(detail) {
            ConfirmationMode::Allow => ApprovalDecision::Allow,
            ConfirmationMode::Deny => ApprovalDecision::Deny,
            ConfirmationMode::Ask => {
                if !self.interactive {
                    return ApprovalDecision::Deny;
                }
                eprint!("\n[approve {tool}] {detail}\nallow? [y/N] ");
                let _ = std::io::stderr().flush();
                let mut line = String::new();
                match std::io::stdin().read_line(&mut line) {
                    Ok(_) => match line.trim().to_lowercase().as_str() {
                        "y" | "yes" => ApprovalDecision::Allow,
                        _ => ApprovalDecision::Deny,
                    },
                    Err(_) => ApprovalDecision::Deny,
                }
            }
        }
    }
}

async fn ask(args: AskArgs) -> Result<()> {
    if args.prompt.is_empty() {
        bail!("`spackle ask` needs a prompt; e.g. `spackle ask \"explain this file\"`");
    }
    let prompt = args.prompt.join(" ");
    let loaded = load_config(&args.endpoint)?;
    for warning in &loaded.warnings {
        eprintln!("warning: {warning}");
    }

    let model = args
        .model
        .clone()
        .unwrap_or_else(|| loaded.config.endpoint.model.clone());
    let client = build_client(&loaded.config)?;
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

    // Workspace: flag, then config, then current directory.
    let workspace = match (&args.workspace, &loaded.config.agent.workspace) {
        (Some(path), _) => path.clone(),
        (None, Some(path)) => PathBuf::from(path),
        (None, None) => std::env::current_dir().context("cannot determine current directory")?,
    };
    let root = WorkspaceRoot::new(&workspace)
        .with_context(|| format!("workspace {} is not usable", workspace.display()))?;

    let (registry, tools_schema) = if args.no_tools {
        (ToolRegistry::new(), Vec::<ToolDefinition>::new())
    } else {
        (standard_registry(root.clone()), standard_schemas())
    };

    // System prompt: assembled prompt + optional extra instructions.
    let assembled = SystemPrompt::assemble(&SystemPromptInput {
        project_instructions: loaded.instructions.clone(),
        workspace: Some(root.as_path().to_string_lossy().into_owned()),
        model: Some(model.clone()),
        platform: Some(std::env::consts::OS.to_owned()),
        profile: Some(profile_name.clone()),
        toolchain: detect_toolchain(),
    });
    let mut system = args.system.clone().unwrap_or_default();
    if system.is_empty() {
        system = assembled.as_str().to_owned();
    } else {
        system = format!("{system}\n\n{}", assembled.as_str());
    }

    let interactive = stderr_is_terminal();
    let gate: Box<dyn ApprovalGate> = if args.yes {
        Box::new(AllowAllGate)
    } else if interactive {
        Box::new(PolicyGate {
            policy: loaded.config.agent.confirmations,
            interactive: true,
        })
    } else {
        Box::new(DenyAllGate)
    };

    let context = TurnContext {
        transport: &transport,
        tools: &registry,
        gate: gate.as_ref(),
        model: model.clone(),
        system_prompt: system,
        tools_schema,
        sampling,
        config: (&loaded.config.agent).into(),
    };

    // Session journal under the state directory.
    let session_id = SessionId::from_workspace(root.as_path());
    let paths = SessionPaths::from_state_dir(&loaded.state_dir);
    let mut store = if args.no_journal {
        None
    } else {
        match SessionStore::open(&paths.sessions_dir, session_id.clone()) {
            Ok(store) => Some(store),
            Err(error) => {
                eprintln!("warning: session journal unavailable: {error}");
                None
            }
        }
    };
    let mut transcript: Vec<Message> = Vec::new();
    if args.resume
        && let Some(store) = &store
    {
        match store.load_transcript() {
            Ok(prior) => {
                if !prior.messages.is_empty() {
                    eprintln!(
                        "(resuming {} prior messages from {})",
                        prior.messages.len(),
                        store.path().display()
                    );
                }
                transcript = prior.messages;
            }
            Err(error) => eprintln!("warning: cannot resume session: {error}"),
        }
    }
    journal_message(&mut store, &session_id, "user", &prompt, &model);

    let show_reasoning = args.show_reasoning;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
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
    {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancel.cancel();
        });
    }

    let baseline = transcript.len();
    let outcome = run_turn(
        &context,
        session_id.as_str(),
        &mut transcript,
        Message::user(&prompt),
        cancel,
        tx,
    )
    .await;

    printer.await.expect("printer task must not panic");

    // Journal everything the turn appended, then the outcome events.
    for message in &transcript[baseline..] {
        journal_blocks(&mut store, &session_id, message, &model);
    }
    let events = match &outcome {
        Ok(outcome) => outcome.events.clone(),
        Err(error) => error.events.clone(),
    };
    journal_events(&mut store, &session_id, &events, &model);
    if let (Some(store), Ok(outcome)) = (&mut store, &outcome) {
        let _ = store.checkpoint(&spackle_core::message::Transcript {
            messages: transcript.clone(),
        });
        let _ = outcome;
    }

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

fn journal_message(
    store: &mut Option<SessionStore>,
    session_id: &SessionId,
    role: &str,
    content: &str,
    model: &str,
) {
    let Some(store) = store else { return };
    let mut record = SessionRecord::new(session_id, SessionRecordKind::Message);
    record.role = Some(role.to_owned());
    record.content = Some(content.to_owned());
    record.model = Some(model.to_owned());
    let _ = store.append(record);
}

fn journal_blocks(
    store: &mut Option<SessionStore>,
    session_id: &SessionId,
    message: &Message,
    model: &str,
) {
    let Some(store) = store else { return };
    for block in &message.blocks {
        let mut record = match block {
            ContentBlock::ToolCall(call) => {
                let mut record = SessionRecord::new(session_id, SessionRecordKind::ToolCall);
                record.call_id = Some(call.id.clone());
                record.tool = Some(call.name.clone());
                record.arguments = Some(call.arguments.clone());
                record
            }
            ContentBlock::ToolResult(result) => {
                let mut record = SessionRecord::new(session_id, SessionRecordKind::ToolResult);
                record.call_id = Some(result.id.clone());
                record.tool = Some(result.name.clone());
                record.result = Some(serde_json::json!({
                    "output": result.output,
                    "is_error": result.is_error,
                }));
                record
            }
            ContentBlock::Text { text } => {
                let mut record = SessionRecord::new(session_id, SessionRecordKind::Message);
                record.role = Some(
                    match message.role {
                        spackle_core::message::Role::Assistant => "assistant",
                        spackle_core::message::Role::User => "user",
                        spackle_core::message::Role::System => "system",
                        spackle_core::message::Role::Tool => "tool",
                    }
                    .to_owned(),
                );
                record.content = Some(text.clone());
                record
            }
            ContentBlock::Reasoning { .. } => continue,
        };
        record.model = Some(model.to_owned());
        let _ = store.append(record);
    }
}

fn journal_events(
    store: &mut Option<SessionStore>,
    session_id: &SessionId,
    events: &[AgentEvent],
    model: &str,
) {
    let Some(store) = store else { return };
    for event in events {
        let mut record = SessionRecord::new(session_id, SessionRecordKind::Meta);
        record.extra = Some(serde_json::json!({
            "kind": event.kind.as_str(),
            "payload": event.payload,
        }));
        record.model = Some(model.to_owned());
        let _ = store.append(record);
    }
}

pub(crate) fn load_config(flags: &EndpointFlags) -> Result<spackle_core::config::LoadedConfig> {
    let start_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut loader = Loader::standard(start_dir);
    loader.cli_overrides = flags.overrides();
    loader
        .load()
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub(crate) fn build_client(config: &spackle_core::config::Config) -> Result<LlamaCppClient> {
    LlamaCppClient::from_endpoint_config(&config.endpoint)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

/// Whether stderr is a terminal (approval prompts only make sense then).
fn stderr_is_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}

/// Detected build/test toolchains, rendered for the environment prompt.
pub(crate) fn detect_toolchain() -> Option<String> {
    let candidates = [
        "python3", "python", "node", "deno", "cargo", "rustc", "go", "javac", "gcc", "clang",
        "make", "git", "sh", "bash",
    ];
    let found: Vec<&str> = candidates
        .iter()
        .copied()
        .filter(|name| which::which(name).is_ok())
        .collect();
    (!found.is_empty()).then(|| found.join(", "))
}
