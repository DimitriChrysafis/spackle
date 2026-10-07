# spackle

A terminal coding agent for local models. Give it a failing repo and it reads
the code, edits files, runs the tests, and keeps going until they pass —
entirely through sandboxed tools, against a llama.cpp-style server you run
yourself. No cloud, no telemetry, loopback-only by default.

Built for Qwen3.8-27B on llama.cpp, but the transport speaks plain
OpenAI-compatible chat completions too, so any conforming endpoint works
(public HTTPS endpoints are an explicit opt-in).

```text
            ┌────────────────────┐
            │ spackle-cli        │   ask · doctor · eval
            └─────────┬──────────┘
                      │
            ┌─────────▼──────────┐
            │ spackle-core       │   agent loop, session journal,
            │                    │   loop guard, context budget,
            │                    │   approval gate
            └────┬───────────┬───┘
                 │           │
     ┌───────────▼───┐   ┌───▼──────────────┐
     │ spackle-tools │   │ spackle-llamacpp │   SSE stream, tool-call
     │ read / write  │   │                  │   assembly, llama.cpp +
     │ edit / grep / │   │                  │   openai request dialects
     │ list / run    │   │                  │
     └───────┬───────┘   └───┬──────────────┘
             │               │
      workspace root    http://127.0.0.1:8080
      (canon'd paths,   (llama.cpp / any
       approvals)        OpenAI-compatible)
```

## Tools

| tool          | what it does                                             |
| ------------- | -------------------------------------------------------- |
| `read_file`   | numbered lines, byte cap, binary detection, secret flags |
| `list_files`  | directory walk honoring `.gitignore`                     |
| `grep`        | ripgrep-style regex search, capped results               |
| `write_file`  | atomic create/overwrite, sandboxed to the root           |
| `edit_file`   | exact-match replace with unique-match enforcement + diff |
| `run_command` | shell commands with timeout, output caps, cancellation   |

All paths are canonicalized inside the workspace root — `..`, absolute paths,
and symlink escapes are refused. `edit_file` and `write_file` refuse to touch
a file the model hasn't read first. Mutating calls, secret-looking paths, and
destructive or network commands go through an approval gate (auto-approved
with `--yes`, denied in non-interactive use, prompted otherwise).

## Quickstart

```sh
# llama.cpp server serving your model on :8080
cargo run --release -p spackle-cli -- doctor
cargo run --release -p spackle-cli -- ask "fix the failing tests" --workspace /path/to/repo
```

Config is layered: `.spackle/config.toml` in the repo wins over the
platform config dir (`~/Library/Application Support/local.spackle/` on
macOS); see `config/example.toml`. CLI flags override everything:
`--base-url`, `--model`, `--api llamacpp|openai`, `--api-key-env`,
`--allow-private-lan`, `--allow-public-endpoint`.

OpenAI-compatible endpoint example:

```toml
[endpoint]
base_url = "https://api.openai.com"
model = "gpt-5-mini"
api = "openai"
api_key_env = "OPENAI_API_KEY"
allow_public_endpoint = true
```

## Benchmark

`bench/tasks/` holds 31 hand-written bug-fix tasks (12 Python, 8 Rust,
6 Node, 2 shell) — wrong operators and off-by-ones up through shared-memo
leaks, HTML-escape ordering, a binary search that never converges, and a
bug hiding two files away from the failing test. Each task has a `repo/`
fixture and a `check` command that exits zero when the bug is really fixed.
`spackle eval` copies the fixture to a temp dir, runs one agent turn, then
runs the check — no credit for "looks fixed".

```sh
spackle eval --tasks bench/tasks --runs 3 --jobs 3 \
  --model <model> --base-url <url> --api openai
```

Results land in `bench/results/<name>.json` with per-run steps, tool calls,
invalid calls, tokens, wall time, timeouts, and loop-guard aborts.

### Results

3 runs per task, balanced profile. Every task folder's check ran; a run only
counts when the exit status is zero.

| model                            | tasks | runs | solved | mean steps | invalid calls | loop aborts | timeouts |
| -------------------------------- | ----- | ---- | ------ | ---------- | ------------- | ----------- | -------- |
| gpt-5-mini (openai api)          | 31    | 93   | 93     | 7.2        | 0             | 0           | 0        |
| Qwen3.8-27B-Splash (local)       | 31    | 93   | 93     | 5.8        | 0             | 0           | 0        |

The suite is saturated at this size — both models clear every task. What the
numbers do show: the harness held up over 186 agent turns / 1117 tool calls
with zero malformed calls, and mean wall time is ~13s per run on gpt-5-mini
vs ~71s on the local 27B.

Ablation flags: `--no-loop-guard`, `--no-read-guard`, `--no-output-cap`.

## Development

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --offline
cargo test --workspace --offline
```

162 tests: SSE parser fuzz + fragmentation, stream integration against a real
axum server, tool sandboxing, agent-loop state machine.

Vendored crate sources (`vendor/`) stay out of git but on disk so the whole
workspace builds offline.
