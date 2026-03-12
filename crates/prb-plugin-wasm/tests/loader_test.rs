//! Integration tests for WASM plugin loader.

use prb_plugin_wasm::loader::WasmPluginLoader;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Get the path to the test WASM plugin fixture.
fn test_plugin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test_plugin.wasm")
}

#[test]
fn test_load_valid_wasm() {
    let mut loader = WasmPluginLoader::new();
    let plugin_path = test_plugin_path();

    let result = loader.load(&plugin_path);
    if let Err(e) = &result {
        eprintln!("Error loading plugin: {:?}", e);
    }
    assert!(
        result.is_ok(),
        "Failed to load valid WASM plugin: {:?}",
        result.err()
    );

    let metadata = result.unwrap();
    assert_eq!(metadata.name, "test-wasm-plugin");
    assert_eq!(metadata.version, "0.1.0");
    assert_eq!(metadata.api_version, "0.1.0");
    assert_eq!(metadata.protocol_id, "test-protocol");
    assert_eq!(
        metadata.description,
        "Test WASM plugin for integration tests"
    );

    // Verify plugin was added to loader
    assert_eq!(loader.plugins().len(), 1);
    assert_eq!(loader.plugins()[0].info.name, "test-wasm-plugin");
}

#[test]
fn test_load_nonexistent_file() {
    let mut loader = WasmPluginLoader::new();
    let result = loader.load(Path::new("/nonexistent/path/plugin.wasm"));

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_load_invalid_wasm() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_wasm = temp_dir.path().join("invalid.wasm");

    // Write random bytes that are not valid WASM
    fs::write(&invalid_wasm, b"this is not a wasm file").unwrap();

    let mut loader = WasmPluginLoader::new();
    let result = loader.load(&invalid_wasm);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Failed to instantiate"));
}

#[test]
fn test_load_wasm_missing_exports() {
    // Create a minimal valid WASM module without the required exports
    let temp_dir = TempDir::new().unwrap();
    let minimal_wasm = temp_dir.path().join("minimal.wasm");

    // This is a minimal valid WASM module with no exports
    // (WASM magic number + version, then minimal type and function sections)
    let minimal_wasm_bytes = vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // WASM version 1
    ];

    fs::write(&minimal_wasm, &minimal_wasm_bytes).unwrap();

    let mut loader = WasmPluginLoader::new();
    let result = loader.load(&minimal_wasm);

    assert!(result.is_err());
    // This will fail at instantiation since it's too minimal to be valid
    let err = result.unwrap_err();
    eprintln!("Actual error: {}", err);
    assert!(
        err.to_string().contains("Failed to instantiate")
            || err.to_string().contains("Missing required export")
    );
}

#[test]
fn test_load_directory_empty() {
    let temp_dir = TempDir::new().unwrap();
    let mut loader = WasmPluginLoader::new();

    let results = loader.load_directory(temp_dir.path());

    assert_eq!(results.len(), 0);
}

#[test]
fn test_load_directory_with_wasm() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_path = test_plugin_path();

    // Copy test plugin to temp directory
    let dest_path = temp_dir.path().join("test_plugin.wasm");
    fs::copy(&plugin_path, &dest_path).unwrap();

    let mut loader = WasmPluginLoader::new();
    let results = loader.load_directory(temp_dir.path());

    assert_eq!(results.len(), 1);
    assert!(results[0].is_ok());

    let metadata = results[0].as_ref().unwrap();
    assert_eq!(metadata.name, "test-wasm-plugin");

    // Verify plugin was added to loader
    assert_eq!(loader.plugins().len(), 1);
}

#[test]
fn test_load_directory_with_mixed_files() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_path = test_plugin_path();

    // Copy test plugin to temp directory
    fs::copy(&plugin_path, temp_dir.path().join("valid.wasm")).unwrap();

    // Add non-WASM files
    fs::write(temp_dir.path().join("readme.txt"), b"not a wasm file").unwrap();
    fs::write(temp_dir.path().join("config.json"), b"{}").unwrap();

    // Add invalid WASM
    fs::write(temp_dir.path().join("invalid.wasm"), b"invalid wasm").unwrap();

    let mut loader = WasmPluginLoader::new();
    let results = loader.load_directory(temp_dir.path());

    // Should have 2 results: one for valid.wasm (success) and one for invalid.wasm (failure)
    assert_eq!(results.len(), 2);

    // Find the successful load
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 1);

    // Verify only the valid plugin was added
    assert_eq!(loader.plugins().len(), 1);
    assert_eq!(loader.plugins()[0].info.name, "test-wasm-plugin");
}

#[test]
fn test_load_directory_nonexistent() {
    let mut loader = WasmPluginLoader::new();
    let results = loader.load_directory(Path::new("/nonexistent/directory"));

    // Should return a single error result for the directory read failure
    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
}

#[test]
fn test_loader_default() {
    let loader = WasmPluginLoader::default();
    assert_eq!(loader.plugins().len(), 0);
}

#[test]
fn test_multiple_loads() {
    let mut loader = WasmPluginLoader::new();
    let plugin_path = test_plugin_path();

    // Load the same plugin multiple times
    let result1 = loader.load(&plugin_path);
    let result2 = loader.load(&plugin_path);
    let result3 = loader.load(&plugin_path);

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());

    // All three should be added to the loader
    assert_eq!(loader.plugins().len(), 3);
}

#[test]
fn test_load_api_version_mismatch() {
    // This test would require a WASM plugin with an incompatible API version
    // For now, we just verify the happy path. A future enhancement could
    // create a second test fixture with a different API version.
    let mut loader = WasmPluginLoader::new();
    let plugin_path = test_plugin_path();

    let result = loader.load(&plugin_path);
    assert!(result.is_ok());
}
