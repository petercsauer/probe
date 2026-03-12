---
issue: 4
title: "WASM memory and CPU limits"
severity: Medium
segments_affected: [5]
status: mitigated
---

# Issue 4: WASM Memory and CPU Limits

## Problem

WASM plugins run user-supplied code inside the Probe process. A malicious or
buggy plugin could:

1. **Allocate unbounded memory** — exhausting host memory
2. **Enter an infinite loop** — hanging the decode pipeline
3. **Attempt host system access** — reading files, network, etc.

## Impact

- A runaway plugin could OOM-kill the Probe process
- An infinite loop would hang `prb ingest` with no feedback
- A malicious plugin could exfiltrate capture data (sensitive network traffic)

## Mitigation (incorporated in Segment 5)

### M1: Memory limits via Extism manifest

```rust
let manifest = extism::Manifest::new([wasm_file])
    .with_memory_max(256);  // 256 pages × 64KB = 16MB max
```

Default: 16MB. Configurable per-plugin via `plugin.toml`:

```toml
[plugin.limits]
memory_max_pages = 512  # 32MB for heavy decoders
```

Exceeding the limit produces a trap (Wasm trap: out-of-bounds memory) that
the host catches and converts to a `CoreError::Decode`.

### M2: Execution timeout

```rust
let manifest = extism::Manifest::new([wasm_file])
    .with_timeout(Duration::from_secs(30));
```

Default: 30 seconds for decode calls, 100ms for detect calls.

Exceeding the timeout produces a trap that the host catches.

### M3: No WASI capabilities by default

WASM plugins are loaded with WASI disabled by default (no filesystem, no
network, no environment access):

```rust
extism::Plugin::new(&manifest, [], true)
//                              ^^ no host functions
//                                  ^^^^ WASI enabled but sandboxed
```

Extism's sandbox prevents plugins from accessing host resources.

### M4: Detection is lightweight

Detection calls use minimal resources:
- Memory: 16 pages (1MB) — enough to inspect bytes
- Timeout: 100ms — detection should be <1ms
- This prevents a slow/malicious detector from blocking pipeline progress

### M5: Graceful error handling

All WASM calls are wrapped in `Result`:
```rust
match instance.call("prb_plugin_decode", &input) {
    Ok(output) => { /* process output */ }
    Err(e) => {
        tracing::warn!("WASM plugin error: {}", e);
        // Return raw event as fallback
    }
}
```

A failing plugin never crashes the host — it degrades to raw event output.

## Residual Risk

- **Low**: Extism/Wasmtime are battle-tested runtimes used in production by
  Cloudflare, Shopify, and others
- **Information leakage**: Plugin code sees the stream data it's asked to decode.
  A malicious plugin could encode data in its output (e.g., in metadata fields).
  This is inherent to any plugin system — the plugin must see the data to decode
  it. Mitigation: users should only install trusted plugins.

## Acceptance Criteria

- Plugin exceeding memory limit → clean error, host continues
- Plugin exceeding timeout → clean error, host continues
- Plugin attempting file I/O → WASI sandbox blocks it
- Detection calls limited to 100ms and 1MB
- All failures degrade to raw events, never crash
