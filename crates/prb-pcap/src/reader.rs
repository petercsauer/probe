//! PCAP/pcapng file reader with TLS key extraction.

use crate::error::PcapError;
use pcap_parser::traits::PcapReaderIterator;
use pcap_parser::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Linktype identifier from PCAP/pcapng headers.
pub type Linktype = u32;

/// TLS key material extracted from pcapng Decryption Secrets Blocks.
///
/// Maps client_random (48 bytes) to key material for TLS 1.2/1.3 decryption.
#[derive(Debug, Default)]
pub struct TlsKeyStore {
    keys: HashMap<Vec<u8>, Vec<u8>>,
}

impl TlsKeyStore {
    /// Creates an empty key store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a TLS key entry.
    pub fn insert(&mut self, client_random: Vec<u8>, key_material: Vec<u8>) {
        self.keys.insert(client_random, key_material);
    }

    /// Looks up key material by client_random.
    pub fn get(&self, client_random: &[u8]) -> Option<&[u8]> {
        self.keys.get(client_random).map(|v| v.as_slice())
    }

    /// Returns the number of stored keys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns true if no keys are stored.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Returns an iterator over all key entries.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.keys.iter().map(|(k, v)| (k.as_slice(), v.as_slice()))
    }
}

/// A packet extracted from a PCAP/pcapng file.
#[derive(Debug)]
pub struct PcapPacket {
    /// Linktype for this packet (determines layer 2 protocol).
    pub linktype: Linktype,
    /// Packet timestamp in microseconds since UNIX epoch.
    pub timestamp_us: u64,
    /// Raw packet data (includes layer 2 header).
    pub data: Vec<u8>,
}

/// Reader for PCAP and pcapng files with automatic format detection.
pub struct PcapFileReader {
    reader: Box<dyn PcapReaderIterator>,
    tls_keys: TlsKeyStore,
    interfaces: HashMap<u32, Linktype>,
    default_linktype: Linktype,
}

impl PcapFileReader {
    /// Opens a PCAP or pcapng file with automatic format detection.
    ///
    /// Format is detected via magic bytes:
    /// - pcap: 0xa1b2c3d4 or 0xd4c3b2a1 (native/swapped endian)
    /// - pcapng: 0x0a0d0d0a
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, PcapError> {
        use std::io::Seek;

        let path = path.as_ref();
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read magic bytes to detect format
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        // Reset to start
        reader.seek(std::io::SeekFrom::Start(0))?;

        match u32::from_le_bytes(magic) {
            0x0a0d0d0a => {
                // pcapng format
                // Use 1MB buffer for streaming large captures
                let pcapng_reader = PcapNGReader::new(1024 * 1024, reader).map_err(|e| {
                    PcapError::Parse(format!("failed to create pcapng reader: {:?}", e))
                })?;
                Ok(Self {
                    reader: Box::new(pcapng_reader),
                    tls_keys: TlsKeyStore::new(),
                    interfaces: HashMap::new(),
                    default_linktype: 1, // Default to Ethernet
                })
            }
            0xa1b2c3d4 | 0xd4c3b2a1 => {
                // pcap format (native or swapped endian)
                // Use 1MB buffer for streaming large captures
                let pcap_reader = LegacyPcapReader::new(1024 * 1024, reader).map_err(|e| {
                    PcapError::Parse(format!("failed to create pcap reader: {:?}", e))
                })?;
                // Default to Ethernet (linktype 1) for legacy pcap
                // We'll extract the actual linktype from the first header block
                Ok(Self {
                    reader: Box::new(pcap_reader),
                    tls_keys: TlsKeyStore::new(),
                    interfaces: HashMap::new(),
                    default_linktype: 1, // Will be updated from header
                })
            }
            _ => Err(PcapError::UnsupportedFormat(format!(
                "unknown magic bytes: {:02x}{:02x}{:02x}{:02x}",
                magic[0], magic[1], magic[2], magic[3]
            ))),
        }
    }

    /// Returns a reference to the extracted TLS keys.
    pub fn tls_keys(&self) -> &TlsKeyStore {
        &self.tls_keys
    }

    /// Consumes the reader and returns the TLS key store.
    pub fn into_tls_keys(self) -> TlsKeyStore {
        self.tls_keys
    }

