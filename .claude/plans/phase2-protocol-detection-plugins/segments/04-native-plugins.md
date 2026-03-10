---
segment: 4
title: "Native Plugin System (libloading)"
crate: prb-plugin-api, prb-plugin-native
status: pending
depends_on: [2, 3]
estimated_effort: "6-8 hours"
risk: 5/10
---

# Segment 4: Native Plugin System

## Objective

Build the native (shared library) plugin system that allows users to compile
custom protocol decoders as `.so`/`.dylib`/`.dll` files and load them at runtime.
This uses `libloading` for dynamic library loading and a C-compatible ABI for
the plugin interface.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  prb-plugin-api (stable ABI crate)                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Plugin trait (Rust-side)                              │ │
│  │  C ABI functions (extern "C")                         │ │
│  │  Serialized types (DetectRequest, DecodeRequest, etc.) │ │
│  └────────────────────────────────────────────────────────┘ │
└───────────┬─────────────────────────────────┬───────────────┘
            │                                 │
  ┌─────────▼─────────┐            ┌──────────▼──────────┐
  │  prb-plugin-native │            │  user-plugin.so     │
  │  (host loader)     │◄──loads────│  (cdylib)           │
  │  libloading        │            │  implements C ABI   │
  └────────────────────┘            └─────────────────────┘
```

## `prb-plugin-api` Crate — The Stable Contract

This crate is the **only** dependency a plugin author needs. It defines the ABI
contract using `repr(C)` types and `extern "C"` function signatures.

### Cargo.toml

```toml
[package]
name = "prb-plugin-api"
version = "0.1.0"
edition.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
semver = "1"
```

### API Version

```rust
/// Plugin API version. Plugins compiled against a different major version
/// will be rejected. Minor version bumps are backward-compatible.
pub const API_VERSION: &str = "0.1.0";
```

### C ABI Types

```rust
/// Plugin metadata returned by `prb_plugin_info()`.
#[repr(C)]
pub struct PluginInfo {
    /// Plugin name (null-terminated UTF-8).
    pub name: *const c_char,
    /// Plugin version (null-terminated UTF-8, semver).
    pub version: *const c_char,
    /// Plugin description (null-terminated UTF-8).
    pub description: *const c_char,
    /// API version this plugin was compiled against.
    pub api_version: *const c_char,
    /// Protocol ID this plugin handles (null-terminated UTF-8).
    pub protocol_id: *const c_char,
}

/// Result of a detect call.
#[repr(C)]
pub struct DetectResultFfi {
    /// Whether the plugin can handle this data (0 = no, 1 = yes).
    pub detected: u8,
    /// Confidence (0.0 - 1.0).
    pub confidence: f32,
}

/// Buffer for passing data across the FFI boundary.
#[repr(C)]
pub struct ByteBuffer {
    pub ptr: *const u8,
    pub len: usize,
}

/// Owned buffer allocated by the plugin, freed by the host.
#[repr(C)]
pub struct OwnedBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub capacity: usize,
}
```

### Required Plugin Exports

Every native plugin must export these `extern "C"` functions:

```rust
/// Return plugin metadata.
extern "C" fn prb_plugin_info() -> PluginInfo;

/// Detect whether this plugin can decode the given data.
///
/// `data` points to the first N bytes of the stream.
/// `src_port` and `dst_port` are the transport-layer ports.
/// `transport` is 0 for TCP, 1 for UDP.
extern "C" fn prb_plugin_detect(
    data: ByteBuffer,
    src_port: u16,
    dst_port: u16,
    transport: u8,
) -> DetectResultFfi;

/// Create a new decoder instance. Returns an opaque handle.
extern "C" fn prb_plugin_decoder_create() -> *mut c_void;

/// Decode a byte stream chunk.
///
/// `handle` is the decoder instance from `prb_plugin_decoder_create`.
/// `data` is the stream chunk to decode.
/// `ctx_json` is the DecodeContext serialized as JSON.
///
/// Returns an OwnedBuffer containing JSON-serialized Vec<DebugEventDto>.
/// The host is responsible for freeing the buffer with `prb_plugin_buffer_free`.
extern "C" fn prb_plugin_decode(
    handle: *mut c_void,
    data: ByteBuffer,
    ctx_json: ByteBuffer,
) -> OwnedBuffer;

