//! Overlay widgets for the TUI.

pub mod capture_config;
pub mod command_palette;
pub mod export_dialog;
// WIP: pub mod follow_stream;
// WIP: pub mod metrics;
pub mod plugin_manager;
pub mod welcome;
pub mod which_key;

pub use capture_config::CaptureConfigOverlay;
pub use command_palette::CommandPaletteOverlay;
pub use export_dialog::ExportDialogOverlay;
// WIP: pub use follow_stream::FollowStreamOverlay;
// WIP: pub use metrics::MetricsOverlay;
pub use plugin_manager::{PluginEntry, PluginManagerOverlay, PluginType};
pub use welcome::WelcomeOverlay;
pub use which_key::WhichKeyOverlay;
