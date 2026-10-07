//! `spackle eval` — run the task benchmark against the configured endpoint.
//!
//! Each task directory holds a `task.toml` (`prompt`, `check`, optional
//! `timeout_seconds`) plus a `repo/` fixture copied into a fresh temp dir per
//! run. A run is one agent turn with the standard tools; afterwards `check`
//! is executed in the temp dir and a zero exit status counts as solved.
//! Results go to a JSON report; a table is printed at the end.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};
use spackle_core::agent::{
    AllowAllGate, LoopConfig, SamplingParams, StreamEvent, TurnContext, run_turn,
};
use spackle_core::cancel::CancellationToken;
use spackle_core::event::EventKind;
use spackle_core::message::Message;
use spackle_core::prompt::{SystemPrompt, SystemPromptInput};
use spackle_llamacpp::LlamaTransport;
use spackle_tools::{RegistryOptions, WorkspaceRoot, standard_registry_with, standard_schemas};

use crate::{EndpointFlags, ReasoningArg, build_client, detect_toolchain, load_config};

/// Default per-task turn budget when `task.toml` does not set one.
const DEFAULT_TASK_TIMEOUT_SECONDS: u64 = 300;
/// Per-check subprocess budget (fix verification should be fast).
const CHECK_TIMEOUT_SECONDS: u64 = 120;

#[derive(Debug, Parser)]
pub(crate) struct EvalArgs {
    /// Directory of benchmark tasks (each subdir: task.toml + repo/).
    #[arg(long)]
    pub tasks: PathBuf,

    /// Runs per task (3+ gives a meaningful solve rate).
    #[arg(long, default_value_t = 3)]
    pub runs: u32,

    /// Concurrent runs (each is an independent workspace copy).
    #[arg(long, default_value_t = 1)]
    pub jobs: usize,

    /// Only run tasks whose directory name contains this string.
    #[arg(long)]
    pub only: Option<String>,

    /// Model name or alias served by the endpoint.
    #[arg(long)]
    pub model: Option<String>,

    /// Generation profile name (e.g. fast, balanced, deep).
    #[arg(long)]
    pub profile: Option<String>,

    /// Reasoning effort override (none, low, medium, high, xhigh).
    #[arg(long, value_enum)]
    pub reasoning: Option<ReasoningArg>,

    /// JSON report output path (default: <tasks>/../results/eval-<ts>.json).
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Per-task turn time budget in seconds (overrides task.toml).
    #[arg(long)]
    pub timeout_seconds: Option<u64>,

    /// Max agent steps per run.
    #[arg(long)]
    pub max_steps: Option<u32>,

    /// Transient-error retries per inference call (with backoff). A local
    /// engine restart takes a while; the default forgives that.
    #[arg(long, default_value_t = 6)]
    pub retries: u32,

    /// Ablation: disable the repeated-call loop guard.
    #[arg(long)]
    pub no_loop_guard: bool,

    /// Ablation: allow edits to files the model never read.
    #[arg(long)]
    pub no_read_guard: bool,

    /// Ablation: do not truncate tool output.
    #[arg(long)]
    pub no_output_cap: bool,

    /// Print each task's protected (non-editable) files and exit.
    #[arg(long)]
    pub list_protected: bool,

    #[command(flatten)]
    pub endpoint: EndpointFlags,
}

#[derive(Debug, Deserialize)]
struct TaskSpec {
    /// Instruction handed to the agent as the user message.
    prompt: String,
    /// Shell command run in the workspace copy; exit 0 means solved.
    check: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
    /// Extra globs of files the agent may not touch. Merged with the
    /// built-in test defaults.
    #[serde(default)]
    protect: Vec<String>,
}

/// Files that count as "the grader" and must match the fixture byte-for-byte
/// when the check runs.
const DEFAULT_PROTECT: &[&str] = &[
    "tests/**",
    "test/**",
    "**/test_*.py",
    "**/*_test.py",
    "**/*.test.js",
    "**/*.test.ts",
    "**/*.spec.js",
    "**/*.spec.ts",
    "**/conftest.py",
    "**/check.py",
    "**/run_tests*.sh",
    "**/check.sh",
    "**/test.sh",
];