/// Free a buffer returned by `prb_plugin_decode`.
extern "C" fn prb_plugin_buffer_free(buf: OwnedBuffer);

/// Destroy a decoder instance.
extern "C" fn prb_plugin_decoder_destroy(handle: *mut c_void);
```

### Safe Rust Helper Macros

To make writing plugins ergonomic, provide macros that generate the FFI boilerplate:

```rust
/// Macro to implement all required C ABI exports from a Rust struct.
///
/// Usage:
/// ```rust
/// use prb_plugin_api::*;
///
/// struct MyDecoder { /* ... */ }
///
/// impl PluginDecoder for MyDecoder {
///     fn info() -> PluginMetadata { /* ... */ }
///     fn detect(ctx: &DetectContext) -> Option<f32> { /* ... */ }
///     fn decode(&mut self, data: &[u8], ctx: &DecodeCtx) -> Vec<DebugEventDto> { /* ... */ }
/// }
///
/// prb_export_plugin!(MyDecoder);
/// ```
#[macro_export]
macro_rules! prb_export_plugin {
    ($decoder_type:ty) => {
        // Generates all extern "C" functions wrapping the safe Rust impl
    };
}
```

### Safe Rust Trait (plugin-side)

```rust
/// Safe Rust trait that plugin authors implement.
/// The `prb_export_plugin!` macro generates FFI wrappers.
pub trait PluginDecoder: Send {
    fn info() -> PluginMetadata where Self: Sized;
    fn detect(ctx: &DetectContext) -> Option<f32> where Self: Sized;
    fn new() -> Self where Self: Sized;
    fn decode(&mut self, data: &[u8], ctx: &DecodeCtx) -> Result<Vec<DebugEventDto>, String>;
}

/// Plugin metadata in safe Rust form.
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub protocol_id: String,
}

/// Detection context in safe Rust form.
pub struct DetectContext<'a> {
    pub initial_bytes: &'a [u8],
    pub src_port: u16,
    pub dst_port: u16,
    pub transport: TransportLayer,
}

/// Decode context in safe Rust form.
pub struct DecodeCtx {
    pub src_addr: String,
    pub dst_addr: String,
    pub timestamp_nanos: u64,
    pub metadata: HashMap<String, String>,
}

/// Serializable event DTO (Data Transfer Object) for the FFI boundary.
/// Mirrors prb-core DebugEvent but without internal types.
#[derive(Serialize, Deserialize)]
pub struct DebugEventDto {
    pub timestamp_nanos: u64,
    pub transport: String,
    pub direction: String,
    pub payload_raw: Option<Vec<u8>>,
    pub payload_decoded: Option<serde_json::Value>,
    pub schema_name: Option<String>,
    pub metadata: HashMap<String, String>,
    pub correlation_keys: Vec<CorrelationKeyDto>,
    pub warnings: Vec<String>,
}
```

## `prb-plugin-native` Crate — The Host Loader

### Cargo.toml

```toml
[package]
name = "prb-plugin-native"
version.workspace = true
edition.workspace = true

[dependencies]
prb-core = { path = "../prb-core" }
prb-plugin-api = { path = "../prb-plugin-api" }
prb-detect = { path = "../prb-detect" }
libloading = "0.8"
semver = "1"
tracing.workspace = true
serde_json.workspace = true
```

### `NativePluginLoader`

```rust
/// Loads native shared library plugins and adapts them to the DecoderRegistry.
pub struct NativePluginLoader {
    loaded_plugins: Vec<LoadedPlugin>,
}

