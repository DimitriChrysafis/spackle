# spackle

`spackle` is a local-only terminal coding-agent harness for models served by
llama.cpp (or any OpenAI-compatible endpoint). It is being built for the
Qwen3.8-27B model running on this machine.

```sh
cargo check --workspace --offline
cargo test --workspace --offline
cargo run --offline -p spackle-cli -- doctor
```

The default endpoint is a llama.cpp server:

```text
http://127.0.0.1:8080
```

No cloud fallback, no telemetry.