#[derive(Debug)]
struct Task {
    name: String,
    repo_dir: PathBuf,
    spec: TaskSpec,
    protect: globset::GlobSet,
}

/// Compile a task's protect globs (defaults + task.toml extras).
fn protect_set(extra: &[String]) -> Result<globset::GlobSet> {
    let mut builder = globset::GlobSetBuilder::new();
    let mut add = |pattern: &str| -> Result<()> {
        builder.add(
            globset::Glob::new(pattern).with_context(|| format!("bad protect glob `{pattern}`"))?,
        );
        Ok(())
    };
    for pattern in DEFAULT_PROTECT {
        add(pattern)?;
    }
    for pattern in extra {
        add(pattern)?;
    }
    Ok(builder.build()?)
}

#[derive(Debug, Serialize)]
struct RunRecord {
    task: String,
    run: u32,
    solved: bool,
    steps: u32,
    tool_calls: u32,
    /// Calls to unknown tools or with arguments that never parsed as JSON.
    invalid_tool_calls: u32,
    /// Tool executions that returned is_error (includes failed commands).
    tool_errors: u32,
    prompt_tokens: u64,
    completion_tokens: u64,
    wall_ms: u64,
    /// True when protected (test) files differed from the fixture.
    #[serde(default)]
    tampered: bool,
    /// Agent-side failure reason, if the turn did not complete.
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct TaskSummary {
    task: String,
    runs: u32,
    solved: u32,
    solve_rate: f64,
    mean_steps: f64,
    mean_tool_calls: f64,
    mean_completion_tokens: f64,
    mean_wall_s: f64,
    timeouts: u32,
    loop_aborts: u32,
    /// Runs where protected files were modified, deleted, or added.
    tampered: u32,
}

#[derive(Debug, Serialize)]
struct Report {
    meta: serde_json::Value,
    runs: Vec<RunRecord>,
    summary: Vec<TaskSummary>,
}

pub async fn run(args: EvalArgs) -> Result<()> {
    let tasks = load_tasks(&args.tasks, args.only.as_deref())?;
    if tasks.is_empty() {
        bail!("no tasks under {}", args.tasks.display());
    }

    if args.list_protected {
        for task in &tasks {
            let files = collect_protected(&task.repo_dir, &task.protect)?;
            println!("{} ({} protected)", task.name, files.len());
            for path in files.keys() {
                println!("  {}", path.display());
            }
        }
        return Ok(());
    }

    let loaded = load_config(&args.endpoint)?;
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| loaded.config.endpoint.model.clone());
    let client = build_client(&loaded.config)?;
    let transport = LlamaTransport::new(client, model.clone());

    let profile_name = args
        .profile
        .clone()
        .unwrap_or_else(|| loaded.config.generation.active_profile.clone());
    let mut sampling = match loaded.config.generation.profiles.get(&profile_name) {
        Some(profile) => SamplingParams::from(profile),
        None => bail!("unknown generation profile `{profile_name}`"),
    };
    if let Some(effort) = args.reasoning.and_then(ReasoningArg::to_effort) {
        sampling.reasoning_effort = Some(effort);
    }

    let mut loop_config = LoopConfig::from(&loaded.config.agent);
    loop_config.max_model_retries = args.retries;
    if let Some(max_steps) = args.max_steps {
        loop_config.max_steps = max_steps;
    }
    if args.no_loop_guard {
        loop_config.repeated_call_limit = u32::MAX;
    }
    if args.no_output_cap {
        loop_config.max_tool_output_bytes = u64::MAX;
    }
    let toolchain = detect_toolchain();
    let schemas = standard_schemas();
    let known_tools: std::collections::HashSet<&str> =
        schemas.iter().map(|t| t.name.as_str()).collect();

    eprintln!(
        "eval: {} tasks x {} runs, model={model}, profile={profile_name}",
        tasks.len(),
        args.runs
    );

