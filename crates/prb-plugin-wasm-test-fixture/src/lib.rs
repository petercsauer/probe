//! Test WASM plugin fixture for prb-plugin-wasm integration tests.
//!
//! This plugin implements the minimal required exports for testing
//! the WASM plugin loader and adapter.

use extism_pdk::*;

/// Returns plugin metadata as JSON.
#[plugin_fn]
pub fn prb_plugin_info(_input: String) -> FnResult<String> {
    let metadata = r#"{
        "name": "test-wasm-plugin",
        "version": "0.1.0",
        "api_version": "0.1.0",
        "protocol_id": "test-protocol",
        "description": "Test WASM plugin for integration tests",
        "transport": "Tcp"
    }"#;
    Ok(metadata.to_string())
}

/// Protocol detection function - returns high confidence for testing.
/// Returns null if src_port is 9999 (special test case for no-match).
#[plugin_fn]
pub fn prb_plugin_detect(input: String) -> FnResult<String> {
    // Check for special test case: src_port 9999 means no detection
    if input.contains("\"src_port\":9999") || input.contains("\"src_port\": 9999") {
        Ok("null".to_string())
    } else {
        // Return high confidence for most cases
        Ok("0.95".to_string())
    }
}

/// Decode function - returns minimal debug events for testing.
#[plugin_fn]
pub fn prb_plugin_decode(input: String) -> FnResult<String> {
    // Parse the input to generate appropriate test responses
    if input.contains("empty") {
        Ok("[]".to_string())
    } else {
        // Return a minimal debug event
        let events = r#"[{
            "timestamp_nanos": 1234567890,
            "transport": "tcp",
            "direction": "inbound",
            "src_addr": "127.0.0.1:8080",
            "dst_addr": "127.0.0.1:9090",
            "payload_raw": [72, 101, 108, 108, 111],
            "payload_decoded": null,
            "schema_name": null,
            "metadata": {},
            "correlation_keys": [],
            "warnings": []
        }]"#;
        Ok(events.to_string())
    }
}
