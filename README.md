# spackle

a terminal coding agent for local models. point it at a repo with failing tests and it reads the code, edits files, and runs the tests until they pass. it talks to a llama.cpp server on your machine, or any openai-compatible endpoint.

## tools

- `read_file`: numbered lines, binary detection
- `list_files`: respects `.gitignore`
- `grep`: regex search
- `write_file`: atomic write, inside the repo only
- `edit_file`: exact match, must be unique, shows a diff
- `run_command`: shell with timeout and output cap

## safety

- paths stay inside the repo. `..` and symlink escapes are refused.
- a file has to be read before it can be edited.
- writes, secret-looking files, and destructive or network commands ask first. `--yes` skips the prompts.

## quickstart

```sh
cargo run --release -p spackle-cli -- doctor
cargo run --release -p spackle-cli -- ask "fix the failing tests" --workspace /path/to/repo
```

config is `.spackle/config.toml` in the repo. see `config/example.toml`.

## benchmark

![hard suite](images/hard-suite.png)

21 small repos, one planted bug each. the prompt never names the bug. a run only counts if the checks pass and no protected test file was touched.

- Qwen3.8-27B, local: 58/63 (92%)
- gpt-5-mini: 60/63 (95%)
- gpt-4o-mini: 19%. it scored 40% until the tamper check showed it editing tests.

caveats: 12 local runs died on Metal stalls and were rerun. the first pass was 47/63. 63 runs per setting is a small sample.

```sh
spackle eval --tasks bench/tasks-hard --runs 3 --jobs 6 --model <model> --base-url <url> --api openai
```

## development

```sh
cargo test --workspace --offline
```
