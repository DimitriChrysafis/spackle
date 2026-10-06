# Offline inventory

Prepared on 2026-08-21 for the spackle repository.

## Development inputs

- Rust toolchain: 1.89.0, Apple arm64
- Installed components: Cargo, rustc, standard library, rustfmt, Clippy,
  rust-src, rust-analyzer
- `Cargo.lock`: pinned dependency graph
- `vendor/`: 377 versioned crate-source directories, approximately 545 MB
- Cargo source replacement and forced offline mode: `.cargo/config.toml`
- Upstream llama.cpp source: approximately 203 MB at commit
  `5b6ddc9675a9c2d10c4458a510b09674a3decec1`
- Official Qwen3.8 non-weight artifacts: 13 files, approximately 24 MB
- Local llama.cpp/LM Studio runtime copy and ripgrep: approximately 26 MB
- Installed GGUF models are linked through `offline/models/` without wasting
  another approximately 38 GB of disk space.

## Prepared build

- Release starter binary: `target/release/spackle` (approximately 4.6 MB)
- The workspace passes formatting, offline check, offline tests, Clippy with
  warnings denied, and an offline release build.
- Dependency resolution was separately verified with an empty `CARGO_HOME` and
  `CARGO_NET_OFFLINE=true`.
- The read-only doctor probe connected to `http://127.0.0.1:8080`, found
  `qwen3.8-27b-local`, 150,016 served context tokens, one slot, and correctly
  classified `/metrics` HTTP 501 as unsupported.
