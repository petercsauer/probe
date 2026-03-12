//! Tests for native plugin FFI types.

use prb_plugin_api::native::*;

#[test]
fn test_byte_buffer_from_slice() {
    let data = vec![1, 2, 3, 4, 5];
    let buffer = ByteBuffer::from_slice(&data);

    assert_eq!(buffer.len, 5);
    assert!(!buffer.ptr.is_null());

    let slice = unsafe { buffer.as_slice() };
    assert_eq!(slice, &[1, 2, 3, 4, 5]);
}

#[test]
fn test_byte_buffer_empty() {
    let data: &[u8] = &[];
    let buffer = ByteBuffer::from_slice(data);

    assert_eq!(buffer.len, 0);

    let slice = unsafe { buffer.as_slice() };
    assert_eq!(slice, &[] as &[u8]);
}

#[test]
fn test_byte_buffer_null_handling() {
    let buffer = ByteBuffer {
        ptr: std::ptr::null(),
        len: 10,
    };

    let slice = unsafe { buffer.as_slice() };
    assert_eq!(slice, &[] as &[u8]);
}

#[test]
fn test_owned_buffer_from_vec() {
    let data = vec![10, 20, 30, 40];
    let capacity = data.capacity();
    let buffer = OwnedBuffer::from_vec(data);

    assert_eq!(buffer.len, 4);
    assert_eq!(buffer.capacity, capacity);
    assert!(!buffer.ptr.is_null());

    // Convert back to Vec
    let recovered = unsafe { buffer.into_vec() };
    assert_eq!(recovered, vec![10, 20, 30, 40]);
}

#[test]
fn test_owned_buffer_empty() {
    let buffer = OwnedBuffer::empty();

    assert_eq!(buffer.len, 0);
    assert_eq!(buffer.capacity, 0);
    assert!(buffer.ptr.is_null());
}

#[test]
fn test_owned_buffer_roundtrip() {
    let original = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let buffer = OwnedBuffer::from_vec(original.clone());
    let recovered = unsafe { buffer.into_vec() };

    assert_eq!(recovered, original);
}

