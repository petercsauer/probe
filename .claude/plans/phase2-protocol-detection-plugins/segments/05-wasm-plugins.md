---
segment: 5
title: "WASM Plugin System (extism)"
crate: prb-plugin-wasm
status: pending
depends_on: [2, 3]
parallelizable_with: [4]
estimated_effort: "6-8 hours"
risk: 6/10
---

# Segment 5: WASM Plugin System (extism)

## Objective

Build the WASM-based plugin system using Extism as the runtime. WASM plugins
are sandboxed, portable, and safe — ideal for community/third-party decoders.
A plugin author compiles their decoder to `wasm32-unknown-unknown` and distributes
a single `.wasm` file that works on all platforms.

## Why Extism

| Option | Pros | Cons |
|--------|------|------|
| **Extism** (chosen) | Production-ready (v1.10), batteries-included (memory, I/O, manifest), Rust PDK | Slightly higher overhead than raw wasmtime |
| Raw wasmtime | Maximum control, lowest overhead | Significant boilerplate for memory management, no PDK |
| Wasmer | Good performance | Less mature Rust SDK than Extism |
| WASI Component Model | Future standard, typed interfaces | Still maturing, toolchain gaps |

Extism provides:
- Safe memory management across the host-guest boundary
- JSON/MessagePack serialization built-in
- Resource limits (memory, CPU fuel metering)
- Mature Rust host SDK and Rust PDK
- Active maintenance (v1.10.0, Feb 2025)

## Architecture

```
┌──────────────────────────────────────────────────┐
│  prb-plugin-wasm (host)                          │
│                                                  │
│  ┌─────────────┐   ┌──────────────────────────┐ │
│  │ WasmRuntime  │   │ WasmDecoderInstance      │ │
│  │ (extism)     │   │ - Plugin instance        │ │
│  │              │──►│ - Calls exported fns     │ │
│  │ Fuel limits  │   │ - JSON across boundary   │ │
│  │ Memory caps  │   └──────────────────────────┘ │
│  └─────────────┘                                 │
└──────────────────────────────────────────────────┘
            ▲
            │ loads .wasm
            │
┌───────────┴──────────────────────────────────────┐
│  my-decoder.wasm (guest, compiled from Rust PDK) │
│                                                  │
│  Exports:                                        │
│  - prb_plugin_info() → JSON                      │
│  - prb_plugin_detect(JSON) → JSON                │
│  - prb_plugin_decode(JSON) → JSON                │
└──────────────────────────────────────────────────┘
```

## WASM Plugin Exports (Guest Side)

WASM plugins export three functions using Extism's `#[plugin_fn]` macro.
All data is passed as JSON (Extism handles serialization).

```rust
// In the WASM plugin (compiled with prb-plugin-api + extism-pdk)
use extism_pdk::*;
use prb_plugin_api::*;

#[plugin_fn]
pub fn prb_plugin_info() -> FnResult<Json<PluginMetadata>> {
    Ok(Json(PluginMetadata {
        name: "my-decoder".into(),
        version: "0.1.0".into(),
        description: "My custom protocol decoder".into(),
        protocol_id: "my-proto".into(),
    }))
}

#[plugin_fn]
pub fn prb_plugin_detect(Json(ctx): Json<DetectContext>) -> FnResult<Json<Option<f32>>> {
    // Return Some(confidence) if this plugin can handle the data
    if ctx.initial_bytes.starts_with(b"MY_MAGIC") {
        Ok(Json(Some(0.95)))
    } else {
        Ok(Json(None))
    }
}

#[plugin_fn]
pub fn prb_plugin_decode(Json(req): Json<WasmDecodeRequest>) -> FnResult<Json<Vec<DebugEventDto>>> {
    // Decode the byte stream and return events
    let events = vec![];
    Ok(Json(events))
}
```

### `WasmDecodeRequest` — Combined input for decode

```rust
/// Request sent to the WASM decode function.
/// Combines data + context in one JSON message (single Extism call).
#[derive(Serialize, Deserialize)]
pub struct WasmDecodeRequest {
    /// Raw stream data, base64-encoded.
    pub data_b64: String,
    /// Decode context.
    pub ctx: DecodeCtx,
}
```

**Why base64?** WASM plugin functions in Extism receive a single input buffer.
Binary data must be encoded for JSON transport. Base64 adds ~33% overhead but
keeps the interface simple. For large streams, the native plugin path is
preferred.

## `prb-plugin-wasm` Crate

### Cargo.toml

```toml
[package]
name = "prb-plugin-wasm"
version.workspace = true
edition.workspace = true

[dependencies]
prb-core = { path = "../prb-core" }
prb-plugin-api = { path = "../prb-plugin-api" }
prb-detect = { path = "../prb-detect" }
extism = "1.10"
serde.workspace = true
serde_json.workspace = true
base64.workspace = true
tracing.workspace = true
toml = "0.8"
```

### `WasmPluginLoader`

