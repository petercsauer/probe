//! Protocol detection for PRB.
//!
//! This crate provides the `ProtocolDetector` trait and built-in detectors
//! for gRPC/HTTP2, ZMTP, and DDS/RTPS protocols, plus a central registry
//! for coordinating detection and decoding.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod detector;
pub mod engine;
pub mod registry;
pub mod types;

pub use detector::{
    GrpcDetector, GuessCrateDetector, PortMappingDetector, RtpsDetector, ZmtpDetector,
};
pub use engine::DetectionEngine;
pub use registry::{DecoderFactory, DecoderRegistry, StreamKey};
pub use types::{
    DetectionContext, DetectionMethod, DetectionResult, ProtocolDetector, ProtocolId,
    TransportLayer,
};
