//! PCAP/pcapng file reading and TLS key extraction for PRB.
//!
//! This crate provides transparent reading of both legacy PCAP and modern pcapng
//! capture formats, with support for extracting embedded TLS keys from pcapng
//! Decryption Secrets Blocks (DSB).

mod error;
pub mod flow_key;
pub mod mmap_reader;
mod normalize;
pub mod parallel;
mod pipeline;
mod pipeline_core;
mod reader;
pub mod tcp;
pub mod tls;

pub use error::PcapError;
pub use flow_key::{FlowKey, FlowProtocol};
pub use mmap_reader::{MmapPcapReader, PacketLocation};
pub use normalize::{NormalizedPacket, OwnedNormalizedPacket, PacketNormalizer, TcpFlags, TcpSegmentInfo, TransportInfo};
pub use parallel::{BatchStage, ParallelPipeline, PipelineConfig, StreamStage};
pub use pipeline::{PcapCaptureAdapter, PipelineStats};
pub use pipeline_core::{PipelineCore, ProcessedEvents};
pub use reader::{PcapFileReader, TlsKeyStore};
pub use tcp::{ReassembledStream, StreamDirection, StreamEvent, TcpReassembler};
pub use tls::{DecryptedStream, TlsStreamProcessor};
