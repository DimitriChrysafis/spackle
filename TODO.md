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
spackle-cli        doctor + ask + eval (benchmark runner, --jobs parallel,
                   protected-test grading)
spackle-tui        banner only, skipped on purpose
```

all tests green (169), clippy clean, builds offline.

## grader hardening (this round)

- every task resolves a protected-file set: default globs for common test
  layouts (tests/**, test_*.py, *.test.js, check scripts, test inputs) plus
  an optional `protect` list in task.toml
- after the agent turn, protected files are compared byte-for-byte against
  the fixture; edited/deleted/added test files mark the run tampered and
  unsolved, originals are restored before check runs
- run artifacts (__pycache__, node_modules, target, .pyc) are excluded so
  running the suite during diagnosis does not count as tampering
- `eval --list-protected` prints the protected set per task; a unit test
  fails if any bench task has no protected files
- rs smoke fixtures had tests inline in the source file; split into
  tests/mod.rs and checks now require a nonzero pass count so deleting the
  module declaration cannot fake a pass

## results

two suites: bench/tasks (31 single-bug smoke tasks) and bench/tasks-hard
(21 multi-file repos ~150-240 lines, prompt = "tests fail, fix it" + check
command, cause never disclosed). verified by running each task's check
command after the agent's turn on the restored test set.

hard suite, 3 runs per task, post-tamper-check grading:

- Qwen3.8-27B-Splash (local): 58/63 (92%) read guard on; 57/63 (90%) off.
  note: the guarded run hit Metal engine stalls mid-turn on 12 runs; those
  were re-attempted once and merged, recorded in the result json meta.
  raw pre-merge number was 47/63
- gpt-5-mini (cloud reference): 60/63 (95%), zero tampered runs
- gpt-4o-mini (cloud reference): 12/63 (19%) guard on, 12/63 (19%) off.
  11 and 9 runs tampered with test files

the grader change moved real numbers: gpt-4o-mini dropped 40% -> 19%
because 11 of 63 runs had edited or deleted tests under the old grader.
the old files live in bench/results/*-pre-tamper-check.json and are not
regradeable (workdirs deleted).

paired read-guard ablation (per task, 3 runs each):
- splash-27b: guard helped 2 tasks (hc-calc-py, hc-toml-lite), hurt 1
  (hc-router-js), same 18 -> 92% vs 90%, within noise
- gpt-4o-mini: guard helped 5 tasks, hurt 4, same 12 -> 19% vs 19%.
  at 63 runs per condition this is within noise; do not claim a win

headroom: hc-toml-lite is the hardest task (2/15 solves across all five
conditions). every task was solved by at least one model, so the suite has
no dead zones.

measured fixes along the way: truncated mid-stream chunk retries as
transient; transient retries back off so a restarting engine does not sink
a sweep; openai endpoints that reject reasoning_effort degrade once;
protected-test grading catches test editing.

## resume bullet

built spackle, a rust coding agent for local llama.cpp / openai-compatible
models with sandboxed tools and a read-before-edit guard; a local
qwen3.8-27b solves 92% of a 21-repo bug-fix benchmark that never names the
bug (gpt-5-mini: 95%)
