//! Invalid plugin with incompatible API version for testing error handling.

#![allow(unexpected_cfgs)]

use prb_plugin_api::native::PluginDecoder;
use prb_plugin_api::{DebugEventDto, DecodeCtx, DetectContext, PluginMetadata};

/// Invalid plugin with incompatible API version.
pub struct InvalidPlugin;

impl PluginDecoder for InvalidPlugin {
    fn info() -> PluginMetadata {
        PluginMetadata {
            name: "invalid-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "An invalid plugin with wrong API version".to_string(),
            protocol_id: "invalid".to_string(),
            // Incompatible major version
            api_version: "99.0.0".to_string(),
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
        Ok(vec![])
    }
}

// Export the plugin
prb_plugin_api::prb_export_plugin!(InvalidPlugin);