    // Shadow owned values with shared refs so `async move` copies the
    // reference instead of moving the value.
    let transport = &transport;
    let schemas = &schemas;
    let known_tools = &known_tools;
    let loop_config = &loop_config;
    let sampling = &sampling;
    let model = model.as_str();
    let profile_name = profile_name.as_str();
    let toolchain = toolchain.as_deref();
    let args = &args;

    let mut pending: Vec<(usize, u32)> = Vec::new();
    for (task_idx, _) in tasks.iter().enumerate() {
        for run_idx in 0..args.runs {
            pending.push((task_idx, run_idx));
        }
    }
    let mut records: Vec<RunRecord> = Vec::new();
    let mut in_flight = futures_util::stream::FuturesUnordered::new();
    let mut next = 0_usize;
    loop {
        while next < pending.len() && in_flight.len() < args.jobs.max(1) {
            let (task_idx, run_idx) = pending[next];
            next += 1;
            let task = &tasks[task_idx];
            in_flight.push(async move {
                (
                    task_idx,
                    run_idx,
                    run_one(
                        task,
                        run_idx,
                        transport,
                        schemas,
                        known_tools,
                        loop_config,
                        sampling,
                        model,
                        profile_name,
                        toolchain,
                        args,
                    )
                    .await,
                )
            });
        }
        let Some((task_idx, run_idx, record)) = futures_util::StreamExt::next(&mut in_flight).await
        else {
            break;
        };
        let task = &tasks[task_idx];
        match &record {
            Ok(record) => {
                eprintln!(
                    "  {}#{run_idx}: {} ({} steps, {} tool calls, {:.1}s)",
                    task.name,
                    if record.solved { "PASS" } else { "FAIL" },
                    record.steps,
                    record.tool_calls,
                    record.wall_ms as f64 / 1000.0,
                );
            }
            Err(error) => {
                eprintln!("  {}#{run_idx}: harness error: {error}", task.name);
            }
        }
        records.push(record.unwrap_or_else(|error| RunRecord {
            task: task.name.clone(),
            run: run_idx,
            solved: false,
            steps: 0,
            tool_calls: 0,
            invalid_tool_calls: 0,
            tool_errors: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            wall_ms: 0,
            tampered: false,
            error: Some(format!("harness: {error}")),
        }));
    }
    // Keep the printed/report order deterministic regardless of finish order.
    let index_of = |record: &RunRecord| {
        tasks
            .iter()
            .position(|t| t.name == record.task)
            .unwrap_or(usize::MAX)
    };
    records.sort_by_key(|r| (index_of(r), r.run));

    let summary = summarize(&records);
    let report = Report {
        meta: serde_json::json!({
            "model": model.to_owned(),
            "profile": profile_name.to_owned(),
            "runs_per_task": args.runs,
            "no_loop_guard": args.no_loop_guard,
            "no_read_guard": args.no_read_guard,
            "no_output_cap": args.no_output_cap,
            "tasks": tasks.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
        }),
        runs: records,
        summary,
    };

