---
segment: 29
title: "Plugin Management CLI"
crate: prb-cli
status: pending
depends_on: [27, 28]
estimated_effort: "3-4 hours"
risk: 3/10
---

# Segment 6: Plugin Management CLI

## Objective

Add `prb plugins` subcommands to the CLI for listing, inspecting, installing,
and removing protocol decoder plugins. Also integrate plugin loading into the
`prb ingest` pipeline so plugins are automatically discovered and used.

## CLI Commands

### `prb plugins list`

Lists all available decoders (built-in + loaded plugins).

```
$ prb plugins list

Built-in Decoders:
  grpc        gRPC/HTTP2 decoder       v0.1.0   HTTP/2 + HPACK + gRPC LPM
  zmtp        ZMQ/ZMTP decoder         v0.1.0   ZMTP 3.0/3.1 greeting + frames
  rtps        DDS/RTPS decoder         v0.1.0   RTPS + SEDP discovery

Loaded Plugins:
  thrift      Apache Thrift decoder    v0.1.0   native  ~/.prb/plugins/thrift/libthrift.dylib
  capnproto   Cap'n Proto decoder      v0.2.0   wasm    ~/.prb/plugins/capnproto/decoder.wasm

Plugin directory: ~/.prb/plugins/
```

### `prb plugins info <name>`

Detailed information about a decoder.

```
$ prb plugins info grpc

Name:         gRPC/HTTP2 decoder
Protocol ID:  grpc
Version:      0.1.0
Source:       built-in
Description:  Decodes gRPC over HTTP/2 with HPACK header compression and
              Length-Prefixed Message framing

Detection:
  Transport:  TCP
  Magic:      "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" (HTTP/2 connection preface)
  Ports:      50051, 443 (with TLS)
  Confidence: 0.95 (magic bytes), 0.5 (port only)

Capabilities:
  ✓ HTTP/2 frame parsing
  ✓ HPACK header decompression
  ✓ gRPC LPM (5-byte header)
  ✓ gzip/deflate decompression
  ✓ Trailers (grpc-status, grpc-message)
  ✓ Mid-stream HPACK degradation
```

### `prb plugins install <path-or-url>`

Install a plugin from a local path or URL.

```
$ prb plugins install ./target/release/libmy_decoder.dylib
Installing plugin from ./target/release/libmy_decoder.dylib...
Validating plugin...
  Name:       my-decoder
  Version:    0.1.0
  API:        0.1.0 (compatible)
  Protocol:   my-proto
  Type:       native
Installed to ~/.prb/plugins/my-decoder/

$ prb plugins install ./target/wasm32-unknown-unknown/release/my_decoder.wasm
Installing plugin from ./target/wasm32-unknown-unknown/release/my_decoder.wasm...
Validating plugin...
  Name:       my-wasm-decoder
  Version:    0.1.0
  API:        0.1.0 (compatible)
  Protocol:   my-proto
  Type:       wasm
Installed to ~/.prb/plugins/my-wasm-decoder/
```

### `prb plugins remove <name>`

Remove an installed plugin.

```
$ prb plugins remove my-decoder
Removing plugin 'my-decoder' from ~/.prb/plugins/my-decoder/...
Removed.
```

## Implementation

### Plugin Directory

Default: `~/.prb/plugins/`. Configurable via `PRB_PLUGIN_DIR` env var or
`--plugin-dir` CLI flag.

```rust
fn plugin_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PRB_PLUGIN_DIR") {
        PathBuf::from(dir)
    } else if let Some(home) = dirs::home_dir() {
        home.join(".prb").join("plugins")
    } else {
        PathBuf::from(".prb/plugins")
    }
}
```

### Plugin Discovery on Startup

When `prb ingest` runs, it automatically:
1. Creates a `DecoderRegistry::with_builtins()`
2. Scans the plugin directory for plugin manifests
3. Loads each discovered plugin (native or WASM)
4. Registers plugin decoders and detectors with the registry