    /// Reads all packets from the capture file.
    ///
    /// This method consumes the reader and processes all blocks,
    /// extracting packets and DSB entries.
    pub fn read_all_packets(&mut self) -> Result<Vec<PcapPacket>, PcapError> {
        let mut packets = Vec::new();

        loop {
            match self.reader.next() {
                Ok((offset, block)) => {
                    // Process the block based on type
                    let default_linktype = self.default_linktype;
                    match block {
                        PcapBlockOwned::LegacyHeader(header) => {
                            // Legacy pcap file header - extract linktype
                            self.default_linktype = header.network.0 as u32;
                        }
                        PcapBlockOwned::Legacy(packet) => {
                            // Legacy pcap packet
                            let timestamp_us =
                                packet.ts_sec as u64 * 1_000_000 + packet.ts_usec as u64;
                            packets.push(PcapPacket {
                                linktype: default_linktype,
                                timestamp_us,
                                data: packet.data.to_vec(),
                            });
                        }
                        PcapBlockOwned::NG(ng_block) => {
                            // Process pcapng block - must be done in a way that doesn't borrow self twice
                            Self::process_ng_block_static(
                                ng_block,
                                &mut packets,
                                &mut self.interfaces,
                                &mut self.tls_keys,
                                default_linktype,
                            )?;
                        }
                    }
                    self.reader.consume(offset);
                }
                Err(pcap_parser::PcapError::Eof) => break,
                Err(pcap_parser::PcapError::Incomplete(_)) => {
                    // Need more data, but we're at EOF
                    break;
                }
                Err(e) => {
                    return Err(PcapError::Parse(format!("parse error: {:?}", e)));
                }
            }
        }

        Ok(packets)
    }

    /// Processes a pcapng block (static version to avoid borrow issues).
    fn process_ng_block_static(
        block: Block,
        packets: &mut Vec<PcapPacket>,
        interfaces: &mut HashMap<u32, Linktype>,
        tls_keys: &mut TlsKeyStore,
        default_linktype: Linktype,
    ) -> Result<(), PcapError> {
        match block {
            Block::InterfaceDescription(idb) => {
                // Track interface linktype
                let interface_id = interfaces.len() as u32;
                interfaces.insert(interface_id, idb.linktype.0 as u32);
            }
            Block::EnhancedPacket(epb) => {
                // Enhanced packet with interface reference
                let linktype = *interfaces.get(&epb.if_id).unwrap_or(&default_linktype);
                let timestamp_us = (epb.ts_high as u64) << 32 | epb.ts_low as u64;
                packets.push(PcapPacket {
                    linktype,
                    timestamp_us,
                    data: epb.data.to_vec(),
                });
            }
            Block::SimplePacket(spb) => {
                // Simple packet uses interface 0
                let linktype = *interfaces.get(&0).unwrap_or(&default_linktype);
                packets.push(PcapPacket {
                    linktype,
                    timestamp_us: 0, // Simple packets don't have timestamps
                    data: spb.data.to_vec(),
                });
            }
            Block::DecryptionSecrets(dsb) => {
                // Extract TLS key material
                Self::extract_tls_keys_static(&dsb, tls_keys)?;
            }
            _ => {
                // Ignore other block types (section header, interface stats, etc.)
            }
        }
        Ok(())
    }

    /// Extracts TLS keys from a Decryption Secrets Block (static version).
    fn extract_tls_keys_static(
        dsb: &DecryptionSecretsBlock,
        tls_keys: &mut TlsKeyStore,
    ) -> Result<(), PcapError> {
        // DSB secrets_type: 0x544c534b = "TLSK" (TLS Key Log)
        if dsb.secrets_type != SecretsType(0x544c534b) {
            return Ok(()); // Not a TLS key log
        }

        // Parse TLS key log format (NSS Key Log Format)
        // Format: "CLIENT_RANDOM <hex client_random> <hex master_secret>"
        let secrets_data = std::str::from_utf8(dsb.data)
            .map_err(|e| PcapError::TlsKey(format!("invalid UTF-8 in DSB: {}", e)))?;

        for line in secrets_data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            if parts[0] == "CLIENT_RANDOM" {
                let client_random = hex::decode(parts[1]).map_err(|e| {
                    PcapError::TlsKey(format!("invalid hex in client_random: {}", e))
                })?;
                let master_secret = hex::decode(parts[2]).map_err(|e| {
                    PcapError::TlsKey(format!("invalid hex in master_secret: {}", e))
                })?;
                tls_keys.insert(client_random, master_secret);
            }
        }

        Ok(())
    }
}
