# ADR 0004: Plugin Architecture

## Status

Accepted

## Context

PRB needs to support adding new protocol decoders without modifying the core codebase.
Users may want to decode proprietary protocols or experimental formats.

## Decision

Implement a dual plugin system supporting both native (shared library) and WebAssembly plugins:

**Native plugins (.so/.dylib/.dll):**
- Maximum performance (no serialization overhead)
- Direct access to Rust types via FFI
- Security: runs in host process (full system access)
- Use case: High-performance production decoders

**WASM plugins (.wasm):**
- Sandboxed execution via Extism runtime
- Cross-platform (compile once, run anywhere)
- Memory-safe and resource-limited
- Use case: Untrusted third-party plugins

Both plugin types implement:
1. `detect()` - Identify if data matches the protocol
2. `decode()` - Parse bytes into DebugEvents

## Consequences

**Positive:**
- Extend PRB without core code changes
- Native plugins have zero-overhead decoding
- WASM plugins provide safety and portability
- Plugin API is stable (semantic versioning)

**Negative:**
- Plugin ABI stability requires careful design
- WASM plugins have serialization overhead (JSON or MessagePack)
- Native plugins can crash the host process
- Complex plugin loading and lifecycle management

## Implementation

### Plugin API Contract

Defined in `prb-plugin-api`:

```rust
pub trait ProtocolDecoder {
    fn protocol_id(&self) -> &str;
    fn detect(&self, ctx: &DetectionContext) -> Option<DetectionResult>;
    fn decode(&self, data: &[u8]) -> Result<Vec<DebugEvent>, DecodeError>;
}
```

### Native Plugin Example

```rust
#[no_mangle]
pub extern "C" fn prb_plugin_info() -> PluginInfo {
    PluginInfo {
        name: c"my-protocol".as_ptr(),
        version: c"0.1.0".as_ptr(),
        api_version: c"0.1.0".as_ptr(),
        protocol_id: c"my-protocol".as_ptr(),
    }
}

#[no_mangle]
pub extern "C" fn prb_plugin_detect(ctx: ByteBuffer) -> DetectResultFfi {
    // Detection logic
}

#[no_mangle]
pub extern "C" fn prb_plugin_decode(data: ByteBuffer) -> OwnedBuffer {
    // Decoding logic - return JSON-serialized events
}
```

### WASM Plugin Example

WASM plugins export the same functions but use Extism's PDK for I/O:

```rust
use extism_pdk::*;

#[plugin_fn]
pub fn detect(input: Vec<u8>) -> FnResult<Vec<u8>> {
    let ctx: DetectContext = serde_json::from_slice(&input)?;
    // Detection logic
    Ok(serde_json::to_vec(&result)?)
}

#[plugin_fn]
pub fn decode(input: Vec<u8>) -> FnResult<Vec<u8>> {
    // Decode and return events as JSON
}
```

## Security Model

**Native plugins:**
- Full system access (file I/O, network, FFI)
- No sandboxing - trust required
- Use for first-party and vetted plugins only

**WASM plugins:**
- Memory-isolated sandbox via Extism
- Configurable resource limits (memory, CPU)
- No file/network access by default
- Safe for third-party plugins

## Plugin Discovery

Plugins are loaded from:
1. `~/.prb/plugins/` (user plugins)
2. `/usr/local/lib/prb/plugins/` (system plugins)
3. `--plugin-dir` CLI argument

PRB scans for:
- Native: `*.so`, `*.dylib`, `*.dll`
- WASM: `*.wasm`

## API Versioning

Plugins declare their API version. PRB checks compatibility:
- Major version mismatch → reject plugin
- Minor version newer than host → reject plugin
- Minor version older than host → accept (backward compatible)

## Future Considerations

- Plugin marketplace/registry
- Hot-reloading during development
- Plugin configuration files
- Chaining plugins (protocol layers)
