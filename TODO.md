# TODO

goal: a local coding agent that fixes real bugs on my own machine with a 27b model, plus a benchmark that proves it. the benchmark is the resume line, everything else supports it.

## where it stands (checked 2026-10-06)

```
crate            state
spackle-core       agent loop, config, session journal, loop guard, context budget. 91 tests pass
spackle-llamacpp   http client, sse parser, LlamaTransport. 10 of 22 lib tests fail (all sse), tests/stream.rs does not compile
spackle-tools      only WorkspaceRoot (path guard). no actual tools
spackle-cli        builds. `doctor` and `ask` exist, ask is a single streamed completion, no agent loop
spackle-tui        a banner function
```

the repo is private, 4 commits, and has 545 mb of vendored crates (21k files) checked in.

a model server is already running locally: splash on `http://127.0.0.1:8000`, model id `incoai/Qwen3.8-27B-Splash`. the config still points at `127.0.0.1:8080` / `qwen3.8-27b-local` from the old llama-server setup.

---

## phase 0: clean the repo (do before anything goes public)

- [ ] delete `README copy.md` (identical to README.md)
- [ ] get `vendor/` out of git
  - [ ] add `/vendor/` to `.gitignore`, keep the folder on disk so offline builds still work
  - [ ] history still holds ~58 mb of vendored files. only 4 commits exist, so the clean fix is a fresh history: new orphan branch with the current tree minus vendor, then force push. destructive, do it on purpose, not by accident
