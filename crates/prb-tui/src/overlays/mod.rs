//! Overlay widgets for the TUI.

pub mod capture_config;
pub mod command_palette;
pub mod diff_view;
pub mod export_dialog;
pub mod filter_template;
pub mod follow_stream;
pub mod metrics;
pub mod plugin_manager;
pub mod session_info;
pub mod theme_editor;
pub mod tls_keylog_picker;
pub mod welcome;
pub mod which_key;

pub use capture_config::CaptureConfigOverlay;
pub use command_palette::CommandPaletteOverlay;
pub use diff_view::DiffViewOverlay;
pub use export_dialog::ExportDialogOverlay;
pub use filter_template::FilterTemplateOverlay;
pub use follow_stream::FollowStreamOverlay;
pub use metrics::MetricsOverlay;
pub use plugin_manager::{PluginEntry, PluginManagerOverlay, PluginManagerView, PluginType};
pub use session_info::{ChannelDisplay, SessionInfo, SessionInfoOverlay};
pub use theme_editor::ThemeEditorOverlay;
pub use tls_keylog_picker::TlsKeylogPickerOverlay;
pub use welcome::WelcomeOverlay;
pub use which_key::WhichKeyOverlay;