```rust
fn load_plugins(registry: &mut DecoderRegistry) -> Result<()> {
    let plugin_dir = plugin_dir();
    if !plugin_dir.exists() {
        return Ok(());
    }

    let mut native_loader = NativePluginLoader::new();
    let mut wasm_loader = WasmPluginLoader::new();

    for entry in std::fs::read_dir(&plugin_dir)? {
        let entry = entry?;
        let manifest_path = entry.path().join("plugin.toml");
        if !manifest_path.exists() { continue; }

        let manifest: PluginManifest = toml::from_str(
            &std::fs::read_to_string(&manifest_path)?
        )?;

        match &manifest.plugin.backend {
            PluginBackend::Native { library } => {
                let lib_path = entry.path().join(library);
                match native_loader.load(&lib_path) {
                    Ok(info) => {
                        tracing::info!("Loaded native plugin: {} v{}", info.name, info.version);
                        registry.register_native_plugin(/* ... */);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load plugin {}: {}", manifest.plugin.name, e);
                    }
                }
            }
            PluginBackend::Wasm { module } => {
                let wasm_path = entry.path().join(module);
                match wasm_loader.load(&wasm_path) {
                    Ok(info) => {
                        tracing::info!("Loaded WASM plugin: {} v{}", info.name, info.version);
                        registry.register_wasm_plugin(/* ... */);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load plugin {}: {}", manifest.plugin.name, e);
                    }
                }
            }
        }
    }

    Ok(())
}
```

### CLI Structure (clap)

```rust
#[derive(Parser)]
#[command(name = "prb")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Plugin directory (default: ~/.prb/plugins/)
    #[arg(long, global = true)]
    plugin_dir: Option<PathBuf>,

    /// Disable automatic plugin loading
    #[arg(long, global = true)]
    no_plugins: bool,
}

#[derive(Subcommand)]
enum Commands {
    Ingest(IngestArgs),
    Inspect(InspectArgs),
    Schemas(SchemaArgs),
    /// Manage protocol decoder plugins
    Plugins(PluginsArgs),
}

#[derive(Args)]
struct PluginsArgs {
    #[command(subcommand)]
    command: PluginsCommand,
}

#[derive(Subcommand)]
enum PluginsCommand {
    /// List all available decoders and plugins
    List,
    /// Show detailed info about a decoder
    Info { name: String },
    /// Install a plugin from a file path
    Install {
        /// Path to .so/.dylib/.dll or .wasm file
        path: PathBuf,
        /// Optional plugin name (defaults to plugin's self-reported name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove an installed plugin
    Remove { name: String },
}
```

### `plugins install` Logic

```rust
fn install_plugin(path: &Path, name: Option<&str>) -> Result<()> {
    let plugin_dir = plugin_dir();

    // Determine type from extension
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let backend = match ext {
        "so" | "dylib" | "dll" => PluginBackend::Native { library: path.file_name().unwrap().into() },
        "wasm" => PluginBackend::Wasm { module: path.file_name().unwrap().into() },
        _ => anyhow::bail!("Unknown plugin file type: .{ext}. Expected .so, .dylib, .dll, or .wasm"),
    };

    // Validate by loading
    let info = match &backend {
        PluginBackend::Native { .. } => {
            let mut loader = NativePluginLoader::new();
            loader.load(path)?
        }
        PluginBackend::Wasm { .. } => {
            let mut loader = WasmPluginLoader::new();
            loader.load(path)?
        }
    };

    let plugin_name = name.map(String::from).unwrap_or_else(|| info.name.clone());

    // Create plugin directory
    let dest_dir = plugin_dir.join(&plugin_name);
    std::fs::create_dir_all(&dest_dir)?;

    // Copy the plugin binary
    let dest_file = dest_dir.join(path.file_name().unwrap());
    std::fs::copy(path, &dest_file)?;

    // Write plugin.toml manifest
    let manifest = PluginManifest {
        plugin: PluginManifestPlugin {
            name: plugin_name.clone(),
            version: info.version.clone(),
            description: info.description.clone(),
            api_version: API_VERSION.into(),
            protocol_id: info.protocol_id.clone(),
            backend,
        },
    };
    let toml_str = toml::to_string_pretty(&manifest)?;
    std::fs::write(dest_dir.join("plugin.toml"), toml_str)?;

    println!("Installed plugin '{}' to {}", plugin_name, dest_dir.display());
    Ok(())
}
```