    print_table(&report);
    let out = args.out.clone().unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        args.tasks
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("results")
            .join(format!("eval-{model}-{stamp}.json"))
    });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out, serde_json::to_string_pretty(&report)?)?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_one(
    task: &Task,
    run_idx: u32,
    transport: &LlamaTransport,
    schemas: &[spackle_core::agent::ToolDefinition],
    known_tools: &std::collections::HashSet<&str>,
    loop_config: &LoopConfig,
    sampling: &SamplingParams,
    model: &str,
    profile_name: &str,
    toolchain: Option<&str>,
    args: &EvalArgs,
) -> Result<RunRecord> {
    let workdir = tempfile::tempdir().context("tempdir for task copy")?;
    copy_dir(&task.repo_dir, workdir.path())?;
    let root = WorkspaceRoot::new(workdir.path()).context("workspace root")?;

    let registry = standard_registry_with(
        root.clone(),
        RegistryOptions {
            read_guard: !args.no_read_guard,
        },
    );

    let system = SystemPrompt::assemble(&SystemPromptInput {
        project_instructions: None,
        workspace: Some(root.as_path().to_string_lossy().into_owned()),
        model: Some(model.to_owned()),
        platform: Some(std::env::consts::OS.to_owned()),
        profile: Some(profile_name.to_owned()),
        toolchain: toolchain.map(str::to_owned),
    });
    let gate = AllowAllGate;
    let mut config = *loop_config;
    config.max_turn_seconds = args
        .timeout_seconds
        .or(task.spec.timeout_seconds)
        .unwrap_or(DEFAULT_TASK_TIMEOUT_SECONDS);

    let context = TurnContext {
        transport,
        tools: &registry,
        gate: &gate,
        model: model.to_owned(),
        system_prompt: system.as_str().to_owned(),
        tools_schema: schemas.to_vec(),
        sampling: sampling.clone(),
        config,
    };

    // run_turn requires a stream sink; drain it quietly.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let started = Instant::now();
    let mut transcript: Vec<Message> = Vec::new();
    let session = format!("eval-{}-{}", task.name, run_idx);
    let outcome = run_turn(
        &context,
        &session,
        &mut transcript,
        Message::user(&task.spec.prompt),
        CancellationToken::new(),
        tx,
    )
    .await;
    let wall_ms = started.elapsed().as_millis() as u64;
    drain.abort();

    let (steps, events, error) = match &outcome {
        Ok(outcome) => (outcome.steps, &outcome.events, None),
        Err(error) => (
            error
                .events
                .iter()
                .filter(|e| e.kind == EventKind::InferenceStarted)
                .count() as u32,
            &error.events,
            Some(error.reason.clone()),
        ),
    };

    let mut prompt_tokens = 0_u64;
    let mut completion_tokens = 0_u64;
    let mut tool_errors = 0_u32;
    for event in events {
        match event.kind {
            EventKind::Telemetry => {
                prompt_tokens += event.payload["prompt_tokens"].as_u64().unwrap_or(0);
                completion_tokens += event.payload["completion_tokens"].as_u64().unwrap_or(0);
            }
            EventKind::ToolCallFinished => {
                if event.payload["is_error"].as_bool().unwrap_or(false) {
                    tool_errors += 1;
                }
            }
            _ => {}
        }
    }

    let mut tool_calls = 0_u32;
    let mut invalid_tool_calls = 0_u32;
    for message in &transcript {
        for call in message.tool_calls() {
            tool_calls += 1;
            // A String payload means the streamed arguments never parsed
            // as JSON; a missing name means the model invented a tool.
            if call.arguments.is_string() || !known_tools.contains(call.name.as_str()) {
                invalid_tool_calls += 1;
            }
        }
    }

    let tampered = match tampered_paths(&task.repo_dir, workdir.path(), &task.protect) {
        Ok(paths) if paths.is_empty() => None,
        Ok(paths) => Some(paths),
        Err(error) => {
            return Ok(RunRecord {
                task: task.name.clone(),
                run: run_idx,
                solved: false,
                steps,
                tool_calls,
                invalid_tool_calls,
                tool_errors,
                prompt_tokens,
                completion_tokens,
                wall_ms,
                tampered: false,
                error: Some(format!("protect scan: {error}")),
            });
        }
    };
    if tampered.is_some() {
        restore_protected(&task.repo_dir, workdir.path(), &task.protect)?;
    }

    let check = run_check(&task.spec.check, workdir.path(), CHECK_TIMEOUT_SECONDS).await;
    let solved = tampered.is_none() && matches!(check, Ok(true));

    Ok(RunRecord {
        task: task.name.clone(),
        run: run_idx,
        solved,
        steps,
        tool_calls,
        invalid_tool_calls,
        tool_errors,
        prompt_tokens,
        completion_tokens,
        wall_ms,
        tampered: tampered.is_some(),
        error: match tampered {
            Some(paths) => Some(format!("tampered: {}", paths.join(", "))),
            None => error.or(match check {
                Err(error) => Some(format!("check harness: {error}")),
                Ok(_) => None,
            }),
        },
    })
}

