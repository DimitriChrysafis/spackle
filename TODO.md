# TODO

goal: a local coding agent that fixes real bugs on my own machine, plus a
benchmark that proves it. the benchmark is the resume line, everything else
supports it.

## where it stands (2026-10-07)

```
crate              state
spackle-core       agent loop, config, sessions, loop guard, retry backoff
spackle-llamacpp   sse parser, tool-call assembly, llamacpp+openai dialects,
                   capability fallback (reasoning_effort)
spackle-tools      read/write/edit/grep/list/run_command, sandboxed,
                   read-before-edit guard
spackle-cli        doctor + ask + eval (benchmark runner, --jobs parallel)
spackle-tui        banner only, skipped on purpose
```

all tests green, clippy clean, builds offline.

## results

two suites: bench/tasks (31 single-bug smoke tasks) and bench/tasks-hard
(21 multi-file repos ~150-240 lines, prompt = "tests fail, fix it" + check
command, cause never disclosed). verified by running each task's check
command after the agent's turn.

hard suite, 3 runs per task:

- gpt-5-mini: 60/63 (95%). only hc-toml-lite fails all 3 runs
- gpt-4o-mini: 25/63 (40%)
- gpt-4o-mini --no-read-guard: 19/63 (30%) - the guard is worth +10pts on
  this suite; on the smoke suite it was a wash (93/93 both ways)

measured fixes along the way: truncated mid-stream chunk now retries as
transient instead of dying fatal; transient retries back off so a restarting
engine doesn't sink a sweep; openai endpoints that reject reasoning_effort
get one degrade-and-retry instead of a capability error.

## resume bullet

built a rust coding agent (spackle) for local llama.cpp models with sandboxed
file, search, and shell tools plus an openai-compatible transport; ships a
52-task benchmark that verifies fixes by running each repo's checks -
gpt-4o-mini solves 40% of the hard suite and gpt-5-mini 95% (3 runs/task).
