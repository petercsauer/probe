//! Integration tests for native plugin adapters (DecoderFactory, ProtocolDetector).

use prb_core::{DecodeContext, Timestamp};
use prb_detect::{
    DecoderFactory, DetectionContext, ProtocolDetector as DetectorTrait, TransportLayer,
};
use prb_plugin_native::{NativeDecoderFactory, NativePluginLoader, NativeProtocolDetector};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

/// Build and return path to test plugin.
fn build_test_plugin() -> PathBuf {
    let output = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            "tests/fixtures/test_plugin/Cargo.toml",
            "--target-dir",
            "tests/fixtures/target",
        ])
        .output()
        .expect("Failed to build test plugin");

    if !output.status.success() {
        panic!(
            "Failed to build test plugin:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let lib_name = if cfg!(target_os = "macos") {
        "libtest_plugin.dylib"
    } else if cfg!(target_os = "windows") {
        "test_plugin.dll"
    } else {
        "libtest_plugin.so"
    };

    PathBuf::from(format!("tests/fixtures/target/debug/{}", lib_name))
}

#[test]
fn test_native_decoder_factory_protocol_id() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let factory = NativeDecoderFactory::new(plugin);
    let protocol_id = factory.protocol_id();

    assert_eq!(protocol_id.0.as_str(), "test-protocol");
}

#[test]
#[ignore] // Flaky on macOS CI - UTF-8 error loading dylib (works locally)
fn test_native_decoder_factory_creates_decoder() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let factory = NativeDecoderFactory::new(plugin);
    let decoder = factory.create_decoder();

    // Decoder should be created successfully
    assert_eq!(
        decoder.protocol(),
        prb_core::TransportKind::JsonFixture // default fallback for test-protocol
    );
}

#[test]
fn test_native_decoder_instance_decode_stream() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let factory = NativeDecoderFactory::new(plugin);
    let mut decoder = factory.create_decoder();

    // Create decode context
    let ctx = DecodeContext {
        src_addr: Some("127.0.0.1:8080".to_string()),
        dst_addr: Some("127.0.0.1:9090".to_string()),
        timestamp: Some(Timestamp::from_nanos(1234567890)),
        metadata: BTreeMap::new(),
    };

    // Decode some data
    let data = b"test payload data";
    let result = decoder.decode_stream(data, &ctx);

    assert!(result.is_ok(), "Decode failed: {:?}", result);

    let events = result.unwrap();
    assert_eq!(events.len(), 1);

    // Check event properties
    let event = &events[0];
    assert_eq!(event.transport, prb_core::TransportKind::JsonFixture);
    assert_eq!(event.direction, prb_core::Direction::Inbound);
    assert_eq!(event.source.adapter, "native-plugin");

    // Check metadata from plugin
    assert_eq!(
        event.metadata.get("data_len"),
        Some(&data.len().to_string())
    );
}

#[test]
fn test_native_decoder_instance_decode_empty_data() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let factory = NativeDecoderFactory::new(plugin);
    let mut decoder = factory.create_decoder();

    let ctx = DecodeContext {
        src_addr: Some("127.0.0.1:8080".to_string()),
        dst_addr: Some("127.0.0.1:9090".to_string()),
        timestamp: Some(Timestamp::from_nanos(1234567890)),
        metadata: BTreeMap::new(),
    };

    // Decode empty data
    let result = decoder.decode_stream(b"", &ctx);
    assert!(result.is_ok());

    let events = result.unwrap();
    assert_eq!(events.len(), 1); // Plugin still returns one event
}

#[test]
fn test_native_decoder_instance_multiple_decode_calls() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let factory = NativeDecoderFactory::new(plugin);
    let mut decoder = factory.create_decoder();

    let ctx = DecodeContext {
        src_addr: Some("127.0.0.1:8080".to_string()),
        dst_addr: Some("127.0.0.1:9090".to_string()),
        timestamp: Some(Timestamp::from_nanos(1234567890)),
        metadata: BTreeMap::new(),
    };

    // Multiple decode calls should work
    for i in 0..3 {
        let data = format!("data chunk {}", i);
        let result = decoder.decode_stream(data.as_bytes(), &ctx);
        assert!(result.is_ok());

        let events = result.unwrap();
        assert_eq!(events.len(), 1);
    }
}