fn load_tasks(dir: &Path, only: Option<&str>) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("cannot list tasks in {}", dir.display()))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();
    for entry in entries {
        let name = entry
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(filter) = only
            && !name.contains(filter)
        {
            continue;
        }
        let spec_path = entry.join("task.toml");
        let repo_dir = entry.join("repo");
        if !spec_path.is_file() || !repo_dir.is_dir() {
            continue;
        }
        let spec: TaskSpec = toml::from_str(
            &std::fs::read_to_string(&spec_path)
                .with_context(|| format!("cannot read {}", spec_path.display()))?,
        )
        .with_context(|| format!("bad task.toml in {name}"))?;
        let protect =
            protect_set(&spec.protect).with_context(|| format!("bad protect globs in {name}"))?;
        tasks.push(Task {
            name,
            repo_dir,
            spec,
            protect,
        });
    }
    Ok(tasks)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Walk `dir` and return {relative path: bytes} for every file matching the
/// protect globs.
fn collect_protected(
    dir: &Path,
    globs: &globset::GlobSet,
) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = std::collections::BTreeMap::new();
    for entry in ignore::WalkBuilder::new(dir).require_git(false).build() {
        let entry = entry?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let rel = entry.path().strip_prefix(dir)?.to_path_buf();
        if globs.is_match(&rel) {
            files.insert(rel, std::fs::read(entry.path())?);
        }
    }
    Ok(files)
}

/// Paths where the workdir's protected files differ from the fixture:
/// changed content, missing files, or new files under protected globs.
fn tampered_paths(
    fixture_dir: &Path,
    workdir: &Path,
    globs: &globset::GlobSet,
) -> Result<Vec<String>> {
    let fixture = collect_protected(fixture_dir, globs)?;
    let current = collect_protected(workdir, globs)?;
    let mut bad: Vec<String> = Vec::new();
    for (rel, bytes) in &fixture {
        match current.get(rel) {
            Some(have) if have == bytes => {}
            Some(_) => bad.push(format!("{} (edited)", rel.display())),
            None => bad.push(format!("{} (deleted)", rel.display())),
        }
    }
    for rel in current.keys() {
        if !fixture.contains_key(rel) {
            bad.push(format!("{} (added)", rel.display()));
        }
    }
    bad.sort();
    Ok(bad)
}

/// Overwrite the workdir's protected files with the fixture copies and remove
/// files the agent added under protected globs, so `check` sees the real tests.
fn restore_protected(fixture_dir: &Path, workdir: &Path, globs: &globset::GlobSet) -> Result<()> {
    let fixture = collect_protected(fixture_dir, globs)?;
    let current = collect_protected(workdir, globs)?;
    for rel in current.keys() {
        if !fixture.contains_key(rel) {
            std::fs::remove_file(workdir.join(rel))?;
        }
    }
    for (rel, bytes) in &fixture {
        let dest = workdir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, bytes)?;
    }
    Ok(())
}

/// Run the task check command inside the workspace copy. Returns
/// `Ok(true)` when the command exits zero.
async fn run_check(command: &str, dir: &Path, timeout_seconds: u64) -> Result<bool> {
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("cannot spawn check `{command}`"))?;
    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_seconds),
        child.wait(),
    )
    .await
    {
        Ok(status) => Ok(status.map(|s| s.success()).unwrap_or(false)),
        Err(_) => {
            let _ = child.kill().await;
            Ok(false)
        }
    }
}