### `plugins remove` Logic

```rust
fn remove_plugin(name: &str) -> Result<()> {
    let dest_dir = plugin_dir().join(name);
    if !dest_dir.exists() {
        anyhow::bail!("Plugin '{}' not found at {}", name, dest_dir.display());
    }
    std::fs::remove_dir_all(&dest_dir)?;
    println!("Removed plugin '{}'", name);
    Ok(())
}
```

## Updated `prb ingest` Flow

```rust
fn run_ingest(args: IngestArgs, cli: &Cli) -> Result<()> {
    let mut registry = DecoderRegistry::with_builtins();

    // Load plugins unless disabled
    if !cli.no_plugins {
        let dir = cli.plugin_dir.clone().unwrap_or_else(plugin_dir);
        if let Err(e) = load_plugins(&mut registry, &dir) {
            tracing::warn!("Plugin loading failed: {}", e);
        }
    }

    // Apply protocol override
    if let Some(protocol) = &args.protocol {
        registry.set_protocol_override(ProtocolId::new(protocol));
    }

    // Apply custom port mappings
    if let Some(port_map) = &args.port_map {
        registry.apply_port_mappings(parse_port_map(port_map)?);
    }

    let mut adapter = PcapCaptureAdapter::with_registry(
        args.input.clone(),
        args.tls_keylog.clone(),
        registry,
    );

    // ... rest of ingest
}
```

## New Dependencies for `prb-cli`

```toml
[dependencies]
# ... existing
prb-detect = { path = "../prb-detect" }
prb-plugin-native = { path = "../prb-plugin-native" }
prb-plugin-wasm = { path = "../prb-plugin-wasm" }
dirs = "5"
toml = "0.8"
```

## Tasks

### T6.1: Add `Plugins` subcommand to CLI
- Define `PluginsArgs`, `PluginsCommand` enum
- Add to main `Commands` enum
- Add `--plugin-dir` and `--no-plugins` global flags

### T6.2: Implement `plugins list`
- Show built-in decoders with protocol, version, description
- Show loaded plugins with source type and path
- Format as aligned table

### T6.3: Implement `plugins info <name>`
- Look up decoder by name or protocol ID
- Show detection details (transport, magic bytes, ports, confidence)
- Show capabilities summary

### T6.4: Implement `plugins install`
- Detect plugin type from file extension
- Validate by trial-loading
- Copy to plugin directory
- Generate `plugin.toml` manifest
- Test: install native plugin → appears in `plugins list`
- Test: install WASM plugin → appears in `plugins list`

### T6.5: Implement `plugins remove`
- Remove plugin directory
- Test: remove installed plugin → no longer in `plugins list`

### T6.6: Integrate plugin loading into `prb ingest`
- Load plugins from plugin directory on startup
- Respect `--no-plugins` flag
- Respect `--plugin-dir` override
- Test: installed plugin is used during ingest

### T6.7: End-to-end integration tests
- Install example native plugin → ingest data → verify decoded events
- Install example WASM plugin → ingest data → verify decoded events
- `prb plugins list` shows all decoders
- `--no-plugins` flag disables plugin loading
- `--protocol` override still works with plugins loaded

## Verification

```bash
cargo test -p prb-cli
cargo test --workspace

# Manual verification
cargo run -- plugins list
cargo run -- plugins install ./target/release/libmy_plugin.dylib
cargo run -- plugins list   # Shows new plugin
cargo run -- plugins info my-plugin
cargo run -- ingest capture.pcap   # Uses plugin if protocol matches
cargo run -- plugins remove my-plugin
```

Full round-trip: install → list → ingest (with plugin) → remove works correctly.
