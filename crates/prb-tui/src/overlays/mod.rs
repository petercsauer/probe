//! Overlay widgets for the TUI.

pub mod capture_config;
pub mod command_palette;
pub mod diff_view;
pub mod export_dialog;
pub mod follow_stream;
pub mod metrics;
pub mod plugin_manager;
pub mod session_info;
pub mod welcome;
pub mod which_key;

pub use capture_config::CaptureConfigOverlay;
pub use command_palette::CommandPaletteOverlay;
pub use diff_view::DiffViewOverlay;
pub use export_dialog::ExportDialogOverlay;
pub use follow_stream::FollowStreamOverlay;
pub use metrics::MetricsOverlay;
pub use plugin_manager::{PluginEntry, PluginManagerOverlay, PluginType};
pub use session_info::{ChannelDisplay, SessionInfo, SessionInfoOverlay};
pub use welcome::WelcomeOverlay;
pub use which_key::WhichKeyOverlay;