```rust
/// Loads and manages WASM plugin instances.
pub struct WasmPluginLoader {
    plugins: Vec<WasmPlugin>,
}

struct WasmPlugin {
    /// Extism plugin instance.
    instance: extism::Plugin,
    /// Cached metadata from prb_plugin_info().
    info: PluginMetadata,
    /// Path to the .wasm file.
    path: PathBuf,
}

impl WasmPluginLoader {
    pub fn new() -> Self { /* ... */ }

    /// Load a WASM plugin from a .wasm file.
    ///
    /// Validates:
    /// 1. File exists and is valid WASM
    /// 2. Required exports are present (prb_plugin_info, prb_plugin_detect, prb_plugin_decode)
    /// 3. prb_plugin_info() returns valid metadata
    /// 4. API version is compatible
    pub fn load(&mut self, path: &Path) -> Result<PluginMetadata, PluginError> {
        let manifest = extism::Manifest::new([extism::Wasm::file(path)])
            .with_memory_max(256)          // 256 pages = 16MB max memory
            .with_timeout(Duration::from_secs(30));

        let mut plugin = extism::Plugin::new(&manifest, [], true)?;

        // Call prb_plugin_info to get metadata
        let info_json = plugin.call::<&str, String>("prb_plugin_info", "")?;
        let info: PluginMetadata = serde_json::from_str(&info_json)?;

        // Validate API version
        validate_api_version(&info.api_version)?;

        self.plugins.push(WasmPlugin { instance: plugin, info: info.clone(), path: path.into() });
        Ok(info)
    }

    /// Load all .wasm files from a directory.
    pub fn load_directory(&mut self, dir: &Path) -> Vec<Result<PluginMetadata, PluginError>> { /* ... */ }
}
```

### Resource Limits

```rust
/// Default resource limits for WASM plugins.
pub struct WasmLimits {
    /// Maximum memory in WASM pages (64KB each). Default: 256 (16MB).
    pub memory_max_pages: u32,
    /// Execution timeout. Default: 30 seconds.
    pub timeout: Duration,
}

impl Default for WasmLimits {
    fn default() -> Self {
        Self {
            memory_max_pages: 256,
            timeout: Duration::from_secs(30),
        }
    }
}
```

### Adapter: WasmPlugin → DecoderFactory + ProtocolDetector

```rust
/// Adapts a WASM plugin to the DecoderFactory trait.
struct WasmDecoderFactory {
    plugin_path: PathBuf,
    info: PluginMetadata,
    limits: WasmLimits,
}

impl DecoderFactory for WasmDecoderFactory {
    fn protocol_id(&self) -> &ProtocolId {
        &ProtocolId::new(&self.info.protocol_id)
    }

    fn create(&self) -> Box<dyn ProtocolDecoder> {
        // Each decoder instance gets its own WASM plugin instance
        // (WASM instances are not thread-safe / reentrant)
        let manifest = extism::Manifest::new([extism::Wasm::file(&self.plugin_path)])
            .with_memory_max(self.limits.memory_max_pages)
            .with_timeout(self.limits.timeout);
        let instance = extism::Plugin::new(&manifest, [], true)
            .expect("plugin already validated during load");
        Box::new(WasmDecoderInstance { instance })
    }
}

/// WASM decoder instance wrapping an Extism plugin.
struct WasmDecoderInstance {
    instance: extism::Plugin,
}

impl ProtocolDecoder for WasmDecoderInstance {
    fn protocol(&self) -> TransportKind { /* from info */ }

    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        use base64::Engine;

        let request = WasmDecodeRequest {
            data_b64: base64::engine::general_purpose::STANDARD.encode(data),
            ctx: DecodeCtx::from(ctx),
        };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| CoreError::Decode(format!("serialize request: {}", e)))?;

        let result_json = self.instance
            .call::<&str, String>("prb_plugin_decode", &request_json)
            .map_err(|e| CoreError::Decode(format!("WASM decode call: {}", e)))?;

        let dtos: Vec<DebugEventDto> = serde_json::from_str(&result_json)
            .map_err(|e| CoreError::Decode(format!("deserialize response: {}", e)))?;

        // Convert DTOs to DebugEvents
        dtos.into_iter()
            .map(|dto| dto.try_into_debug_event())
            .collect()
    }
}
```

### WASM Protocol Detector

```rust
struct WasmProtocolDetector {
    plugin_path: PathBuf,
    info: PluginMetadata,
    limits: WasmLimits,
}

impl ProtocolDetector for WasmProtocolDetector {
    fn name(&self) -> &str { &self.info.name }

    fn transport(&self) -> TransportLayer {
        // Determined from plugin metadata or defaults to both TCP and UDP
        TransportLayer::Tcp // or configured per-plugin
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        // Create a temporary plugin instance for detection
        // (or use a shared one with interior mutability)
        let manifest = extism::Manifest::new([extism::Wasm::file(&self.plugin_path)])
            .with_memory_max(16) // Minimal memory for detection
            .with_timeout(Duration::from_millis(100)); // Fast timeout for detection

        let mut instance = extism::Plugin::new(&manifest, [], true).ok()?;

        let detect_ctx = DetectContext {
            initial_bytes: ctx.initial_bytes,
            src_port: ctx.src_port,
            dst_port: ctx.dst_port,
            transport: ctx.transport,
        };

        let ctx_json = serde_json::to_string(&detect_ctx).ok()?;
        let result_json = instance.call::<&str, String>("prb_plugin_detect", &ctx_json).ok()?;
        let confidence: Option<f32> = serde_json::from_str(&result_json).ok()?;

        confidence.map(|c| DetectionResult {
            protocol: ProtocolId::new(&self.info.protocol_id),
            confidence: c,
            method: DetectionMethod::Heuristic,
            version: None,
        })
    }
}
```