struct LoadedPlugin {
    library: libloading::Library,
    info: PluginInfo,
    // Function pointers cached after initial load
    detect_fn: Symbol<extern "C" fn(ByteBuffer, u16, u16, u8) -> DetectResultFfi>,
    create_fn: Symbol<extern "C" fn() -> *mut c_void>,
    decode_fn: Symbol<extern "C" fn(*mut c_void, ByteBuffer, ByteBuffer) -> OwnedBuffer>,
    free_fn: Symbol<extern "C" fn(OwnedBuffer)>,
    destroy_fn: Symbol<extern "C" fn(*mut c_void)>,
}
```

### Loading and Validation

```rust
impl NativePluginLoader {
    /// Load a plugin from a shared library file.
    ///
    /// Validates:
    /// 1. Library loads without error
    /// 2. All required symbols are present
    /// 3. `prb_plugin_info()` returns valid metadata
    /// 4. API version is compatible (same major, host minor >= plugin minor)
    pub fn load(&mut self, path: &Path) -> Result<PluginInfo, PluginError> {
        // Safety: loading native code is inherently unsafe.
        // We validate the API version to ensure ABI compatibility.
        unsafe {
            let lib = libloading::Library::new(path)?;
            // Look up all required symbols...
            // Call prb_plugin_info() and validate...
            // Cache function pointers...
        }
    }

    /// Discover and load all plugins from a directory.
    ///
    /// Scans for files matching the platform's shared library extension:
    /// - Linux: `*.so`
    /// - macOS: `*.dylib`
    /// - Windows: `*.dll`
    pub fn load_directory(&mut self, dir: &Path) -> Vec<Result<PluginInfo, PluginError>> { /* ... */ }
}
```

### Adapter: NativePlugin → DecoderFactory + ProtocolDetector

```rust
/// Adapts a loaded native plugin to the DecoderFactory trait.
struct NativeDecoderFactory {
    plugin: Arc<LoadedPlugin>,
}

impl DecoderFactory for NativeDecoderFactory {
    fn protocol_id(&self) -> &ProtocolId { /* from plugin info */ }
    fn create(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(NativeDecoderInstance::new(self.plugin.clone()))
    }
    // ...
}

/// Adapts a loaded native plugin to the ProtocolDetector trait.
struct NativeProtocolDetector {
    plugin: Arc<LoadedPlugin>,
}

impl ProtocolDetector for NativeProtocolDetector {
    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        // Call plugin's prb_plugin_detect() via FFI
        // Convert DetectResultFfi → DetectionResult
    }
}

/// Wraps a native plugin decoder handle as a ProtocolDecoder.
struct NativeDecoderInstance {
    plugin: Arc<LoadedPlugin>,
    handle: *mut c_void,
}

impl ProtocolDecoder for NativeDecoderInstance {
    fn decode_stream(&mut self, data: &[u8], ctx: &DecodeContext) -> Result<Vec<DebugEvent>, CoreError> {
        // Serialize ctx to JSON
        // Call prb_plugin_decode() via FFI
        // Deserialize OwnedBuffer (JSON) → Vec<DebugEventDto>
        // Convert DebugEventDto → DebugEvent
        // Free OwnedBuffer via prb_plugin_buffer_free()
    }
}

impl Drop for NativeDecoderInstance {
    fn drop(&mut self) {
        // Call prb_plugin_decoder_destroy() to free plugin-side state
    }
}
```

## Plugin Directory Structure

```
~/.prb/plugins/
├── my-thrift-decoder/
│   ├── plugin.toml          # Plugin manifest
│   └── libmy_thrift.dylib   # Shared library
└── my-custom-proto/
    ├── plugin.toml
    └── libcustom_proto.so
```

### `plugin.toml` Manifest

```toml
[plugin]
name = "my-thrift-decoder"
version = "0.1.0"
description = "Apache Thrift protocol decoder for Probe"
api_version = "0.1.0"
protocol_id = "thrift"

[plugin.native]
library = "libmy_thrift.dylib"  # Relative to plugin directory

[plugin.metadata]
author = "Jane Doe"
license = "MIT"
homepage = "https://github.com/janedoe/prb-thrift-decoder"
```

## Example Plugin: Thrift Decoder Skeleton

Provide a template/example plugin to guide plugin authors:

```
examples/
└── plugin-thrift-skeleton/
    ├── Cargo.toml
    ├── src/
    │   └── lib.rs
    └── plugin.toml