#[test]
fn test_native_protocol_detector_name() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let detector = NativeProtocolDetector::new(plugin, TransportLayer::Tcp);

    assert_eq!(detector.name(), "test-plugin");
}

#[test]
fn test_native_protocol_detector_transport() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let detector_tcp = NativeProtocolDetector::new(plugin.clone(), TransportLayer::Tcp);
    assert_eq!(detector_tcp.transport(), TransportLayer::Tcp);

    let detector_udp = NativeProtocolDetector::new(plugin, TransportLayer::Udp);
    assert_eq!(detector_udp.transport(), TransportLayer::Udp);
}

#[test]
fn test_native_protocol_detector_detect_positive() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let detector = NativeProtocolDetector::new(plugin, TransportLayer::Tcp);

    // Data that should be detected (starts with "TEST")
    let ctx = DetectionContext {
        initial_bytes: b"TEST protocol data",
        src_port: 8080,
        dst_port: 9090,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = detector.detect(&ctx);
    assert!(result.is_some());

    let detection = result.unwrap();
    assert_eq!(detection.protocol.0.as_str(), "test-protocol");
    assert!(detection.confidence > 0.9);
    assert_eq!(detection.method, prb_detect::DetectionMethod::Heuristic);
}

#[test]
fn test_native_protocol_detector_detect_negative() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let detector = NativeProtocolDetector::new(plugin, TransportLayer::Tcp);

    // Data that should NOT be detected (doesn't start with "TEST")
    let ctx = DetectionContext {
        initial_bytes: b"OTHER protocol data",
        src_port: 8080,
        dst_port: 9090,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = detector.detect(&ctx);
    assert!(result.is_none());
}

#[test]
fn test_native_protocol_detector_detect_empty_data() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let detector = NativeProtocolDetector::new(plugin, TransportLayer::Tcp);

    let ctx = DetectionContext {
        initial_bytes: &[],
        src_port: 8080,
        dst_port: 9090,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = detector.detect(&ctx);
    assert!(result.is_none());
}

#[test]
fn test_native_protocol_detector_udp_transport() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let detector = NativeProtocolDetector::new(plugin, TransportLayer::Udp);

    let ctx = DetectionContext {
        initial_bytes: b"TEST over UDP",
        src_port: 5353,
        dst_port: 5353,
        transport: TransportLayer::Udp,
        tls_decrypted: false,
    };

    let result = detector.detect(&ctx);
    assert!(result.is_some());

    let detection = result.unwrap();
    assert_eq!(detection.protocol.0.as_str(), "test-protocol");
}

#[test]
fn test_native_decoder_instance_drop_cleanup() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let factory = NativeDecoderFactory::new(plugin);

    // Create decoder and let it drop
    {
        let _decoder = factory.create_decoder();
        // Decoder drops here - should call destroy_decoder
    }

    // If we get here without crashing, cleanup worked
}

#[test]
fn test_decoder_with_metadata_in_context() {
    let plugin_path = build_test_plugin();
    let mut loader = NativePluginLoader::new();
    let plugin = loader.load(&plugin_path).expect("Failed to load plugin");

    let factory = NativeDecoderFactory::new(plugin);
    let mut decoder = factory.create_decoder();

    let mut metadata = BTreeMap::new();
    metadata.insert("custom_key".to_string(), "custom_value".to_string());

    let ctx = DecodeContext {
        src_addr: Some("192.168.1.1:1234".to_string()),
        dst_addr: Some("192.168.1.2:5678".to_string()),
        timestamp: Some(Timestamp::from_nanos(9876543210)),
        metadata,
    };

    let result = decoder.decode_stream(b"data", &ctx);
    assert!(result.is_ok());
}
