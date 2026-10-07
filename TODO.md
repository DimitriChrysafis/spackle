# TODO

goal: a local coding agent that fixes real bugs on my own machine with a 27b model, plus a benchmark that proves it. the benchmark is the resume line, everything else supports it.

## where it stands (2026-10-06)

```
crate              state
spackle-core       agent loop, config, sessions, loop guard, retry backoff. 92 tests
spackle-llamacpp   sse parser, tool-call assembly, llamacpp+openai dialects. 41 tests
spackle-tools      read/write/edit/grep/list/run_command, sandboxed. 29 tests
spackle-cli        doctor + ask + eval (benchmark runner)
spackle-tui        banner only, skipped on purpose
```

all 162 tests green, clippy clean, builds offline.

## results

- eval: 31 hand-written tasks, 3 runs each, verify-by-check-command
- gpt-5-mini: 93/93. mean 7.2 steps, ~13s/run
- qwen3.8-27b splash (local): 93/93. mean 5.8 steps, ~71s/run
- ablation, no read guard: 93/93 - guard not a lever on this suite, kept anyway
- loop guard: never fired in 186+ runs; --no-loop-guard ablation identical by
  construction, noted rather than run
- measured fix: truncated mid-stream chunk was a fatal error; now transient
  retry + backoff. the splash engine died mid-sweep for ~30 min and every
  in-flight run recovered instead of dying (pre-backoff they all failed)

## resume bullet

built a rust coding agent (spackle) for local llama.cpp models with sandboxed
file, search, and shell tools plus an openai-compatible transport; solves
31/31 hand-written bug-fix tasks (93/93 runs) with a local qwen3.8-27b.