```

```rust
// examples/plugin-thrift-skeleton/src/lib.rs
use prb_plugin_api::*;

struct ThriftDecoder {
    // decoder state
}

impl PluginDecoder for ThriftDecoder {
    fn info() -> PluginMetadata {
        PluginMetadata {
            name: "thrift-decoder".into(),
            version: "0.1.0".into(),
            description: "Apache Thrift binary protocol decoder".into(),
            protocol_id: "thrift".into(),
        }
    }

    fn detect(ctx: &DetectContext) -> Option<f32> {
        if ctx.transport != TransportLayer::Tcp { return None; }
        // Thrift binary protocol: version mask 0x80010000
        if ctx.initial_bytes.len() >= 4 {
            let magic = u32::from_be_bytes(ctx.initial_bytes[..4].try_into().ok()?);
            if magic & 0xFFFF0000 == 0x80010000 {
                return Some(0.85);
            }
        }
        None
    }

    fn new() -> Self { ThriftDecoder { /* ... */ } }

    fn decode(&mut self, data: &[u8], ctx: &DecodeCtx) -> Result<Vec<DebugEventDto>, String> {
        // Decode Thrift binary protocol frames...
        Ok(vec![])
    }
}

prb_export_plugin!(ThriftDecoder);
```

```toml
# examples/plugin-thrift-skeleton/Cargo.toml
[package]
name = "prb-plugin-thrift"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
prb-plugin-api = { path = "../../crates/prb-plugin-api" }
```

## Tasks

### T4.1: Create `prb-plugin-api` crate
- Define `PluginInfo`, `DetectResultFfi`, `ByteBuffer`, `OwnedBuffer` (C ABI types)
- Define `PluginDecoder` trait, `PluginMetadata`, `DetectContext`, `DecodeCtx`
- Define `DebugEventDto` and `CorrelationKeyDto` (serializable DTOs)
- Define `API_VERSION` constant
- Implement `prb_export_plugin!` macro

### T4.2: Create `prb-plugin-native` crate
- `NativePluginLoader` struct
- `load()` — load and validate a single plugin
- `load_directory()` — discover plugins in a directory
- Symbol validation (all 5 required exports)
- API version compatibility check

### T4.3: Implement NativePlugin → DecoderFactory adapter
- `NativeDecoderFactory` implementing `DecoderFactory`
- `NativeDecoderInstance` implementing `ProtocolDecoder`
- JSON serialization across FFI boundary
- Proper cleanup in `Drop`

### T4.4: Implement NativePlugin → ProtocolDetector adapter
- `NativeProtocolDetector` implementing `ProtocolDetector`
- Maps `DetectResultFfi` to `DetectionResult`

### T4.5: Plugin manifest parsing
- Define `plugin.toml` schema
- Parse with `toml` crate
- Validate fields

### T4.6: Create example plugin skeleton
- `examples/plugin-thrift-skeleton/`
- Working `Cargo.toml` with `cdylib` crate-type
- Minimal implementation of `PluginDecoder`
- Build and verify it loads

### T4.7: Write tests
- Unit test: load valid plugin → succeeds
- Unit test: load plugin with wrong API version → fails
- Unit test: load plugin missing symbols → fails
- Integration test: compile example plugin, load it, detect + decode
- Test DebugEventDto → DebugEvent conversion roundtrip

### T4.8: DTO ↔ DebugEvent conversion
- `DebugEventDto` → `DebugEvent` (host-side, in `prb-plugin-native`)
- `DebugEvent` → `DebugEventDto` (plugin-side, in `prb-plugin-api`)
- Handle all payload variants, correlation keys, metadata

## Verification

```bash
cargo test -p prb-plugin-api
cargo test -p prb-plugin-native
# Build example plugin
cd examples/plugin-thrift-skeleton && cargo build --release
# Load test
cargo test -p prb-plugin-native -- --test integration
```

Example plugin compiles as cdylib, loads via `NativePluginLoader`, and its
detect + decode functions are callable through the `ProtocolDecoder` trait.
