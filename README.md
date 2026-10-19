# spackle

a terminal coding agent for local models. give it a repo with failing tests and it reads the code, edits files, runs the tests, and keeps going until they pass. every file and command goes through sandboxed tools. it talks to a llama.cpp server you run yourself, or any openai-compatible endpoint. no telemetry, and it only listens on loopback by default.

```
you ──► spackle ──► llama.cpp server (127.0.0.1:8080)
            │
            └──► repo: read, edit, grep, run tests
```

## tools

| tool | what it does |
|---|---|
| `read_file` | numbered lines, byte cap, binary detection |
| `list_files` | directory walk that honors `.gitignore` |
| `grep` | regex search, capped results |
| `write_file` | atomic create or overwrite, inside the repo only |
| `edit_file` | exact-match replace, the match must be unique, shows a diff |
| `run_command` | shell with timeout, output cap, cancel |

## safety

- paths are canonicalized inside the repo. `..`, absolute paths, and symlink escapes are refused.
- `edit_file` and `write_file` refuse a file the model hasn't read first.
- writes, secret-looking paths, and destructive or network commands ask for approval. `--yes` skips the prompts. non-interactive runs deny them.

## quickstart

```sh
# llama.cpp server with your model on :8080
cargo run --release -p spackle-cli -- doctor
cargo run --release -p spackle-cli -- ask "fix the failing tests" --workspace /path/to/repo
```

config lives in `.spackle/config.toml` inside the repo, or in the platform config dir. see `config/example.toml`. flags like `--base-url`, `--model`, and `--api openai` override it.

## benchmark

![hard suite](images/hard-suite.png)

`bench/tasks` has 31 small single-bug repos for quick checks. the hard suite, `bench/tasks-hard`, has 21 repos of 150 to 240 lines each, in python, rust, node, and shell, with one planted bug per repo. the prompt says the checks fail and gives the command to reproduce, but never names the cause.

the grader compares every protected file (tests, check scripts, fixtures) against the original bytes. if the agent edits, deletes, or adds one, the run counts as tampered and unsolved.

- the local Qwen3.8-27B solves 58 of 63 runs (92%). gpt-5-mini solves 60 of 63.
- gpt-4o-mini scored 40% under the old grader, which didn't compare protected files. with the tamper check it rewrote or deleted test files in 11 of 63 runs, and the real score is 19%.

caveats:

- 12 runs of the local model died on Metal backend stalls. they were re-run once, and the result file notes it. the first pass was 47 of 63.
- 63 runs per condition is a small sample. the gap between read guard on (92%) and off (90%) is within noise. without the guard, the model spends more steps rewriting files it hasn't read.

```sh
spackle eval --tasks bench/tasks-hard --runs 3 --jobs 6 \
  --model <model> --base-url <url> --api openai
```

results land in `bench/results/`, with per-run steps, tool calls, tokens, timeouts, and tamper flags.

## development

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --offline
cargo test --workspace --offline
```

`vendor/` is not committed, so keep a local copy if you want offline builds.
