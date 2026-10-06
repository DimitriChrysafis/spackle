# Offline development bundle

This directory contains machine-specific runtime copies and upstream reference
material needed while Wi-Fi is unavailable.

Prepared contents:

- `reference-src/llama.cpp/`: shallow snapshot of official llama.cpp source
- `references/qwen3.8-27b/`: Qwen model/configuration documents
- `runtime/llama.cpp-mac-arm64-apple-metal-advsimd-2.28.2/`: local runtime copy
- `runtime/bin/rg`: local ripgrep 15.2 binary
- `models/`: convenient symlinks to the already-installed GGUF files
- `SERVER_SNAPSHOT.json`: sanitized endpoint/capability snapshot
- `SHA256SUMS`: integrity inventory for non-vendored reference assets

Rust crate sources live at the repository root in `vendor/`. Cargo is configured
to use them with networking disabled.

The llama.cpp source revision is recorded in `LLAMA_CPP_REVISION.txt`. Verify
the downloaded/copied non-source assets with:

```sh
shasum -a 256 -c offline/SHA256SUMS
```

The 16 GB Q4_K_M model, 21 GB Q6_K model, and 888 MB vision projector already
exist under `~/.lmstudio/models/`; they are not duplicated here. Their exact
paths and checks are recorded in `MACHINE_SNAPSHOT.md`.
