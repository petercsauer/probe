//! Integration tests for native plugin loader.

use prb_plugin_native::{NativePluginLoader, PluginError};
use std::path::PathBuf;
use std::process::Command;

/// Get the path to the built test plugin shared library.
fn build_test_plugin(name: &str) -> PathBuf {
    // Build the test plugin
    let manifest_path = format!("tests/fixtures/{}/Cargo.toml", name);
    let output = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            &manifest_path,
            "--target-dir",
            "tests/fixtures/target",
        ])
        .output()
        .expect("Failed to build test plugin");

    if !output.status.success() {
        panic!(
            "Failed to build test plugin:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Determine the library extension based on platform
    let lib_name = if cfg!(target_os = "macos") {
        format!("lib{}.dylib", name.replace('-', "_"))
    } else if cfg!(target_os = "windows") {
        format!("{}.dll", name.replace('-', "_"))
    } else {
        format!("lib{}.so", name.replace('-', "_"))
    };

    PathBuf::from(format!("tests/fixtures/target/debug/{}", lib_name))
}

#[test]
fn test_load_valid_plugin() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let result = loader.load(&plugin_path);
    assert!(result.is_ok(), "Failed to load valid plugin: {:?}", result);

    let plugin = result.unwrap();
    let metadata = plugin.metadata();

    assert_eq!(metadata.name, "test-plugin");
    assert_eq!(metadata.version, "0.1.0");
    assert_eq!(metadata.protocol_id, "test-protocol");
    assert_eq!(metadata.api_version, prb_plugin_api::API_VERSION);
}

#[test]
fn test_load_nonexistent_plugin() {
    let mut loader = NativePluginLoader::new();
    let result = loader.load(std::path::Path::new("/nonexistent/plugin.so"));

    assert!(result.is_err());
    match result {
        Err(PluginError::LibraryLoad(_)) => {}
        other => panic!("Expected LibraryLoad error, got: {:?}", other),
    }
}

#[test]
fn test_load_incompatible_api_version() {
    let plugin_path = build_test_plugin("invalid_plugin");
    let mut loader = NativePluginLoader::new();

    let result = loader.load(&plugin_path);
    assert!(result.is_err());

    match result {
        Err(PluginError::IncompatibleVersion(msg)) => {
            assert!(msg.contains("major version mismatch"));
        }
        other => panic!("Expected IncompatibleVersion error, got: {:?}", other),
    }
}

#[test]
fn test_plugin_detect_function() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    // Test data that should be detected (starts with "TEST")
    let test_data = b"TEST data here";
    let result = plugin.detect(test_data, 8080, 9090, 0);

    assert_eq!(result.detected, 1);
    assert!(result.confidence > 0.9);

    // Test data that should NOT be detected
    let other_data = b"OTHER data here";
    let result = plugin.detect(other_data, 8080, 9090, 0);

    assert_eq!(result.detected, 0);
}

#[test]
fn test_plugin_create_and_destroy_decoder() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    // Create decoder handle
    let handle = plugin.create_decoder();
    assert!(!handle.is_null());

    // Destroy decoder handle
    plugin.destroy_decoder(handle);
}

#[test]
fn test_plugin_decode_function() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    // Create decoder
    let handle = plugin.create_decoder();
    assert!(!handle.is_null());

    // Prepare decode context
    let ctx_json = br#"{"src_addr":"127.0.0.1:8080","dst_addr":"127.0.0.1:9090","timestamp_nanos":null,"metadata":{}}"#;

    // Decode some data
    let data = b"test payload";
    let result_buf = plugin.decode(handle, data, ctx_json);

    // Check that we got a result
    assert!(!result_buf.ptr.is_null());
    assert!(result_buf.len > 0);

    // Convert to Vec and parse JSON
    let result_json = unsafe { result_buf.into_vec() };
    let events: Vec<serde_json::Value> =
        serde_json::from_slice(&result_json).expect("Failed to parse result JSON");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["transport"], "test-protocol");
    assert_eq!(events[0]["direction"], "request");

    // Cleanup
    plugin.destroy_decoder(handle);
}

#[test]
fn test_plugin_loader_keeps_library_alive() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    // Get a reference to the library
    let lib = plugin.library();

    // Library should be valid
    assert!(std::sync::Arc::strong_count(&lib) >= 2); // plugin + our reference
}

#[test]
fn test_load_multiple_plugins() {
    let test_plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    // Load the same plugin twice
    let plugin1 = loader
        .load(&test_plugin_path)
        .expect("Failed to load plugin 1");
    let plugin2 = loader
        .load(&test_plugin_path)
        .expect("Failed to load plugin 2");

    // Both should be valid and separate instances
    assert_eq!(plugin1.metadata().name, "test-plugin");
    assert_eq!(plugin2.metadata().name, "test-plugin");

    // Loader should track both
    assert_eq!(loader.plugins().len(), 2);
}

#[test]
fn test_load_directory() {
    // Build both plugins
    build_test_plugin("test_plugin");
    build_test_plugin("invalid_plugin");

    let mut loader = NativePluginLoader::new();
    let plugin_dir = PathBuf::from("tests/fixtures/target/debug");

    let results = loader.load_directory(&plugin_dir);

    // Should find at least our test plugins
    assert!(!results.is_empty());

    // At least one should succeed (test_plugin)
    let successes: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
    assert!(
        !successes.is_empty(),
        "Expected at least one successful plugin load"
    );

    // At least one should fail (invalid_plugin)
    let failures: Vec<_> = results.iter().filter(|r| r.is_err()).collect();
    assert!(
        !failures.is_empty(),
        "Expected at least one failed plugin load"
    );
}

#[test]
fn test_load_directory_nonexistent() {
    let mut loader = NativePluginLoader::new();
    let results = loader.load_directory(std::path::Path::new("/nonexistent/dir"));

    // Should return one error result
    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
}

#[test]
fn test_plugin_metadata_strings_are_valid_utf8() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");
    let metadata = plugin.metadata();

    // All strings should be valid UTF-8 (no panics)
    assert!(!metadata.name.is_empty());
    assert!(!metadata.version.is_empty());
    assert!(!metadata.description.is_empty());
    assert!(!metadata.protocol_id.is_empty());
    assert!(!metadata.api_version.is_empty());
}

#[test]
fn test_plugin_detect_with_different_transports() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let test_data = b"TEST data";

    // Test TCP (0)
    let result_tcp = plugin.detect(test_data, 8080, 9090, 0);
    assert_eq!(result_tcp.detected, 1);

    // Test UDP (1)
    let result_udp = plugin.detect(test_data, 8080, 9090, 1);
    assert_eq!(result_udp.detected, 1);
}

#[test]
fn test_plugin_decode_empty_data() {
    let plugin_path = build_test_plugin("test_plugin");
    let mut loader = NativePluginLoader::new();

    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");
    let handle = plugin.create_decoder();

    let ctx_json = br#"{"src_addr":"127.0.0.1:8080","dst_addr":"127.0.0.1:9090","timestamp_nanos":null,"metadata":{}}"#;

    // Decode empty data
    let result_buf = plugin.decode(handle, b"", ctx_json);

    // Should still return valid result
    assert!(!result_buf.ptr.is_null() || result_buf.len == 0);

    plugin.destroy_decoder(handle);
}

#[test]
fn test_default_loader() {
    let loader = NativePluginLoader::default();
    assert_eq!(loader.plugins().len(), 0);
}
