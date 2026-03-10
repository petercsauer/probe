//! WASM plugin loader and manager.

use crate::error::PluginError;
use extism::{Manifest, Plugin, Wasm};
use prb_plugin_api::{validate_api_version, PluginMetadata};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Manages loaded WASM plugin instances.
pub struct WasmPluginLoader {
    plugins: Vec<WasmPlugin>,
}

pub struct WasmPlugin {
    /// Path to the .wasm file.
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Cached metadata from prb_plugin_info().
    #[allow(dead_code)]
    pub info: PluginMetadata,
}

impl WasmPluginLoader {
    /// Create a new empty loader.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Load a WASM plugin from a .wasm file.
    ///
    /// Validates:
    /// 1. File exists and is valid WASM
    /// 2. Required exports are present (prb_plugin_info, prb_plugin_detect, prb_plugin_decode)
    /// 3. prb_plugin_info() returns valid metadata
    /// 4. API version is compatible
    pub fn load(&mut self, path: &Path) -> Result<PluginMetadata, PluginError> {
        if !path.exists() {
            return Err(PluginError::Load(format!(
                "Plugin file not found: {}",
                path.display()
            )));
        }

        debug!(path = %path.display(), "Loading WASM plugin");

        // Create a temporary plugin instance to validate exports and get metadata
        let manifest = Manifest::new([Wasm::file(path)])
            .with_memory_max(16) // Minimal memory for validation
            .with_timeout(std::time::Duration::from_secs(5));

        let mut plugin = Plugin::new(&manifest, [], true)
            .map_err(|e| PluginError::Load(format!("Failed to instantiate plugin: {}", e)))?;

        // Validate required exports
        self.validate_exports(&plugin)?;

        // Call prb_plugin_info to get metadata
        let info_json = plugin
            .call::<&str, String>("prb_plugin_info", "")
            .map_err(|e| {
                PluginError::Execution(format!("Failed to call prb_plugin_info: {}", e))
            })?;

        let info: PluginMetadata = serde_json::from_str(&info_json).map_err(|e| {
            PluginError::Load(format!("Invalid metadata from prb_plugin_info: {}", e))
        })?;

        // Validate API version
        validate_api_version(&info.api_version)
            .map_err(PluginError::ApiVersion)?;

        debug!(
            plugin = %info.name,
            version = %info.version,
            protocol = %info.protocol_id,
            "Successfully loaded WASM plugin"
        );

        self.plugins.push(WasmPlugin {
            path: path.to_path_buf(),
            info: info.clone(),
        });

        Ok(info)
    }

    /// Load all .wasm files from a directory.
    pub fn load_directory(&mut self, dir: &Path) -> Vec<Result<PluginMetadata, PluginError>> {
        let mut results = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!(dir = %dir.display(), error = %e, "Failed to read plugin directory");
                results.push(Err(PluginError::Io(e)));
                return results;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                results.push(self.load(&path));
            }
        }

        results
    }

    /// Get all loaded plugins.
    pub fn plugins(&self) -> &[WasmPlugin] {
        &self.plugins
    }

    /// Validate that all required exports are present.
    fn validate_exports(&self, plugin: &Plugin) -> Result<(), PluginError> {
        let required = ["prb_plugin_info", "prb_plugin_detect", "prb_plugin_decode"];

        for export_name in &required {
            if !plugin.function_exists(export_name) {
                return Err(PluginError::MissingExport(export_name.to_string()));
            }
        }

        Ok(())
    }
}

impl Default for WasmPluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_creation() {
        let loader = WasmPluginLoader::new();
        assert_eq!(loader.plugins().len(), 0);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let mut loader = WasmPluginLoader::new();
        let result = loader.load(Path::new("/nonexistent/plugin.wasm"));
        assert!(result.is_err());
    }
}