fn summarize(records: &[RunRecord]) -> Vec<TaskSummary> {
    let mut order: Vec<String> = Vec::new();
    for record in records {
        if !order.contains(&record.task) {
            order.push(record.task.clone());
        }
    }
    order
        .into_iter()
        .map(|task| {
            let runs: Vec<&RunRecord> = records.iter().filter(|r| r.task == task).collect();
            let n = runs.len() as f64;
            let solved = runs.iter().filter(|r| r.solved).count() as u32;
            let mean = |f: fn(&RunRecord) -> f64| runs.iter().map(|r| f(r)).sum::<f64>() / n;
            TaskSummary {
                task,
                runs: runs.len() as u32,
                solved,
                solve_rate: solved as f64 / n,
                mean_steps: mean(|r| r.steps as f64),
                mean_tool_calls: mean(|r| r.tool_calls as f64),
                mean_completion_tokens: mean(|r| r.completion_tokens as f64),
                mean_wall_s: mean(|r| r.wall_ms as f64 / 1000.0),
                timeouts: runs
                    .iter()
                    .filter(|r| {
                        r.error
                            .as_deref()
                            .is_some_and(|e| e.contains("time budget"))
                    })
                    .count() as u32,
                loop_aborts: runs
                    .iter()
                    .filter(|r| {
                        r.error
                            .as_deref()
                            .is_some_and(|e| e.contains("repeated identical"))
                    })
                    .count() as u32,
                tampered: runs.iter().filter(|r| r.tampered).count() as u32,
            }
        })
        .collect()
}

