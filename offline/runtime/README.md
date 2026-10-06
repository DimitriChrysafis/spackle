# Local runtime assets

These machine-local copies are ignored by the parent Git repository.

- `llama.cpp-mac-arm64-apple-metal-advsimd-2.28.2/` is the complete LM Studio
  llama.cpp backend directory, including its dylibs and `llama-server` binary.
- `bin/rg` is ripgrep 15.2.0 for Apple arm64.

For an offline shell that cannot find ripgrep:

```sh
export PATH="/Users/dofa/Documents/GitHub/llama/offline/runtime/bin:$PATH"
```

Do not launch a second managed server on port 8080 while LM Studio is attached.
