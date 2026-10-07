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

Two suites under `bench/`:

- `bench/tasks/` — 31 single-bug smoke tasks, one or two files each
  (wrong operators, off-by-ones, escape ordering). Fast sanity check.
- `bench/tasks-hard/` — 21 multi-file repos, ~150-240 lines each
  (Python, Rust, Node, shell), one planted bug per repo. Prompts only
  report that checks fail plus the command to reproduce; the cause is
  never named. This is the headline suite.

Each task has a `repo/` fixture and a `check` command that exits zero when
the bug is really fixed. `spackle eval` copies the fixture to a temp dir,
runs one agent turn, then runs the check — no credit for "looks fixed".

```sh
spackle eval --tasks bench/tasks-hard --runs 3 --jobs 6 \
  --model <model> --base-url <url> --api openai
```

Results land in `bench/results/<name>.json` with per-run steps, tool calls,
invalid calls, tokens, wall time, timeouts, and loop-guard aborts.

### Results (hard suite, 3 runs per task, balanced profile)

| run                                    | solved | solve % | mean steps | invalid calls |
| -------------------------------------- | ------ | ------- | ---------- | ------------- |
| gpt-5-mini                             | 60/63  | 95%     | 9.3        | 0             |
| gpt-4o-mini                            | 25/63  | 40%     | 20.6       | 0             |
| gpt-4o-mini, `--no-read-guard`         | 19/63  | 30%     | 20.4       | 0             |

The suite discriminates. gpt-4o-mini tops out the step budget (24) on the
hardest tasks; gpt-5-mini clears everything except `hc-toml-lite`, where
string-aware comment stripping defeats it in all three runs.

The read-guard ablation shows a real before/after on this suite: removing
the read-before-edit rule drops gpt-4o-mini from 40% to 30%. Without the
guard the model writes files it never opened, misses context, and either
hits the step cap or declares victory while the check still fails. On the
smoke suite the same ablation shows no delta (93/93 both ways) — guard
value only shows up when tasks get hard enough to punish blind edits.

Per-task solve rates on the hard suite span 0-100% for gpt-4o-mini
(`hc-semver`, `hc-logrotate-py`, `hc-ratelimit` solved every time;
`hc-calc-py`, `hc-globmatch-rs`, `hc-ini-rs`, `hc-querystring-js` never
solved) — headroom for ablations and model comparisons to mean something.

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
