//! Protocol detection for PRB.
//!
//! This crate provides the `ProtocolDetector` trait and built-in detectors
//! for gRPC/HTTP2, ZMTP, and DDS/RTPS protocols.

pub mod detector;
pub mod engine;
pub mod types;

pub use detector::{GrpcDetector, GuessCrateDetector, PortMappingDetector, RtpsDetector, ZmtpDetector};
pub use engine::DetectionEngine;
pub use types::{
    DetectionContext, DetectionMethod, DetectionResult, ProtocolDetector, ProtocolId,
    TransportLayer,
};
