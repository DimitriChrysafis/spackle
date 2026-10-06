# Local machine snapshot

- Captured: 2026-08-21, Europe/Istanbul
- Machine: Apple M3 Max, arm64
- Unified memory: 36 GB
- Rust: 1.89.0
- Cargo: 1.89.0
- llama.cpp endpoint: `http://127.0.0.1:8080`
- Model alias: `qwen3.8-27b-local`
- Model architecture: Qwen3.8/Qwen3.5 hybrid, 27.32B parameters
- Loaded quant: Q4_K_M, approximately 16.8 GB
- Server context: 150,016 tokens
- Native model context: 262,144 tokens
- Slots: 1
- Metal/full GPU offload: enabled
- Flash attention: enabled
- KV cache K/V: q8_0/q8_0
- Vision and video: enabled
- `/v1/chat/completions/input_tokens`: supported
- `/slots`: supported
- `/metrics`: disabled on the attached server (HTTP 501)
- `/version`: unavailable on the attached server (HTTP 404)

## Installed assets

```text
/Users/dofa/.lmstudio/models/lmstudio-community/Qwen3.8-27B-GGUF/Qwen3.8-27B-Q4_K_M.gguf
/Users/dofa/.lmstudio/models/lmstudio-community/Qwen3.8-27B-GGUF/Qwen3.8-27B-Q6_K.gguf
/Users/dofa/.lmstudio/models/lmstudio-community/Qwen3.8-27B-GGUF/mmproj-Qwen3.8-27B-BF16.gguf
/Users/dofa/.lmstudio/extensions/backends/llama.cpp-mac-arm64-apple-metal-advsimd-2.28.2/llama-server
```

Do not restart or terminate the attached LM Studio server from attach mode.
