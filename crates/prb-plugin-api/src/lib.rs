#![allow(clippy::cargo_common_metadata)]
//! Plugin API for PRB protocol decoders.
//!
//! This crate defines the stable contract between PRB and its plugins,
//! supporting both native (shared library) and WASM plugins.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod dto;
pub mod native;
pub mod types;

#[cfg(feature = "wasm-pdk")]
pub mod wasm;

pub use dto::{CorrelationKeyDto, DebugEventDto};
pub use types::{DecodeCtx, DetectContext, PluginMetadata, TransportLayer};

/// Plugin API version. Plugins compiled against a different major version
/// will be rejected. Minor version bumps are backward-compatible.
pub const API_VERSION: &str = "0.1.0";

/// Validate that a plugin API version is compatible with this host.
///
/// # Errors
/// Returns an error if the plugin API version is incompatible with the host.
pub fn validate_api_version(plugin_version: &str) -> Result<(), String> {
    let host_ver = semver::Version::parse(API_VERSION)
        .map_err(|e| format!("Invalid host API version: {e}"))?;
    let plugin_ver = semver::Version::parse(plugin_version)
        .map_err(|e| format!("Invalid plugin API version: {e}"))?;

    if host_ver.major != plugin_ver.major {
        return Err(format!(
            "Plugin API major version mismatch: plugin={}, host={}",
            plugin_ver.major, host_ver.major
        ));
    }

    if plugin_ver.minor > host_ver.minor {
        return Err(format!(
            "Plugin requires newer API version: plugin={}.{}, host={}.{}",
            plugin_ver.major, plugin_ver.minor, host_ver.major, host_ver.minor
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_api_version_success() {
        assert!(validate_api_version("0.1.0").is_ok());
        assert!(validate_api_version("0.0.9").is_ok());
    }

    #[test]
    fn test_validate_api_version_major_mismatch() {
        let result = validate_api_version("1.0.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("major version mismatch"));
    }

    #[test]
    fn test_validate_api_version_newer_minor() {
        let result = validate_api_version("0.2.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("newer API version"));
    }

    #[test]
    fn test_validate_api_version_invalid() {
        let result = validate_api_version("not-a-version");
        assert!(result.is_err());
    }
}
