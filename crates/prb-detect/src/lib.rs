#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::single_match_else)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::trivially_copy_pass_by_ref)]
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
