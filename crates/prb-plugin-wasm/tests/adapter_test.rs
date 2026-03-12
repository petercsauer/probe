//! Integration tests for WASM plugin adapters.

use prb_core::{DecodeContext, Timestamp};
use prb_detect::{DecoderFactory, DetectionContext, ProtocolDetector, TransportLayer};
use prb_plugin_api::PluginMetadata;
use prb_plugin_wasm::adapter::{WasmDecoderFactory, WasmProtocolDetector};
use prb_plugin_wasm::loader::WasmPluginLoader;
use prb_plugin_wasm::runtime::WasmLimits;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Get the path to the test WASM plugin fixture.
fn test_plugin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test_plugin.wasm")
}

/// Load the test plugin and return its metadata.
fn load_test_plugin() -> PluginMetadata {
    let mut loader = WasmPluginLoader::new();
    let plugin_path = test_plugin_path();
    loader
        .load(&plugin_path)
        .expect("Failed to load test plugin")
}

#[test]
fn test_decoder_factory_creation() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();
    let limits = WasmLimits::default();

    let factory = WasmDecoderFactory::new(plugin_path, metadata.clone(), limits);

    // Verify protocol_id
    assert_eq!(factory.protocol_id().0.as_str(), "test-protocol");
}

#[test]
fn test_decoder_factory_create_decoder() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();
    let limits = WasmLimits::default();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);

    // Creating a decoder should succeed
    let decoder = factory.create_decoder();

    // Verify it implements ProtocolDecoder
    assert_eq!(decoder.protocol(), prb_core::TransportKind::RawTcp);
}

#[test]
fn test_decoder_decode_stream_minimal() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();
    let limits = WasmLimits::default();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);
    let mut decoder = factory.create_decoder();

    // Create a minimal decode context
    let ctx = DecodeContext {
        src_addr: Some("127.0.0.1:8080".to_string()),
        dst_addr: Some("127.0.0.1:9090".to_string()),
        timestamp: Some(Timestamp::from_nanos(1000000000)),
        metadata: BTreeMap::new(),
    };

    // Decode some test data
    let data = b"Hello, World!";
    let result = decoder.decode_stream(data, &ctx);

    if let Err(e) = &result {
        eprintln!("Decode error: {:?}", e);
    }
    assert!(result.is_ok(), "Decode failed: {:?}", result.err());
    let events = result.unwrap();

    // The test plugin should return one event
    assert_eq!(events.len(), 1);

    let event = &events[0];
    assert_eq!(event.transport, prb_core::TransportKind::RawTcp);
    assert_eq!(event.direction, prb_core::Direction::Inbound);
    assert_eq!(event.source.adapter, "wasm-plugin");
}

#[test]
fn test_decoder_decode_stream_with_metadata() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();
    let limits = WasmLimits::default();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);
    let mut decoder = factory.create_decoder();

    // Create context with metadata
    let mut ctx_metadata = BTreeMap::new();
    ctx_metadata.insert("key1".to_string(), "value1".to_string());
    ctx_metadata.insert("key2".to_string(), "value2".to_string());

    let ctx = DecodeContext {
        src_addr: Some("192.168.1.100:1234".to_string()),
        dst_addr: Some("192.168.1.200:5678".to_string()),
        timestamp: Some(Timestamp::from_nanos(9999999999)),
        metadata: ctx_metadata,
    };

    let data = b"test data";
    let result = decoder.decode_stream(data, &ctx);

    assert!(result.is_ok());
    let events = result.unwrap();
    assert!(!events.is_empty());
}

#[test]
fn test_protocol_detector_creation() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    let detector = WasmProtocolDetector::new(plugin_path, metadata.clone());

    assert_eq!(detector.name(), "test-wasm-plugin");
    assert_eq!(detector.transport(), TransportLayer::Tcp);
}

#[test]
fn test_protocol_detector_detect_success() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    let detector = WasmProtocolDetector::new(plugin_path, metadata);

    // Create detection context
    let initial_bytes = b"test protocol data";
    let ctx = DetectionContext {
        initial_bytes,
        src_port: 8080,
        dst_port: 9090,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = detector.detect(&ctx);

    assert!(result.is_some());
    let detection = result.unwrap();
    assert_eq!(detection.protocol.0.as_str(), "test-protocol");
    assert_eq!(detection.confidence, 0.95);
    assert_eq!(detection.method, prb_detect::DetectionMethod::Heuristic);
}

#[test]
fn test_protocol_detector_detect_no_match() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    let detector = WasmProtocolDetector::new(plugin_path, metadata);

    // Use special test case that causes the plugin to return null
    // The test fixture returns null when src_port is 9999
    let initial_bytes = b"test data";
    let ctx = DetectionContext {
        initial_bytes,
        src_port: 9999,
        dst_port: 5678,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = detector.detect(&ctx);

    assert!(result.is_none());
}

