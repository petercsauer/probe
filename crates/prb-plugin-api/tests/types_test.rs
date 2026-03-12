//! Tests for plugin API types.

use prb_plugin_api::types::*;
use std::collections::HashMap;

#[test]
fn test_plugin_metadata_serde() {
    let metadata = PluginMetadata {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        description: "Test plugin".to_string(),
        protocol_id: "test-proto".to_string(),
        api_version: "0.1.0".to_string(),
    };

    let json = serde_json::to_string(&metadata).expect("serialize");
    let deserialized: PluginMetadata = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.name, "test-plugin");
    assert_eq!(deserialized.version, "1.0.0");
    assert_eq!(deserialized.protocol_id, "test-proto");
}

#[test]
fn test_transport_layer_serde() {
    let tcp = TransportLayer::Tcp;
    let json = serde_json::to_string(&tcp).expect("serialize");
    assert_eq!(json, r#""tcp""#);

    let deserialized: TransportLayer = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, TransportLayer::Tcp);

    let udp = TransportLayer::Udp;
    let json = serde_json::to_string(&udp).expect("serialize");
    assert_eq!(json, r#""udp""#);
}

#[test]
fn test_decode_ctx_serde() {
    let mut metadata = HashMap::new();
    metadata.insert("key1".to_string(), "value1".to_string());

    let ctx = DecodeCtx {
        src_addr: Some("192.168.1.1:8080".to_string()),
        dst_addr: Some("192.168.1.2:9090".to_string()),
        timestamp_nanos: Some(1_234_567_890),
        metadata,
    };

    let json = serde_json::to_string(&ctx).expect("serialize");
    let deserialized: DecodeCtx = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.src_addr, Some("192.168.1.1:8080".to_string()));
    assert_eq!(deserialized.dst_addr, Some("192.168.1.2:9090".to_string()));
    assert_eq!(deserialized.timestamp_nanos, Some(1_234_567_890));
    assert_eq!(deserialized.metadata.get("key1").unwrap(), "value1");
}

#[test]
fn test_decode_ctx_empty() {
    let ctx = DecodeCtx {
        src_addr: None,
        dst_addr: None,
        timestamp_nanos: None,
        metadata: HashMap::new(),
    };

    let json = serde_json::to_string(&ctx).expect("serialize");
    let deserialized: DecodeCtx = serde_json::from_str(&json).expect("deserialize");

    assert!(deserialized.src_addr.is_none());
    assert!(deserialized.dst_addr.is_none());
    assert!(deserialized.timestamp_nanos.is_none());
    assert!(deserialized.metadata.is_empty());
}

#[cfg(not(feature = "wasm-pdk"))]
#[test]
fn test_detect_context_native() {
    let ctx = DetectContext {
        initial_bytes: vec![1, 2, 3, 4],
        src_port: 8080,
        dst_port: 9090,
        transport: TransportLayer::Tcp,
    };

    assert_eq!(ctx.initial_bytes, vec![1, 2, 3, 4]);
    assert_eq!(ctx.src_port, 8080);
    assert_eq!(ctx.dst_port, 9090);
    assert_eq!(ctx.transport, TransportLayer::Tcp);
}

#[test]
fn test_wasm_decode_request_serde() {
    let mut metadata = HashMap::new();
    metadata.insert("test".to_string(), "value".to_string());

    let request = WasmDecodeRequest {
        data_b64: "AQIDBA==".to_string(), // base64 for [1,2,3,4]
        ctx: DecodeCtx {
            src_addr: Some("127.0.0.1:8080".to_string()),
            dst_addr: None,
            timestamp_nanos: Some(1000),
            metadata,
        },
    };

    let json = serde_json::to_string(&request).expect("serialize");
    let deserialized: WasmDecodeRequest = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.data_b64, "AQIDBA==");
    assert_eq!(
        deserialized.ctx.src_addr,
        Some("127.0.0.1:8080".to_string())
    );
    assert_eq!(deserialized.ctx.timestamp_nanos, Some(1000));
}