fn print_table(report: &Report) {
    println!();
    println!(
        "| {:<28} | {:>5} | {:>5} | {:>6} | {:>7} | {:>6} | {:>4} | {:>5} | {:>4} |",
        "task", "runs", "solve", "steps", "tools", "tokens", "to", "loop", "tamp"
    );
    println!(
        "|{:-<30}|{:-<7}|{:-<7}|{:-<8}|{:-<9}|{:-<8}|{:-<6}|{:-<7}|{:-<6}|",
        "", "", "", "", "", "", "", "", ""
    );
    for row in &report.summary {
        println!(
            "| {:<28} | {:>5} | {:>4.0}% | {:>6.1} | {:>7.1} | {:>6.0} | {:>4} | {:>5} | {:>4} |",
            row.task,
            row.runs,
            row.solve_rate * 100.0,
            row.mean_steps,
            row.mean_tool_calls,
            row.mean_completion_tokens,
            row.timeouts,
            row.loop_aborts,
            row.tampered,
        );
    }
    let total = report.runs.len() as f64;
    let solved = report.runs.iter().filter(|r| r.solved).count() as f64;
    println!(
        "\noverall: {solved}/{total} solved ({:.0}%)",
        if total > 0.0 {
            solved / total * 100.0
        } else {
            0.0
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny python fixture: `calc.py` with a planted bug plus
    /// `tests/test_calc.py` catching it. Returns the temp dir.
    fn make_fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join("tests")).unwrap();
        std::fs::write(repo.join("tests/__init__.py"), "").unwrap();
        std::fs::write(
            repo.join("tests/test_calc.py"),
            "import unittest\nfrom calc import add\nclass T(unittest.TestCase):\n  def test_add(self):\n    self.assertEqual(add(2, 3), 5)\n",
        )
        .unwrap();
        std::fs::write(repo.join("calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
        dir
    }

    const CHECK: &str = "python3 -m unittest discover -s tests -t . -q";

    fn set() -> globset::GlobSet {
        protect_set(&[]).unwrap()
    }

    /// Mirror of the grading flow in `run_one`: tamper scan, restore, check.
    /// Returns (solved, tampered_paths).
    async fn grade(
        fixture_dir: &Path,
        workdir: &Path,
        globs: &globset::GlobSet,
    ) -> (bool, Vec<String>) {
        let tampered = tampered_paths(fixture_dir, workdir, globs).unwrap();
        if !tampered.is_empty() {
            restore_protected(fixture_dir, workdir, globs).unwrap();
        }
        let solved = tampered.is_empty()
            && matches!(
                run_check(CHECK, workdir, CHECK_TIMEOUT_SECONDS).await,
                Ok(true)
            );
        (solved, tampered)
    }

    #[tokio::test]
    async fn unmodified_fixture_fails_check_without_tamper() {
        let dir = make_fixture();
        let fixture = dir.path().join("repo");
        let work = tempfile::tempdir().unwrap();
        copy_dir(&fixture, work.path()).unwrap();
        let (solved, tampered) = grade(&fixture, work.path(), &set()).await;
        assert!(!solved);
        assert!(tampered.is_empty());
    }

    #[tokio::test]
    async fn real_fix_passes_and_is_not_tampered() {
        let dir = make_fixture();
        let fixture = dir.path().join("repo");
        let work = tempfile::tempdir().unwrap();
        copy_dir(&fixture, work.path()).unwrap();
        std::fs::write(
            work.path().join("calc.py"),
            "def add(a, b):\n    return a + b\n",
        )
        .unwrap();
        let (solved, tampered) = grade(&fixture, work.path(), &set()).await;
        assert!(solved);
        assert!(tampered.is_empty());
    }

    #[tokio::test]
    async fn edited_test_is_tampered_and_check_reruns_real_tests() {
        let dir = make_fixture();
        let fixture = dir.path().join("repo");
        let work = tempfile::tempdir().unwrap();
        copy_dir(&fixture, work.path()).unwrap();
        // Weaken the assertion so the buggy code would pass.
        std::fs::write(
            work.path().join("tests/test_calc.py"),
            "import unittest\nclass T(unittest.TestCase):\n  def test_add(self):\n    pass\n",
        )
        .unwrap();
        let (solved, tampered) = grade(&fixture, work.path(), &set()).await;
        assert!(!solved);
        assert_eq!(tampered, vec!["tests/test_calc.py (edited)"]);
        // The original test file was restored before the check ran, so the
        // real test is back in place and still fails on buggy code.
        assert!(
            std::fs::read_to_string(work.path().join("tests/test_calc.py"))
                .unwrap()
                .contains("add(2, 3)")
        );
    }

    #[tokio::test]
    async fn deleted_test_is_tampered() {
        let dir = make_fixture();
        let fixture = dir.path().join("repo");
        let work = tempfile::tempdir().unwrap();
        copy_dir(&fixture, work.path()).unwrap();
        std::fs::remove_file(work.path().join("tests/test_calc.py")).unwrap();
        let (solved, tampered) = grade(&fixture, work.path(), &set()).await;
        assert!(!solved);
        assert_eq!(tampered, vec!["tests/test_calc.py (deleted)"]);
    }

    #[tokio::test]
    async fn added_test_file_is_tampered_and_removed() {
        let dir = make_fixture();
        let fixture = dir.path().join("repo");
        let work = tempfile::tempdir().unwrap();
        copy_dir(&fixture, work.path()).unwrap();
        std::fs::write(work.path().join("tests/test_extra.py"), "x = 1\n").unwrap();
        let (solved, tampered) = grade(&fixture, work.path(), &set()).await;
        assert!(!solved);
        assert_eq!(tampered, vec!["tests/test_extra.py (added)"]);
        assert!(!work.path().join("tests/test_extra.py").exists());
    }

    #[tokio::test]
    async fn real_fix_with_restored_tests_passes_when_source_only_changed() {
        let dir = make_fixture();
        let fixture = dir.path().join("repo");
        let work = tempfile::tempdir().unwrap();
        copy_dir(&fixture, work.path()).unwrap();
        // Agent fixes the bug AND deletes the test dir: tampered, so unsolved.
        std::fs::write(
            work.path().join("calc.py"),
            "def add(a, b):\n    return a + b\n",
        )
        .unwrap();
        std::fs::remove_dir_all(work.path().join("tests")).unwrap();
        let (solved, tampered) = grade(&fixture, work.path(), &set()).await;
        assert!(!solved);
        assert_eq!(tampered.len(), 2); // __init__.py and test_calc.py deleted
    }

    /// Every benchmark task must resolve at least one protected file, else
    /// the tamper check silently has nothing to compare.
    #[test]
    fn every_bench_task_has_protected_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("bench");
        let mut missing = Vec::new();
        for suite in ["tasks", "tasks-hard"] {
            let dir = root.join(suite);
            if !dir.is_dir() {
                continue;
            }
            for task in load_tasks(&dir, None).unwrap() {
                let files = collect_protected(&task.repo_dir, &task.protect).unwrap();
                if files.is_empty() {
                    missing.push(format!("{suite}/{}", task.name));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "tasks with no protected files: {missing:?}"
        );
    }
}
