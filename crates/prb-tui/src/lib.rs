//! Terminal UI for the PRB universal message debugger.
//!
//! This crate provides an interactive terminal interface for analyzing debug events,
//! with support for filtering, AI-powered explanations, and live capture.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![allow(missing_docs)] // TODO: Complete TUI documentation in future segment

pub mod ai_features;
pub mod ai_smart;
pub mod app;
pub mod autocomplete;
pub mod config;
pub mod demo;
pub mod error_intel;
pub mod event_store;
pub mod filter_state;
pub mod live;
pub mod loader;
pub mod overlays;
pub mod panes;
pub mod query_planner;
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
pub use prb_decode::{DecodedMessage, WireMessage, decode_wire_format, decode_with_schema};
pub use prb_schema::SchemaRegistry;
