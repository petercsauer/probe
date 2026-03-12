pub mod ai_features;
pub mod ai_smart;
pub mod app;
pub mod config;
pub mod demo;
pub mod error_intel;
pub mod event_store;
pub mod filter_state;
pub mod live;
pub mod loader;
pub mod overlays;
pub mod panes;
pub mod ring_buffer;
pub mod session;
pub mod theme;
pub mod trace_extraction;

pub use app::App;
pub use demo::generate_demo_events;
pub use event_store::EventStore;
pub use live::{AppEvent, CaptureState, LiveDataSource};
pub use ring_buffer::RingBuffer;
pub use session::Session;

// Re-export schema types for external use
pub use prb_schema::SchemaRegistry;
pub use prb_decode::{decode_with_schema, decode_wire_format, DecodedMessage, WireMessage};
