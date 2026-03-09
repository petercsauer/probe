//! PCAP/pcapng file reading and TLS key extraction for PRB.
//!
//! This crate provides transparent reading of both legacy PCAP and modern pcapng
//! capture formats, with support for extracting embedded TLS keys from pcapng
//! Decryption Secrets Blocks (DSB).

mod error;
mod normalize;
mod reader;
pub mod tcp;
pub mod tls;

pub use error::PcapError;
pub use normalize::{NormalizedPacket, PacketNormalizer, TcpFlags, TcpSegmentInfo, TransportInfo};
pub use reader::{PcapFileReader, TlsKeyStore};
pub use tcp::{ReassembledStream, StreamDirection, StreamEvent, TcpReassembler};
pub use tls::{DecryptedStream, TlsStreamProcessor};
