//! WASM plugin loader and manager.

use crate::error::PluginError;
use extism::{Manifest, Plugin, Wasm};
use prb_plugin_api::{PluginMetadata, validate_api_version};
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
    /// Cached metadata from `prb_plugin_info()`.
    #[allow(dead_code)]
    pub info: PluginMetadata,
}

/// Validate plugin metadata for completeness and correctness.
fn validate_metadata(metadata: &PluginMetadata) -> Result<(), PluginError> {
    if metadata.name.is_empty() {
        return Err(PluginError::InvalidMetadata("name is required".to_string()));
    }
    if metadata.version.is_empty() {
        return Err(PluginError::InvalidMetadata(
            "version is required".to_string(),
        ));
    }
    if metadata.protocol_id.is_empty() {
        return Err(PluginError::InvalidMetadata(
            "protocol_id is required".to_string(),
        ));
    }

    // Validate version is valid semver
    semver::Version::parse(&metadata.version)
        .map_err(|e| PluginError::InvalidMetadata(format!("Invalid version format: {e}")))?;

    Ok(())
}

impl WasmPluginLoader {
    /// Create a new empty loader.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Load a WASM plugin from a .wasm file.
    ///
    /// Validates:
    /// 1. File exists and is valid WASM
    /// 2. Required exports are present (`prb_plugin_info`, `prb_plugin_detect`, `prb_plugin_decode`)
    /// 3. `prb_plugin_info()` returns valid metadata
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
            .with_memory_max(256) // 16MB for validation
            .with_timeout(std::time::Duration::from_secs(5));

        let mut plugin = Plugin::new(&manifest, [], true)
            .map_err(|e| PluginError::Load(format!("Failed to instantiate plugin: {e}")))?;

        // Validate required exports
        self.validate_exports(&plugin)?;

        // Call prb_plugin_info to get metadata
        let info_json = plugin
            .call::<&str, String>("prb_plugin_info", "")
            .map_err(|e| PluginError::Execution(format!("Failed to call prb_plugin_info: {e}")))?;

        let info: PluginMetadata = serde_json::from_str(&info_json).map_err(|e| {
            PluginError::Load(format!("Invalid metadata from prb_plugin_info: {e}"))
        })?;

        // Validate API version
        validate_api_version(&info.api_version).map_err(PluginError::ApiVersion)?;

        // Validate metadata completeness
        validate_metadata(&info)?;

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
    #[must_use]
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

    fn create_test_metadata(name: &str, version: &str, protocol_id: &str) -> PluginMetadata {
        PluginMetadata {
            name: name.to_string(),
            version: version.to_string(),
            description: "Test plugin".to_string(),
            protocol_id: protocol_id.to_string(),
            api_version: "0.1.0".to_string(),
        }
    }

    #[test]
    fn test_validate_metadata_success() {
        let metadata = create_test_metadata("test-plugin", "1.0.0", "test-proto");
        assert!(validate_metadata(&metadata).is_ok());
    }

    #[test]
    fn test_validate_metadata_empty_name() {
        let metadata = create_test_metadata("", "1.0.0", "test-proto");
        let result = validate_metadata(&metadata);
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::InvalidMetadata(msg) => {
                assert!(msg.contains("name is required"));
            }
            _ => panic!("Expected InvalidMetadata error"),
        }
    }

    #[test]
    fn test_validate_metadata_empty_version() {
        let metadata = create_test_metadata("test-plugin", "", "test-proto");
        let result = validate_metadata(&metadata);
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::InvalidMetadata(msg) => {
                assert!(msg.contains("version is required"));
            }
            _ => panic!("Expected InvalidMetadata error"),
        }
    }

    #[test]
    fn test_validate_metadata_empty_protocol_id() {
        let metadata = create_test_metadata("test-plugin", "1.0.0", "");
        let result = validate_metadata(&metadata);
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::InvalidMetadata(msg) => {
                assert!(msg.contains("protocol_id is required"));
            }
            _ => panic!("Expected InvalidMetadata error"),
        }
    }

    #[test]
    fn test_validate_metadata_invalid_semver() {
        let metadata = create_test_metadata("test-plugin", "not-a-version", "test-proto");
        let result = validate_metadata(&metadata);
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::InvalidMetadata(msg) => {
                assert!(msg.contains("Invalid version format"));
            }
            _ => panic!("Expected InvalidMetadata error"),
        }
    }

    #[test]
    fn test_validate_metadata_valid_semver_variants() {
        // Test various valid semver formats
        let test_cases = vec![
            "1.0.0",
            "0.1.0",
            "1.2.3",
            "1.0.0-alpha",
            "1.0.0-beta.1",
            "1.0.0+build123",
            "1.0.0-rc.1+build456",
        ];

        for version in test_cases {
            let metadata = create_test_metadata("test-plugin", version, "test-proto");
            assert!(
                validate_metadata(&metadata).is_ok(),
                "Version '{}' should be valid",
                version
            );
        }
    }

    #[test]
    fn test_loader_creation() {
        let loader = WasmPluginLoader::new();
        assert_eq!(loader.plugins().len(), 0);
    }

    #[test]
    fn test_loader_default() {
        let loader = WasmPluginLoader::default();
        assert_eq!(loader.plugins().len(), 0);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let mut loader = WasmPluginLoader::new();
        let result = loader.load(Path::new("/nonexistent/plugin.wasm"));
        assert!(result.is_err());
    }
}
