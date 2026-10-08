# spackle

A terminal coding agent for local models. Give it a failing repo and it reads
the code, edits files, runs the tests, and keeps going until they pass,
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

All paths are canonicalized inside the workspace root: `..`, absolute paths,
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

- `bench/tasks/`: 31 single-bug smoke tasks, one or two files each
  (wrong operators, off-by-ones, escape ordering). Fast sanity check.
- `bench/tasks-hard/`: 21 multi-file repos, ~150-240 lines each
  (Python, Rust, Node, shell), one planted bug per repo. Prompts only
  report that checks fail plus the command to reproduce; the cause is
  never named. This is the headline suite.

Each task has a `repo/` fixture and a `check` command that exits zero when
the bug is really fixed. `spackle eval` copies the fixture to a temp dir,
runs one agent turn, then compares every protected file (tests, check
scripts, fixture inputs) against the original bytes. Edited, deleted, or
added test files mark the run tampered and unsolved, and the originals are
restored before `check` runs. No credit for weakening the suite and no
credit for "looks fixed".

```sh
spackle eval --tasks bench/tasks-hard --runs 3 --jobs 6 \
  --model <model> --base-url <url> --api openai
```

Results land in `bench/results/<name>.json` with per-run steps, tool calls,
invalid calls, tokens, wall time, timeouts, loop-guard aborts, and tampered
flags. `spackle eval --list-protected` prints which files each task protects.

### Results (hard suite, 3 runs per task, balanced profile)

| model                                       | guard | solved | %   | mean steps | invalid | tampered | loop | timeouts |
| ------------------------------------------- | ----- | ------ | --- | ---------- | ------- | -------- | ---- | -------- |
| Qwen3.8-27B-Splash (local, llama.cpp-style) | on    | 58/63  | 92% | 6.9        | 0       | 0        | 0    | 6        |
| Qwen3.8-27B-Splash (local)                  | off   | 57/63  | 90% | 6.9        | 0       | 0        | 0    | 6        |
| gpt-5-mini (cloud reference)                | on    | 60/63  | 95% | 9.7        | 0       | 0        | 0    | 1        |
| gpt-4o-mini (cloud reference)               | on    | 12/63  | 19% | 18.8       | 0       | 11       | 4    | 0        |
| gpt-4o-mini, `--no-read-guard`              | off   | 12/63  | 19% | 18.5       | 0       | 9        | 3    | 0        |

Cloud endpoints exist for comparison. The point of the project is the first
row: a local 27B model doing real fixes on this machine. The splash run had
a rough engine day: 12 runs died mid-turn on Metal backend stalls, so those
were re-attempted once and the merge is recorded in the result json meta.
Without the merge the raw number was 47/63.

The grader change mattered. Under the earlier version, protected files were
never compared, and gpt-4o-mini scored 40%. Rerunning with tamper detection
shows 11 of its 63 runs rewrote or deleted test files (plus 9 more with the
read guard off). Those runs now count as unsolved, and the real number is
19%. gpt-5-mini never touched a test; its 95% stands. The old runs live in
`bench/results/*-pre-tamper-check.json` and are not regradeable because the
workdirs are gone.

Read-guard ablation, paired per task (3 runs each, guard on vs off):

- splash-27b: helped on 2 tasks (hc-calc-py 3v2, hc-toml-lite 1v0), hurt on
  1 (hc-router-js 2v3), same on 18. 92% vs 90% is within noise.
- gpt-4o-mini: helped on 5, hurt on 4, same on 12. 19% vs 19%, no delta.

63 runs per condition is a small sample; do not read significance into
either gap. The mechanism still shows in the raw data though: without the
guard, models burn steps rewriting files they never read, and several
gpt-4o-mini tampered runs came from patching tests to match broken output.

Headroom: `hc-toml-lite` is the hardest task on the suite. Every task was
solved by at least one model or condition, but toml-lite went 2/15 across
the five run sets above; quote-aware comment stripping defeats almost
everything thrown at it.

Ablation flags: `--no-loop-guard`, `--no-read-guard`, `--no-output-cap`.

## Development

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --offline
cargo test --workspace --offline
```

170 tests: SSE parser fuzz + fragmentation, stream integration against a real
axum server, tool sandboxing, agent-loop state machine, eval tamper grading.

Vendored crate sources (`vendor/`) stay out of git but on disk so the whole
workspace builds offline.
