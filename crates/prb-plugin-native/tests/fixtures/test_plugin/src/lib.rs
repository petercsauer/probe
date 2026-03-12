//! Test plugin for prb-plugin-native integration tests.

#![allow(unexpected_cfgs)]

use prb_plugin_api::native::PluginDecoder;
use prb_plugin_api::{DebugEventDto, DecodeCtx, DetectContext, PluginMetadata};

/// Simple test plugin that always detects with high confidence.
pub struct TestPlugin;

impl PluginDecoder for TestPlugin {
    fn info() -> PluginMetadata {
        PluginMetadata {
            name: "test-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "A test plugin for integration tests".to_string(),
            protocol_id: "test-protocol".to_string(),
            api_version: prb_plugin_api::API_VERSION.to_string(),
        }
    }

    fn detect(ctx: &DetectContext) -> Option<f32> {
        // Detect if data starts with "TEST"
        if ctx.initial_bytes.len() >= 4 && &ctx.initial_bytes[0..4] == b"TEST" {
            Some(0.95)
        } else {
            None
        }
    }

    fn new() -> Self {
        Self
    }

    fn decode(
        &mut self,
        data: &[u8],
        _ctx: &DecodeCtx,
    ) -> Result<Vec<DebugEventDto>, String> {
        // Simple decode: create one event per decode call
        let event = DebugEventDto::minimal("test-protocol", "request");

        // Store the data length in metadata
        let mut evt = event;
        evt.metadata.insert("data_len".to_string(), data.len().to_string());

        Ok(vec![evt])
    }
}

// Export the plugin
prb_plugin_api::prb_export_plugin!(TestPlugin);