- [ ] strip agent-prompt leftovers
  - [ ] README.md: remove the `/Users/dofa/Desktop/prompt2.txt` line and "independently designed"
  - [ ] docs/OFFLINE_INVENTORY.md: remove the "Final implementation prompt" section (path + sha of prompt2.txt)
  - [ ] docs/BUILD_STATUS.md: remove "Qwen should update this file after every verified vertical slice". then either update it to match reality or delete it, it is stale (says the transport isn't done, but transport.rs exists)
- [ ] `authors = ["llama contributors"]` in Cargo.toml -> my name
- [ ] rename the project. "llama" collides with meta's llama and llama.cpp, nobody will ever find it by search, and recruiters will think it's a fork. pick something short and unused on crates.io
- [ ] from here on: small commits, lowercase, one change each. no "more stuff"

## phase 1: make it green and talk to a model

- [ ] fix the sse parser, `cargo test -p spackle-llamacpp --lib --offline`
  - 10 failures in `crates/spackle-llamacpp/src/sse.rs` (asserts at lines 162, 170, 177, 186, 193, 215, 226, 242, 258, 265)
  - all parser tests fail, including the single-event one, so the bug is in the basic event path, not an edge case. start with `parses_single_data_event`
- [ ] fix `crates/spackle-llamacpp/tests/stream.rs` so it compiles
  - axum `chat` handler doesn't satisfy `Handler`. the fake server's handler signature is off for axum 0.8 (try `#[axum::debug_handler]` to get a real error)
  - line 479: `err` is a `JoinError` from a spawned task, not a `TransportError`. unwrap the join first
  - the `expected { found ,` errors come from a macro call in the same file, fix those first, the rest may cascade
- [ ] all green:
  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --offline -- -D warnings
  cargo test --workspace --offline
  ```
- [ ] point the config at the live server (or start llama-server on 8080 again)
  - [ ] `llama doctor --base-url http://127.0.0.1:8000` reports healthy
  - [ ] `llama ask "say hi"` streams a reply end to end
- [ ] check tool calling works on the server at all: send one chat completion with a `tools` array by hand (curl) and confirm `tool_calls` comes back in openai format. if splash can't do it, use `llama-server --jinja` instead. everything after this depends on it

## phase 2: tools (`crates/spackle-tools`)

deps are already in Cargo.toml: ignore, grep-searcher, grep-regex, globset, diffy, shell-words, tempfile.

every tool goes through `WorkspaceRoot`, returns bounded output, and respects the cancel token.

- [ ] `read_file`: path, optional line range, numbered lines, byte cap with a "truncated, use a range" note. read_only
- [ ] `list_files`: glob, respects .gitignore via `ignore`. read_only
- [ ] `grep`: regex search via grep-searcher, path + line + text, result cap. read_only
- [ ] `write_file`: create or overwrite, needs approval
- [ ] `edit_file`: exact string replace, must match exactly once (error with the match count otherwise), returns a unified diff via diffy. needs approval
- [ ] `run_command`: shell-words parsing, cwd = workspace, timeout, output cap (keep head and tail), kill the process group on cancel. needs approval
- [ ] json schema for each tool, advertised in the request's `tools` array
- [ ] tests per tool
  - [ ] `../` and absolute paths rejected
  - [ ] symlink pointing outside the workspace rejected
  - [ ] edit with 0 matches and with 2 matches both fail cleanly
  - [ ] command timeout actually kills the child
  - [ ] huge file / huge output gets capped

## phase 3: wire the agent loop into the cli

- [ ] `llama ask` runs `agent::run_turn` with `LlamaTransport` + the tools instead of one completion
- [ ] flags: `--workspace <dir>`, `--yes` (auto-approve, needed for eval), `--max-steps`, `--transcript <file.jsonl>`
- [ ] interactive approval prompt when `--yes` isn't set: show the diff or the command, y/n
- [ ] sessions go to `.spackle/sessions/` through the existing journal
- [ ] smoke test: tiny python repo with one failing test, `llama ask "make the tests pass" --yes --workspace /tmp/demo` ends with pytest green. record it, this becomes the readme gif later

## phase 4: the benchmark (this is the resume line)

- [ ] `bench/tasks/<name>/` per task:
  - `repo/` small self-contained project (python mostly, some rust and ts)
  - `task.md` the prompt, written like a real issue
  - `check.sh` exits 0 when solved (usually runs the tests)
  - `meta.toml` timeout, category, difficulty
- [ ] 20 to 30 tasks, mixed:
  - off-by-one / wrong comparison
  - missing import / renamed api after a "dependency bump"
  - failing test caused by a bug two files away from the test
  - small feature with a test already written
  - bug where the obvious fix breaks a different test
  - a few that need running the code to understand (print debugging)
- [ ] write the tasks by hand. if they come from a model they will be too easy and look generated
- [ ] runner: `llama eval bench/tasks --runs 3`
  - fresh temp copy of `repo/` per run
  - records solved y/n, steps, tool calls, invalid tool calls, wall time, tokens in/out, loop-guard aborts, timeouts
  - writes `bench/results/<date>.json` and prints a markdown table
- [ ] runs to do
  - [ ] baseline: balanced profile, 3 runs per task
  - [ ] fast vs balanced vs deep profiles (thinking on/off changes a lot on a 27b)
  - [ ] ablation: loop guard off
  - [ ] ablation: no tool output caps / no context trimming
  - [ ] optional: same tasks by hand in claude code or codex, just to show the gap honestly
- [ ] report solved as `x/n` with the run spread, not just the best run

## phase 5: make a small local model reliable (the interesting part)

each of these should come with a before/after number from phase 4.

- [ ] malformed tool-call json: try a lenient parse, else send the parse error back to the model as a tool error and let it retry. measure invalid-call rate before/after
- [ ] context pressure: server reports n_ctx 150016, but long sessions still blow up. trim or summarize old tool outputs past a budget, keep the last k turns verbatim
- [ ] loops: the loop guard exists, measure how often it fires and whether stopped runs would have recovered
- [ ] thinking tokens: check preserve_thinking actually round-trips through the template, measure thinking on vs off per category
- [ ] read-before-edit: reject `edit_file` on a file the model hasn't read this session, see if it helps

## phase 6: run it on my own engine (stretch, makes all three projects one story)

- [ ] small openai-compatible server around `colibri_runtime` (qwen3.8-flash-next repo): `/v1/chat/completions` with streaming
- [ ] point the harness at it and solve one or two easy tasks with the 180b model
- [ ] be honest about speed: ~1.3 tok/s decode means one task can take a long time. the point is that it works, not that it's fast

## phase 7: ship

- [ ] readme
  - one paragraph: what it is, why local only
  - gif or vhs/asciinema recording of a real task getting fixed
  - quickstart (server, config, one command)
  - benchmark table with the real numbers and how to reproduce
  - small diagram: cli -> agent loop -> transport / tools -> llama.cpp
- [ ] tui only after the benchmark exists. skip it entirely if time is short, nobody hires on a tui
- [ ] make the repo public only after phase 0 and phase 4 are done
- [ ] github description + topics (rust, llm, agents, llama-cpp, local-first)

## resume (only with real numbers)

- [ ] bullet shape: "built a rust coding agent for local llama.cpp models with sandboxed file, search, and shell tools; solves X/N hand-written bug-fix tasks offline with qwen3.8-27b"
- [ ] second bullet from phase 5: "cut invalid tool calls from A% to B% by ..." or "context trimming raised solve rate from X to Y"
- [ ] goes in place of zwift or quantum

## interview prep

- [ ] how is the workspace sandbox enforced, and how would you break it
- [ ] why is a 27b model worse at tool calling than a frontier model, and what did you change because of it
- [ ] how did you make sure the benchmark tasks aren't leaking into the prompt or too easy
- [ ] what happens when the user hits ctrl-c mid tool call
- [ ] why sse parsing needs care with split utf-8 and split `[DONE]`

## done means

- [ ] all tests and clippy green
- [ ] `ask` fixes a real failing test end to end
- [ ] benchmark with 20+ tasks, 3 runs each, results committed
- [ ] at least one phase 5 change with a measured before/after
- [ ] clean history, renamed, public, readme with gif
