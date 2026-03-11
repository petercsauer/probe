//! Overlay widgets for the TUI.

pub mod welcome;
pub mod which_key;
pub mod command_palette;
pub mod plugin_manager;

pub use welcome::WelcomeOverlay;
pub use which_key::WhichKeyOverlay;
pub use command_palette::CommandPaletteOverlay;
pub use plugin_manager::{PluginManagerOverlay, PluginEntry, PluginType};