#[test]
fn test_protocol_detector_udp_transport() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    let detector = WasmProtocolDetector::new(plugin_path, metadata);

    // Test with UDP transport
    let initial_bytes = b"udp test data";
    let ctx = DetectionContext {
        initial_bytes,
        src_port: 5060,
        dst_port: 5061,
        transport: TransportLayer::Udp,
        tls_decrypted: false,
    };

    let result = detector.detect(&ctx);

    // Should still work - the test plugin doesn't care about transport
    assert!(result.is_some());
}

#[test]
fn test_decoder_factory_with_custom_limits() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    // Create custom limits
    let limits = WasmLimits {
        memory_max_pages: 128,
        timeout: std::time::Duration::from_secs(10),
    };

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);
    let mut decoder = factory.create_decoder();

    // Verify decoder still works with custom limits
    let ctx = DecodeContext {
        src_addr: None,
        dst_addr: None,
        timestamp: None,
        metadata: BTreeMap::new(),
    };

    let result = decoder.decode_stream(b"test", &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_decoder_factory_for_detection_limits() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    // Use detection-specific limits
    let limits = WasmLimits::for_detection();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);
    let mut decoder = factory.create_decoder();

    let ctx = DecodeContext {
        src_addr: Some("10.0.0.1:80".to_string()),
        dst_addr: Some("10.0.0.2:443".to_string()),
        timestamp: Some(Timestamp::from_nanos(123456)),
        metadata: BTreeMap::new(),
    };

    let result = decoder.decode_stream(b"quick test", &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_decoder_factory_for_decoding_limits() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    // Use decoding-specific limits (default)
    let limits = WasmLimits::for_decoding();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);
    let mut decoder = factory.create_decoder();

    let ctx = DecodeContext {
        src_addr: Some("172.16.0.1:1234".to_string()),
        dst_addr: Some("172.16.0.2:5678".to_string()),
        timestamp: Some(Timestamp::from_nanos(999999999)),
        metadata: BTreeMap::new(),
    };

    let result = decoder.decode_stream(b"longer decoding test", &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_decoder_instances() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();
    let limits = WasmLimits::default();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);

    // Create multiple decoders - each should be independent
    let mut decoder1 = factory.create_decoder();
    let mut decoder2 = factory.create_decoder();
    let mut decoder3 = factory.create_decoder();

    let ctx = DecodeContext {
        src_addr: None,
        dst_addr: None,
        timestamp: None,
        metadata: BTreeMap::new(),
    };

    // All decoders should work independently
    assert!(decoder1.decode_stream(b"test1", &ctx).is_ok());
    assert!(decoder2.decode_stream(b"test2", &ctx).is_ok());
    assert!(decoder3.decode_stream(b"test3", &ctx).is_ok());
}

#[test]
fn test_decoder_with_empty_data() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();
    let limits = WasmLimits::default();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);
    let mut decoder = factory.create_decoder();

    let ctx = DecodeContext {
        src_addr: None,
        dst_addr: None,
        timestamp: None,
        metadata: BTreeMap::new(),
    };

    // Decode empty data
    let result = decoder.decode_stream(b"", &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_detector_with_empty_bytes() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();

    let detector = WasmProtocolDetector::new(plugin_path, metadata);

    let ctx = DetectionContext {
        initial_bytes: b"",
        src_port: 80,
        dst_port: 443,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    // Should still return a result (test plugin doesn't check data content)
    let result = detector.detect(&ctx);
    assert!(result.is_some());
}

#[test]
fn test_decoder_event_has_correct_source() {
    let plugin_path = test_plugin_path();
    let metadata = load_test_plugin();
    let limits = WasmLimits::default();

    let factory = WasmDecoderFactory::new(plugin_path, metadata, limits);
    let mut decoder = factory.create_decoder();

    let ctx = DecodeContext {
        src_addr: Some("1.2.3.4:100".to_string()),
        dst_addr: Some("5.6.7.8:200".to_string()),
        timestamp: Some(Timestamp::from_nanos(111111)),
        metadata: BTreeMap::new(),
    };

    let result = decoder.decode_stream(b"test", &ctx);
    assert!(result.is_ok());

    let events = result.unwrap();
    assert!(!events.is_empty());

    // Verify source information
    let event = &events[0];
    assert_eq!(event.source.adapter, "wasm-plugin");
    assert_eq!(event.source.origin, "127.0.0.1:8080"); // From test plugin fixture
}