## WASM Plugin PDK Additions to `prb-plugin-api`

Add feature flag for WASM PDK support:

```toml
# prb-plugin-api/Cargo.toml
[features]
default = []
wasm-pdk = ["extism-pdk"]

[dependencies]
extism-pdk = { version = "1.4", optional = true }
```

Provide a `DetectContext` that works in both native and WASM contexts:

```rust
/// When compiled for WASM, DetectContext deserializes initial_bytes from base64.
#[cfg(feature = "wasm-pdk")]
#[derive(Deserialize)]
pub struct DetectContext {
    pub initial_bytes_b64: String,
    pub src_port: u16,
    pub dst_port: u16,
    pub transport: TransportLayer,
}

#[cfg(feature = "wasm-pdk")]
impl DetectContext {
    pub fn initial_bytes(&self) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.initial_bytes_b64)
            .unwrap_or_default()
    }
}
```

## Example WASM Plugin

```
examples/
└── plugin-wasm-skeleton/
    ├── Cargo.toml
    ├── src/
    │   └── lib.rs
    └── plugin.toml
```

```toml
# examples/plugin-wasm-skeleton/Cargo.toml
[package]
name = "prb-plugin-wasm-example"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
prb-plugin-api = { path = "../../crates/prb-plugin-api", features = ["wasm-pdk"] }
extism-pdk = "1.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## Tasks

### T5.1: Create `prb-plugin-wasm` crate skeleton
- Cargo.toml with `extism` dependency
- Module structure: `lib.rs`, `runtime.rs`, `adapter.rs`

### T5.2: Implement `WasmPluginLoader`
- `load()` — load `.wasm` file, create Extism manifest with limits
- `load_directory()` — discover `.wasm` files
- API version validation
- Required export validation
- Test: load valid WASM → succeeds
- Test: load invalid WASM → descriptive error

### T5.3: Implement resource limits
- `WasmLimits` struct with configurable memory and timeout
- Wire into Extism manifest
- Test: plugin exceeding memory limit → error
- Test: plugin exceeding timeout → error

### T5.4: Implement WasmPlugin → DecoderFactory adapter
- `WasmDecoderFactory` creates fresh plugin instances
- `WasmDecoderInstance` wraps Extism plugin, implements `ProtocolDecoder`
- JSON + base64 data passing across boundary
- Test: decode call produces valid `DebugEvent`s

### T5.5: Implement WasmPlugin → ProtocolDetector adapter
- `WasmProtocolDetector` calls `prb_plugin_detect` export
- Fast timeout (100ms) for detection calls
- Minimal memory allocation for detection
- Test: detect returns correct confidence

### T5.6: Add WASM PDK support to `prb-plugin-api`
- Feature flag `wasm-pdk`
- WASM-compatible `DetectContext` with base64 bytes
- `WasmDecodeRequest` type

### T5.7: Create example WASM plugin
- `examples/plugin-wasm-skeleton/`
- Build with `cargo build --target wasm32-unknown-unknown --release`
- Verify loads and runs through `WasmPluginLoader`

### T5.8: Integration test
- Build example WASM plugin in CI
- Load it via `WasmPluginLoader`
- Register with `DecoderRegistry`
- Feed test data through pipeline
- Verify detection and decoding work end-to-end

## Performance Considerations

WASM plugins add overhead compared to built-in decoders:

| Operation | Built-in | WASM |
|-----------|----------|------|
| Detection | <1μs | ~50-100μs (includes WASM instantiation) |
| Decode | variable | +100-500μs per call (JSON serialization + WASM overhead) |
| Memory | shared process | sandboxed (16MB default cap) |

**Mitigation**: Detection creates a lightweight instance; decoding reuses the
instance. The registry runs built-in detectors first (they're faster), and only
falls through to WASM detectors if built-ins don't match.

For hot paths, recommend native plugins. WASM is for safety and portability.

## Verification

```bash
cargo test -p prb-plugin-wasm
# Build WASM example
cd examples/plugin-wasm-skeleton
cargo build --target wasm32-unknown-unknown --release
# Integration test loads the .wasm
cargo test -p prb-plugin-wasm -- --test integration
```

WASM plugin compiles, loads, detects protocol, and decodes data through the
standard `ProtocolDecoder` interface. Resource limits are enforced.