#[allow(clippy::float_cmp)]
#[test]
fn test_detect_result_ffi() {
    let result = DetectResultFfi {
        detected: 1,
        confidence: 0.95,
    };

    assert_eq!(result.detected, 1);
    assert_eq!(result.confidence, 0.95);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_detect_result_ffi_not_detected() {
    let result = DetectResultFfi {
        detected: 0,
        confidence: 0.0,
    };

    assert_eq!(result.detected, 0);
    assert_eq!(result.confidence, 0.0);
}

// Test the macro by creating a minimal test decoder
mod test_decoder {
    use super::*;
    use prb_plugin_api::{DebugEventDto, DecodeCtx, DetectContext, PluginMetadata};

    struct TestDecoder;

    impl PluginDecoder for TestDecoder {
        fn info() -> PluginMetadata {
            PluginMetadata {
                name: "test-decoder".to_string(),
                version: "1.0.0".to_string(),
                description: "Test decoder".to_string(),
                protocol_id: "test".to_string(),
                api_version: prb_plugin_api::API_VERSION.to_string(),
            }
        }

        fn detect(ctx: &DetectContext) -> Option<f32> {
            // Simple heuristic: detect if first byte is 0xFF
            if !ctx.initial_bytes.is_empty() && ctx.initial_bytes[0] == 0xFF {
                Some(0.9)
            } else {
                None
            }
        }

        fn new() -> Self {
            Self
        }

        fn decode(&mut self, _data: &[u8], _ctx: &DecodeCtx) -> Result<Vec<DebugEventDto>, String> {
            Ok(vec![DebugEventDto::minimal("test", "request")])
        }
    }

    #[test]
    fn test_plugin_decoder_trait() {
        let info = TestDecoder::info();
        assert_eq!(info.name, "test-decoder");
        assert_eq!(info.protocol_id, "test");

        let ctx = DetectContext {
            initial_bytes: vec![0xFF, 0x01, 0x02],
            src_port: 8080,
            dst_port: 9090,
            transport: prb_plugin_api::TransportLayer::Tcp,
        };

        let confidence = TestDecoder::detect(&ctx);
        assert_eq!(confidence, Some(0.9));

        let mut decoder = TestDecoder::new();
        let result = decoder.decode(
            &[1, 2, 3],
            &DecodeCtx {
                src_addr: None,
                dst_addr: None,
                timestamp_nanos: None,
                metadata: std::collections::HashMap::new(),
            },
        );

        assert!(result.is_ok());
        let events = result.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_plugin_decoder_detect_no_match() {
        let ctx = DetectContext {
            initial_bytes: vec![0x00, 0x01, 0x02],
            src_port: 8080,
            dst_port: 9090,
            transport: prb_plugin_api::TransportLayer::Tcp,
        };

        let confidence = TestDecoder::detect(&ctx);
        assert_eq!(confidence, None);
    }

    #[test]
    fn test_plugin_decoder_decode_error() {
        struct ErrorDecoder;

        impl PluginDecoder for ErrorDecoder {
            fn info() -> PluginMetadata {
                PluginMetadata {
                    name: "error-decoder".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Error decoder".to_string(),
                    protocol_id: "error".to_string(),
                    api_version: prb_plugin_api::API_VERSION.to_string(),
                }
            }

            fn detect(_ctx: &DetectContext) -> Option<f32> {
                None
            }

            fn new() -> Self {
                Self
            }

            fn decode(
                &mut self,
                _data: &[u8],
                _ctx: &DecodeCtx,
            ) -> Result<Vec<DebugEventDto>, String> {
                Err("Decode error".to_string())
            }
        }

        let mut decoder = ErrorDecoder::new();
        let result = decoder.decode(
            &[1, 2, 3],
            &DecodeCtx {
                src_addr: None,
                dst_addr: None,
                timestamp_nanos: None,
                metadata: std::collections::HashMap::new(),
            },
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Decode error");
    }

    #[test]
    fn test_plugin_decoder_detect_empty_data() {
        let ctx = DetectContext {
            initial_bytes: vec![],
            src_port: 8080,
            dst_port: 9090,
            transport: prb_plugin_api::TransportLayer::Tcp,
        };

        let confidence = TestDecoder::detect(&ctx);
        assert_eq!(confidence, None);
    }

    #[test]
    fn test_plugin_decoder_with_udp_transport() {
        let ctx = DetectContext {
            initial_bytes: vec![0xFF, 0x01, 0x02],
            src_port: 5353,
            dst_port: 5353,
            transport: prb_plugin_api::TransportLayer::Udp,
        };

        let confidence = TestDecoder::detect(&ctx);
        assert_eq!(confidence, Some(0.9));
    }
}

#[test]
fn test_owned_buffer_large_capacity() {
    let mut data = Vec::with_capacity(1000);
    data.extend_from_slice(&[42; 100]);

    let capacity = data.capacity();
    let buffer = OwnedBuffer::from_vec(data);

    assert_eq!(buffer.len, 100);
    assert_eq!(buffer.capacity, capacity);
    assert!(buffer.capacity >= 1000);

    let recovered = unsafe { buffer.into_vec() };
    assert_eq!(recovered.len(), 100);
    assert_eq!(recovered[0], 42);
}

#[test]
fn test_byte_buffer_large_data() {
    let data = vec![255u8; 10000];
    let buffer = ByteBuffer::from_slice(&data);

    assert_eq!(buffer.len, 10000);

    let slice = unsafe { buffer.as_slice() };
    assert_eq!(slice.len(), 10000);
    assert_eq!(slice[0], 255);
    assert_eq!(slice[9999], 255);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_detect_result_ffi_clamping() {
    // Test that confidence values outside 0.0-1.0 can be represented
    let result = DetectResultFfi {
        detected: 1,
        confidence: 1.5, // Above 1.0
    };

    assert_eq!(result.confidence, 1.5);

    let result2 = DetectResultFfi {
        detected: 1,
        confidence: -0.5, // Below 0.0
    };

    assert_eq!(result2.confidence, -0.5);
}

#[test]
fn test_owned_buffer_from_empty_vec() {
    let data = Vec::<u8>::new();
    let buffer = OwnedBuffer::from_vec(data);

    assert_eq!(buffer.len, 0);
    assert!(!buffer.ptr.is_null() || buffer.capacity == 0);

    let recovered = unsafe { buffer.into_vec() };
    assert_eq!(recovered.len(), 0);
}

#[test]
fn test_byte_buffer_single_byte() {
    let data = vec![42u8];
    let buffer = ByteBuffer::from_slice(&data);

    assert_eq!(buffer.len, 1);

    let slice = unsafe { buffer.as_slice() };
    assert_eq!(slice.len(), 1);
    assert_eq!(slice[0], 42);
}
