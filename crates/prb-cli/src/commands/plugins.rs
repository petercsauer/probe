//! Plugin management commands.

use crate::cli::{PluginsArgs, PluginsCommand};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use prb_plugin_api::PluginMetadata;
use prb_plugin_native::{LoadedPlugin, NativePluginLoader};
use prb_plugin_wasm::WasmPluginLoader;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Get the plugin directory path.
fn plugin_dir(custom: Option<&Utf8PathBuf>) -> PathBuf {
    if let Some(dir) = custom {
        return dir.as_std_path().to_path_buf();
    }
    if let Ok(dir) = std::env::var("PRB_PLUGIN_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".prb").join("plugins");
    }
    PathBuf::from(".prb/plugins")
}

/// Plugin manifest structure (plugin.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginManifest {
    plugin: PluginManifestPlugin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginManifestPlugin {
    name: String,
    version: String,
    description: String,
    api_version: String,
    protocol_id: String,
    #[serde(flatten)]
    backend: PluginBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PluginBackend {
    Native { library: String },
    Wasm { module: String },
}

/// Represents a loaded plugin (either native or WASM).
enum LoadedPluginType {
    Native(Arc<LoadedPlugin>),
    Wasm(PluginMetadata, PathBuf),
}

/// Built-in decoder information.
struct BuiltinDecoder {
    name: &'static str,
    protocol_id: &'static str,
    version: &'static str,
    description: &'static str,
}

const BUILTINS: &[BuiltinDecoder] = &[
    BuiltinDecoder {
        name: "gRPC/HTTP2 decoder",
        protocol_id: "grpc",
        version: env!("CARGO_PKG_VERSION"),
        description: "HTTP/2 + HPACK + gRPC LPM",
    },
    BuiltinDecoder {
        name: "ZMQ/ZMTP decoder",
        protocol_id: "zmtp",
        version: env!("CARGO_PKG_VERSION"),
        description: "ZMTP 3.0/3.1 greeting + frames",
    },
    BuiltinDecoder {
        name: "DDS/RTPS decoder",
        protocol_id: "rtps",
        version: env!("CARGO_PKG_VERSION"),
        description: "RTPS + SEDP discovery",
    },
];

/// Run the plugins command.
pub fn run_plugins(args: PluginsArgs, plugin_dir_override: Option<&Utf8PathBuf>) -> Result<()> {
    match args.command {
        PluginsCommand::List => run_list(plugin_dir_override),
        PluginsCommand::Info { name } => run_info(&name, plugin_dir_override),
        PluginsCommand::Install { path, name } => {
            run_install(&path, name.as_deref(), plugin_dir_override)
        }
        PluginsCommand::Remove { name } => run_remove(&name, plugin_dir_override),
    }
}

fn run_list(plugin_dir_override: Option<&Utf8PathBuf>) -> Result<()> {
    let dir = plugin_dir(plugin_dir_override);

    // Show built-in decoders
    println!("Built-in Decoders:");
    for builtin in BUILTINS {
        println!(
            "  {:<12} {:<28} {:<8} {}",
            builtin.protocol_id, builtin.name, builtin.version, builtin.description
        );
    }
    println!();

    // Load plugins from directory
    let plugins = load_all_plugins(&dir)?;

    if plugins.is_empty() {
        println!("No plugins installed.");
    } else {
        println!("Loaded Plugins:");
        for plugin in plugins {
            match plugin {
                LoadedPluginType::Native(p) => {
                    let meta = p.metadata();
                    println!(
                        "  {:<12} {:<28} {:<8} native  (loaded)",
                        meta.protocol_id, meta.name, meta.version
                    );
                }
                LoadedPluginType::Wasm(meta, path) => {
                    println!(
                        "  {:<12} {:<28} {:<8} wasm    {}",
                        meta.protocol_id,
                        meta.name,
                        meta.version,
                        path.display()
                    );
                }
            }
        }
    }

    println!();
    println!("Plugin directory: {}", dir.display());

    Ok(())
}

fn run_info(name: &str, plugin_dir_override: Option<&Utf8PathBuf>) -> Result<()> {
    // Check if it's a built-in decoder
    for builtin in BUILTINS {
        if builtin.protocol_id == name || builtin.name.to_lowercase().contains(&name.to_lowercase())
        {
            println!("Name:         {}", builtin.name);
            println!("Protocol ID:  {}", builtin.protocol_id);
            println!("Version:      {}", builtin.version);
            println!("Source:       built-in");
            println!("Description:  {}", builtin.description);
            println!();
            println!("Detection:");
            println!("  Transport:  TCP");

            match builtin.protocol_id {
                "grpc" => {
                    println!("  Magic:      \"PRI * HTTP/2.0\\r\\n\\r\\nSM\\r\\n\\r\\n\" (HTTP/2 connection preface)");
                    println!("  Ports:      50051, 443 (with TLS)");
                    println!("  Confidence: 0.95 (magic bytes), 0.5 (port only)");
                    println!();
                    println!("Capabilities:");
                    println!("  ✓ HTTP/2 frame parsing");
                    println!("  ✓ HPACK header decompression");
                    println!("  ✓ gRPC LPM (5-byte header)");
                    println!("  ✓ gzip/deflate decompression");
                    println!("  ✓ Trailers (grpc-status, grpc-message)");
                }
                "zmtp" => {
                    println!("  Magic:      0xFF + length + 0x7F + \"ZMTP\" (ZMTP 3.x greeting)");
                    println!("  Ports:      5555, 5556");
                    println!("  Confidence: 0.95 (magic bytes), 0.4 (port only)");
                    println!();
                    println!("Capabilities:");
                    println!("  ✓ ZMTP 3.0/3.1 greeting");
                    println!("  ✓ Frame parsing");
                    println!("  ✓ Socket type detection");
                }
                "rtps" => {
                    println!("  Magic:      \"RTPS\" (RTPS header)");
                    println!("  Ports:      7400-7500 (discovery)");
                    println!("  Confidence: 0.95 (magic bytes), 0.6 (port only)");
                    println!();
                    println!("Capabilities:");
                    println!("  ✓ RTPS message parsing");
                    println!("  ✓ SEDP discovery");
                    println!("  ✓ Data submessages");
                }
                _ => {}
            }

            return Ok(());
        }
    }

    // Check loaded plugins
    let dir = plugin_dir(plugin_dir_override);
    let plugins = load_all_plugins(&dir)?;

    for plugin in plugins {
        let (meta, source) = match plugin {
            LoadedPluginType::Native(p) => (p.metadata().clone(), "native"),
            LoadedPluginType::Wasm(meta, _) => (meta, "wasm"),
        };

        if meta.protocol_id == name || meta.name.to_lowercase().contains(&name.to_lowercase()) {
            println!("Name:         {}", meta.name);
            println!("Protocol ID:  {}", meta.protocol_id);
            println!("Version:      {}", meta.version);
            println!("Source:       {} plugin", source);
            println!("API Version:  {}", meta.api_version);
            println!("Description:  {}", meta.description);
            return Ok(());
        }
    }

    anyhow::bail!("Decoder '{}' not found", name);
}

fn run_install(
    path: &Utf8PathBuf,
    name_override: Option<&str>,
    plugin_dir_override: Option<&Utf8PathBuf>,
) -> Result<()> {
    let path_std = path.as_std_path();
    if !path_std.exists() {
        anyhow::bail!("Plugin file not found: {}", path);
    }

    println!("Installing plugin from {}...", path);

    // Determine type from extension
    let ext = path_std
        .extension()
        .and_then(|e| e.to_str())
        .context("Failed to determine file extension")?;

    let (backend, info) = match ext {
        "so" | "dylib" | "dll" => {
            println!("Validating native plugin...");
            let mut loader = NativePluginLoader::new();
            let plugin = loader.load(path_std).context("Failed to load native plugin")?;
            let info = plugin.metadata().clone();
            let backend = PluginBackend::Native {
                library: path_std
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            };
            (backend, info)
        }
        "wasm" => {
            println!("Validating WASM plugin...");
            let mut loader = WasmPluginLoader::new();
            let info = loader.load(path_std).context("Failed to load WASM plugin")?;
            let backend = PluginBackend::Wasm {
                module: path_std
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            };
            (backend, info)
        }
        _ => {
            anyhow::bail!(
                "Unknown plugin file type: .{}. Expected .so, .dylib, .dll, or .wasm",
                ext
            );
        }
    };

    println!("  Name:       {}", info.name);
    println!("  Version:    {}", info.version);
    println!("  API:        {} (compatible)", info.api_version);
    println!("  Protocol:   {}", info.protocol_id);
    println!(
        "  Type:       {}",
        match backend {
            PluginBackend::Native { .. } => "native",
            PluginBackend::Wasm { .. } => "wasm",
        }
    );

    let plugin_name = name_override.unwrap_or(&info.name);
    let dir = plugin_dir(plugin_dir_override);
    let dest_dir = dir.join(plugin_name);

    // Create plugin directory
    std::fs::create_dir_all(&dest_dir)
        .with_context(|| format!("Failed to create directory: {}", dest_dir.display()))?;

    // Copy the plugin binary
    let dest_file = dest_dir.join(path_std.file_name().unwrap());
    std::fs::copy(path_std, &dest_file)
        .with_context(|| format!("Failed to copy plugin to {}", dest_file.display()))?;

    // Write plugin.toml manifest
    let manifest = PluginManifest {
        plugin: PluginManifestPlugin {
            name: plugin_name.to_string(),
            version: info.version.clone(),
            description: info.description.clone(),
            api_version: info.api_version.clone(),
            protocol_id: info.protocol_id.clone(),
            backend,
        },
    };
    let toml_str =
        toml::to_string_pretty(&manifest).context("Failed to serialize plugin manifest")?;
    std::fs::write(dest_dir.join("plugin.toml"), toml_str)
        .context("Failed to write plugin.toml")?;

    println!("Installed to {}", dest_dir.display());

    Ok(())
}

fn run_remove(name: &str, plugin_dir_override: Option<&Utf8PathBuf>) -> Result<()> {
    let dir = plugin_dir(plugin_dir_override);
    let dest_dir = dir.join(name);

    if !dest_dir.exists() {
        anyhow::bail!(
            "Plugin '{}' not found at {}",
            name,
            dest_dir.display()
        );
    }

    println!("Removing plugin '{}' from {}...", name, dest_dir.display());
    std::fs::remove_dir_all(&dest_dir)
        .with_context(|| format!("Failed to remove directory: {}", dest_dir.display()))?;
    println!("Removed.");

    Ok(())
}

/// Load all plugins from the plugin directory.
fn load_all_plugins(dir: &Path) -> Result<Vec<LoadedPluginType>> {
    let mut plugins = Vec::new();

    if !dir.exists() {
        return Ok(plugins);
    }

    let mut native_loader = NativePluginLoader::new();
    let mut wasm_loader = WasmPluginLoader::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let manifest_path = entry.path().join("plugin.toml");
        if !manifest_path.exists() {
            continue;
        }

        let manifest_str = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
        let manifest: PluginManifest = toml::from_str(&manifest_str)
            .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;

        match &manifest.plugin.backend {
            PluginBackend::Native { library } => {
                let lib_path = entry.path().join(library);
                match native_loader.load(&lib_path) {
                    Ok(plugin) => {
                        tracing::debug!(
                            name = %plugin.metadata().name,
                            version = %plugin.metadata().version,
                            "Loaded native plugin"
                        );
                        plugins.push(LoadedPluginType::Native(plugin));
                    }
                    Err(e) => {
                        tracing::warn!(
                            plugin = %manifest.plugin.name,
                            error = %e,
                            "Failed to load native plugin"
                        );
                    }
                }
            }
            PluginBackend::Wasm { module } => {
                let wasm_path = entry.path().join(module);
                match wasm_loader.load(&wasm_path) {
                    Ok(info) => {
                        tracing::debug!(
                            name = %info.name,
                            version = %info.version,
                            "Loaded WASM plugin"
                        );
                        plugins.push(LoadedPluginType::Wasm(info, wasm_path));
                    }
                    Err(e) => {
                        tracing::warn!(
                            plugin = %manifest.plugin.name,
                            error = %e,
                            "Failed to load WASM plugin"
                        );
                    }
                }
            }
        }
    }

    Ok(plugins)
}
