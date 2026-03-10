//! gRPC/HTTP2 protocol decoder for offline PCAP analysis.
//!
//! This crate implements gRPC protocol decoding from reassembled TCP streams,
//! including HTTP/2 frame parsing, HPACK header decompression, gRPC message
//! extraction with compression support, trailer/status parsing, and graceful
//! degradation for mid-stream captures.

mod decoder;
mod error;
mod h2;
mod lpm;

pub use decoder::GrpcDecoder;
pub use error::GrpcError;

#[cfg(test)]
mod tests;
