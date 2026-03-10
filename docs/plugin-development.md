# Plugin Development

PRB supports extending protocol decoding with custom plugins. Plugins can be distributed as native shared libraries (`.so`/`.dylib`/`.dll`) or WebAssembly modules (`.wasm`).

## Plugin Contract

Every plugin must implement three operations:

1. **Info** -- Return plugin metadata (name, version, protocol ID, API version)
2. **Detect** -- Given the first bytes of a stream plus port information, return whether this plugin can decode the data and a confidence score (0.0--1.0)
3. **Decode** -- Given a raw byte stream and context (addresses, timestamps, metadata), return decoded `DebugEventDto` objects

## API Version

Plugins declare the API version they were compiled against. PRB validates compatibility at load time:

- **Major version mismatch** -- plugin is rejected
- **Plugin minor version > host minor version** -- plugin is rejected (requires newer host)
- **Plugin minor version <= host minor version** -- compatible (backward-compatible changes only)

Current API version: `0.1.0`

## Native Plugin (Shared Library)

Native plugins are Rust crates compiled as `cdylib`. They use the `prb_export_plugin!` macro to generate the required C FFI exports.

### Step 1: Create a new crate

```bash
cargo new --lib my-protocol-decoder
cd my-protocol-decoder
```

### Step 2: Configure Cargo.toml

```toml
[package]
name = "my-protocol-decoder"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
prb-plugin-api = { path = "../prb/crates/prb-plugin-api" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Step 3: Implement the decoder

```rust
use prb_plugin_api::*;
use prb_plugin_api::native::PluginDecoder;

pub struct MyDecoder {
    // Per-stream state (e.g., partial frame buffer)
}

impl PluginDecoder for MyDecoder {
    fn info() -> PluginMetadata {
        PluginMetadata {
            name: "my-protocol".into(),
            version: "0.1.0".into(),
            description: "Decoder for My Custom Protocol".into(),
            protocol_id: "my-proto".into(),
            api_version: API_VERSION.into(),
        }
    }

    fn detect(ctx: &DetectContext) -> Option<f32> {
        // Check if the initial bytes match your protocol's magic bytes
        let bytes = &ctx.initial_bytes;
        if bytes.len() >= 4 && &bytes[0..4] == b"MYPR" {
            Some(0.9) // High confidence
        } else if ctx.dst_port == 9999 {
            Some(0.3) // Low confidence, port-based hint
        } else {
            None // Cannot handle this stream
        }
    }

    fn new() -> Self {
        Self {}
    }

    fn decode(
        &mut self,
        data: &[u8],
        ctx: &DecodeCtx,
    ) -> Result<Vec<DebugEventDto>, String> {
        // Parse the data and produce events
        let event = DebugEventDto {
            transport: "my-proto".into(),
            direction: "inbound".into(),
            timestamp_nanos: ctx.timestamp_nanos,
            payload_json: Some(serde_json::json!({
                "message": "decoded content"
            })),
            metadata: ctx.metadata.clone(),
            correlation_keys: vec![],
            warnings: vec![],
        };
        Ok(vec![event])
    }
}

// Generate all required C FFI exports
prb_export_plugin!(MyDecoder);
```

### Step 4: Build

```bash
cargo build --release
```

The plugin will be at `target/release/libmy_protocol_decoder.dylib` (macOS), `.so` (Linux), or `.dll` (Windows).

### Step 5: Install

```bash
prb plugins install target/release/libmy_protocol_decoder.dylib

# Verify
prb plugins list
```

## WASM Plugin

WASM plugins are compiled to WebAssembly and run in a sandboxed wasmtime runtime. Data exchange happens via JSON serialization.

### Step 1: Add the wasm32 target

```bash
rustup target add wasm32-wasi
```

### Step 2: Configure Cargo.toml

```toml
[package]
name = "my-wasm-decoder"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
prb-plugin-api = { path = "../prb/crates/prb-plugin-api", features = ["wasm-pdk"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Step 3: Implement the WASM exports

WASM plugins export functions that receive and return JSON strings:

```rust
use prb_plugin_api::*;

#[no_mangle]
pub extern "C" fn prb_plugin_info_json() -> *mut u8 {
    let info = PluginMetadata {
        name: "my-wasm-decoder".into(),
        version: "0.1.0".into(),
        description: "WASM protocol decoder".into(),
        protocol_id: "my-proto".into(),
        api_version: API_VERSION.into(),
    };
    let json = serde_json::to_string(&info).unwrap();
    let bytes = json.into_bytes();
    let ptr = bytes.as_ptr() as *mut u8;
    std::mem::forget(bytes);
    ptr
}

// Similar exports for detect and decode...
```

### Step 4: Build

```bash
cargo build --release --target wasm32-wasi
```

### Step 5: Install

```bash
prb plugins install target/wasm32-wasi/release/my_wasm_decoder.wasm
```

## Plugin Directory

Plugins are loaded from `~/.prb/plugins/` by default. Override with:

```bash
prb --plugin-dir /path/to/plugins ingest capture.pcap
```

Disable all plugins:

```bash
prb --no-plugins ingest capture.pcap
```

## Plugin Management

```bash
# List all decoders (built-in + plugins)
prb plugins list

# Show details about a specific decoder
prb plugins info my-proto

# Install a plugin
prb plugins install /path/to/plugin.so --name my-decoder

# Remove a plugin
prb plugins remove my-decoder
```

## Testing Plugins

Test your plugin with a PCAP containing your protocol's traffic:

```bash
# Install and verify detection
prb plugins install target/release/libmy_decoder.dylib
prb plugins list

# Test decoding
prb ingest my-protocol-traffic.pcap

# Force your protocol if auto-detection needs tuning
prb ingest traffic.pcap --protocol my-proto
```

For unit testing within your plugin crate, test the `PluginDecoder` trait methods directly without going through FFI.

## FFI Reference

Native plugins export these C functions (generated by `prb_export_plugin!`):

| Function | Signature | Purpose |
|----------|-----------|---------|
| `prb_plugin_info` | `() -> PluginInfo` | Return plugin metadata |
| `prb_plugin_detect` | `(ByteBuffer, u16, u16, u8) -> DetectResultFfi` | Protocol detection |
| `prb_plugin_decoder_create` | `() -> DecoderHandle` | Create decoder instance |
| `prb_plugin_decode` | `(DecoderHandle, ByteBuffer, ByteBuffer) -> OwnedBuffer` | Decode data chunk |
| `prb_plugin_decoder_destroy` | `(DecoderHandle) -> ()` | Free decoder instance |
| `prb_plugin_buffer_free` | `(OwnedBuffer) -> ()` | Free returned buffer |
