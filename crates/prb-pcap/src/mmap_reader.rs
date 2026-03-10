//! Memory-mapped zero-copy PCAP/pcapng reader.
//!
//! Provides efficient random access to large capture files using memory-mapped I/O.
//! Two-phase approach:
//! 1. Index scan: Build Vec<PacketLocation> with file offsets
//! 2. Zero-copy access: Read packet data directly from mmap without copying

use crate::error::PcapError;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

/// Location of a packet within a PCAP/pcapng file.
#[derive(Debug, Clone, Copy)]
pub struct PacketLocation {
    /// Byte offset from start of file.
    pub offset: u64,
    /// Length of packet data in bytes.
    pub length: u32,
    /// Packet timestamp in microseconds since UNIX epoch.
    pub timestamp_us: u64,
    /// Linktype for this packet (determines layer 2 protocol).
    pub linktype: u32,
}

/// Memory-mapped PCAP/pcapng reader with zero-copy access.
///
/// Uses a two-phase approach:
/// 1. Phase 1: Scan file and build index of PacketLocation entries
/// 2. Phase 2: Access packet data via memory-mapped file without copying
///
/// Memory usage: ~50MB working set regardless of file size.
pub struct MmapPcapReader {
    mmap: Mmap,
    index: Vec<PacketLocation>,
    format: CaptureFormat,
}

/// Detected capture file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureFormat {
    /// Legacy PCAP format.
    LegacyPcap,
    /// Modern pcapng format.
    Pcapng,
}

impl MmapPcapReader {
    /// Opens a PCAP or pcapng file and builds the packet index.
    ///
    /// This performs format detection and the initial index scan.
    /// After construction, packet data can be accessed via zero-copy reads.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, PcapError> {
        let file = File::open(path.as_ref())?;
        let mmap = unsafe {
            // Safety: We hold the file open and the mmap is read-only
            Mmap::map(&file)?
        };

        // Detect format from magic bytes
        if mmap.len() < 4 {
            return Err(PcapError::Parse("file too small to be valid PCAP".to_string()));
        }

        let magic = u32::from_le_bytes([mmap[0], mmap[1], mmap[2], mmap[3]]);
        let format = match magic {
            0x0a0d0d0a => CaptureFormat::Pcapng,
            0xa1b2c3d4 | 0xd4c3b2a1 => CaptureFormat::LegacyPcap,
            _ => {
                return Err(PcapError::UnsupportedFormat(format!(
                    "unknown magic bytes: {:08x}",
                    magic
                )))
            }
        };

        // Phase 1: Build index
        let index = match format {
            CaptureFormat::LegacyPcap => Self::index_legacy_pcap(&mmap)?,
            CaptureFormat::Pcapng => Self::index_pcapng(&mmap)?,
        };

        Ok(Self { mmap, index, format })
    }

    /// Returns the number of indexed packets.
    pub fn packet_count(&self) -> usize {
        self.index.len()
    }

    /// Returns the capture file format.
    pub fn format(&self) -> &str {
        match self.format {
            CaptureFormat::LegacyPcap => "pcap",
            CaptureFormat::Pcapng => "pcapng",
        }
    }

    /// Returns a slice of all packet locations.
    pub fn packet_locations(&self) -> &[PacketLocation] {
        &self.index
    }

    /// Gets zero-copy access to packet data by index.
    ///
    /// Returns a slice directly into the memory-mapped file without copying.
    pub fn get_packet_data(&self, index: usize) -> Result<&[u8], PcapError> {
        let location = self
            .index
            .get(index)
            .ok_or_else(|| PcapError::Parse(format!("packet index {} out of bounds", index)))?;

        let start = location.offset as usize;
        let end = start + location.length as usize;

        if end > self.mmap.len() {
            return Err(PcapError::Parse(format!(
                "packet data extends beyond file boundary: {}..{} (file size: {})",
                start,
                end,
                self.mmap.len()
            )));
        }

        Ok(&self.mmap[start..end])
    }

    /// Builds packet index for legacy PCAP format.
    fn index_legacy_pcap(data: &[u8]) -> Result<Vec<PacketLocation>, PcapError> {
        let mut index = Vec::new();
        let mut pos;

        // Read file header (24 bytes)
        if data.len() < 24 {
            return Err(PcapError::Parse("file too small for PCAP header".to_string()));
        }

        // Extract linktype from header (bytes 20-23)
        let linktype = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        pos = 24;

        // Scan packets
        while pos + 16 <= data.len() {
            // Read packet header (16 bytes)
            let ts_sec = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let ts_usec = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
            let incl_len = u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
            let _orig_len = u32::from_le_bytes([data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15]]);

            let packet_data_offset = pos + 16;
            let packet_data_end = packet_data_offset + incl_len as usize;

            if packet_data_end > data.len() {
                // Incomplete packet at EOF
                break;
            }

            let timestamp_us = ts_sec as u64 * 1_000_000 + ts_usec as u64;

            index.push(PacketLocation {
                offset: packet_data_offset as u64,
                length: incl_len,
                timestamp_us,
                linktype,
            });

            pos = packet_data_end;
        }

        Ok(index)
    }

    /// Builds packet index for pcapng format.
    fn index_pcapng(data: &[u8]) -> Result<Vec<PacketLocation>, PcapError> {
        let mut index = Vec::new();
        let mut pos = 0usize;
        let mut interfaces = Vec::new();
        let mut default_linktype = 1u32; // Default to Ethernet

        while pos + 8 <= data.len() {
            // Read block type and total length
            let block_type = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let block_len = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);

            if block_len < 12 {
                return Err(PcapError::Parse(format!(
                    "invalid block length {} at offset {}",
                    block_len, pos
                )));
            }

            let block_end = pos + block_len as usize;
            if block_end > data.len() {
                // Incomplete block at EOF
                break;
            }

            match block_type {
                0x0a0d0d0a => {
                    // Section Header Block (SHB) - continue
                }
                0x00000001 => {
                    // Interface Description Block (IDB)
                    if pos + 16 <= data.len() {
                        let linktype = u16::from_le_bytes([data[pos + 8], data[pos + 9]]) as u32;
                        interfaces.push(linktype);
                        if interfaces.len() == 1 {
                            default_linktype = linktype;
                        }
                    }
                }
                0x00000006 => {
                    // Enhanced Packet Block (EPB)
                    if pos + 28 <= data.len() {
                        let if_id = u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
                        let ts_high = u32::from_le_bytes([data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15]]);
                        let ts_low = u32::from_le_bytes([data[pos + 16], data[pos + 17], data[pos + 18], data[pos + 19]]);
                        let cap_len = u32::from_le_bytes([data[pos + 20], data[pos + 21], data[pos + 22], data[pos + 23]]);
                        let _orig_len = u32::from_le_bytes([data[pos + 24], data[pos + 25], data[pos + 26], data[pos + 27]]);

                        let packet_data_offset = pos + 28;
                        let timestamp_us = ((ts_high as u64) << 32) | (ts_low as u64);
                        let linktype = interfaces.get(if_id as usize).copied().unwrap_or(default_linktype);

                        index.push(PacketLocation {
                            offset: packet_data_offset as u64,
                            length: cap_len,
                            timestamp_us,
                            linktype,
                        });
                    }
                }
                0x00000003 => {
                    // Simple Packet Block (SPB)
                    if pos + 16 <= data.len() {
                        let orig_len = u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
                        let packet_data_offset = pos + 12;
                        let linktype = interfaces.first().copied().unwrap_or(default_linktype);

                        index.push(PacketLocation {
                            offset: packet_data_offset as u64,
                            length: orig_len,
                            timestamp_us: 0, // Simple packets don't have timestamps
                            linktype,
                        });
                    }
                }
                _ => {
                    // Other block types (DSB, ISB, etc.) - skip
                }
            }

            pos = block_end;
        }

        Ok(index)
    }

    /// Iterates over all packets with zero-copy data access.
    ///
    /// Returns (PacketLocation, packet_data) tuples.
    pub fn iter_packets(&self) -> impl Iterator<Item = (PacketLocation, &[u8])> + '_ {
        self.index.iter().enumerate().filter_map(move |(i, &loc)| {
            self.get_packet_data(i).ok().map(|data| (loc, data))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Creates a minimal legacy PCAP file for testing.
    fn create_test_pcap() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        // PCAP file header (24 bytes)
        file.write_all(&0xa1b2c3d4u32.to_le_bytes()).unwrap(); // Magic
        file.write_all(&2u16.to_le_bytes()).unwrap(); // Major version
        file.write_all(&4u16.to_le_bytes()).unwrap(); // Minor version
        file.write_all(&0i32.to_le_bytes()).unwrap(); // Timezone
        file.write_all(&0u32.to_le_bytes()).unwrap(); // Timestamp accuracy
        file.write_all(&65535u32.to_le_bytes()).unwrap(); // Snaplen
        file.write_all(&1u32.to_le_bytes()).unwrap(); // Linktype (Ethernet)

        // Packet 1
        file.write_all(&1000000u32.to_le_bytes()).unwrap(); // ts_sec
        file.write_all(&500000u32.to_le_bytes()).unwrap(); // ts_usec
        let data1 = b"Packet1";
        file.write_all(&(data1.len() as u32).to_le_bytes()).unwrap(); // incl_len
        file.write_all(&(data1.len() as u32).to_le_bytes()).unwrap(); // orig_len
        file.write_all(data1).unwrap();

        // Packet 2
        file.write_all(&2000000u32.to_le_bytes()).unwrap(); // ts_sec
        file.write_all(&123456u32.to_le_bytes()).unwrap(); // ts_usec
        let data2 = b"Packet2Data";
        file.write_all(&(data2.len() as u32).to_le_bytes()).unwrap(); // incl_len
        file.write_all(&(data2.len() as u32).to_le_bytes()).unwrap(); // orig_len
        file.write_all(data2).unwrap();

        file.flush().unwrap();
        file
    }

    /// Creates a minimal pcapng file for testing.
    fn create_test_pcapng() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        // Section Header Block (SHB)
        file.write_all(&0x0a0d0d0au32.to_le_bytes()).unwrap();
        file.write_all(&28u32.to_le_bytes()).unwrap();
        file.write_all(&0x1a2b3c4du32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&0u16.to_le_bytes()).unwrap();
        file.write_all(&(-1i64).to_le_bytes()).unwrap();
        file.write_all(&28u32.to_le_bytes()).unwrap();

        // Interface Description Block (IDB)
        file.write_all(&1u32.to_le_bytes()).unwrap();
        file.write_all(&20u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap(); // Linktype (Ethernet)
        file.write_all(&0u16.to_le_bytes()).unwrap(); // Reserved
        file.write_all(&65535u32.to_le_bytes()).unwrap(); // Snaplen
        file.write_all(&20u32.to_le_bytes()).unwrap();

        // Enhanced Packet Block 1
        let data1 = b"PcapNG1";
        let epb_len1 = 32 + ((data1.len() + 3) & !3);
        file.write_all(&6u32.to_le_bytes()).unwrap();
        file.write_all(&(epb_len1 as u32).to_le_bytes()).unwrap();
        file.write_all(&0u32.to_le_bytes()).unwrap(); // if_id
        file.write_all(&0u32.to_le_bytes()).unwrap(); // ts_high
        file.write_all(&1500000u32.to_le_bytes()).unwrap(); // ts_low
        file.write_all(&(data1.len() as u32).to_le_bytes()).unwrap(); // cap_len
        file.write_all(&(data1.len() as u32).to_le_bytes()).unwrap(); // orig_len
        file.write_all(data1).unwrap();
        let padding1 = (4 - (data1.len() % 4)) % 4;
        file.write_all(&vec![0u8; padding1]).unwrap();
        file.write_all(&(epb_len1 as u32).to_le_bytes()).unwrap();

        // Enhanced Packet Block 2
        let data2 = b"PcapNG2Data";
        let epb_len2 = 32 + ((data2.len() + 3) & !3);
        file.write_all(&6u32.to_le_bytes()).unwrap();
        file.write_all(&(epb_len2 as u32).to_le_bytes()).unwrap();
        file.write_all(&0u32.to_le_bytes()).unwrap(); // if_id
        file.write_all(&0u32.to_le_bytes()).unwrap(); // ts_high
        file.write_all(&3000000u32.to_le_bytes()).unwrap(); // ts_low
        file.write_all(&(data2.len() as u32).to_le_bytes()).unwrap(); // cap_len
        file.write_all(&(data2.len() as u32).to_le_bytes()).unwrap(); // orig_len
        file.write_all(data2).unwrap();
        let padding2 = (4 - (data2.len() % 4)) % 4;
        file.write_all(&vec![0u8; padding2]).unwrap();
        file.write_all(&(epb_len2 as u32).to_le_bytes()).unwrap();

        file.flush().unwrap();
        file
    }

    #[test]
    fn test_mmap_legacy_pcap_index() {
        let pcap_file = create_test_pcap();
        let reader = MmapPcapReader::open(pcap_file.path()).unwrap();

        assert_eq!(reader.format(), "pcap");
        assert_eq!(reader.packet_count(), 2);

        let locations = reader.packet_locations();
        assert_eq!(locations[0].timestamp_us, 1000000 * 1_000_000 + 500000);
        assert_eq!(locations[0].linktype, 1);
        assert_eq!(locations[0].length, 7);

        assert_eq!(locations[1].timestamp_us, 2000000 * 1_000_000 + 123456);
        assert_eq!(locations[1].linktype, 1);
        assert_eq!(locations[1].length, 11);
    }

    #[test]
    fn test_mmap_pcapng_index() {
        let pcapng_file = create_test_pcapng();
        let reader = MmapPcapReader::open(pcapng_file.path()).unwrap();

        assert_eq!(reader.format(), "pcapng");
        assert_eq!(reader.packet_count(), 2);

        let locations = reader.packet_locations();
        assert_eq!(locations[0].timestamp_us, 1500000);
        assert_eq!(locations[0].linktype, 1);
        assert_eq!(locations[0].length, 7);

        assert_eq!(locations[1].timestamp_us, 3000000);
        assert_eq!(locations[1].linktype, 1);
        assert_eq!(locations[1].length, 11);
    }

    #[test]
    fn test_mmap_zero_copy_data() {
        let pcap_file = create_test_pcap();
        let reader = MmapPcapReader::open(pcap_file.path()).unwrap();

        // Access packet data without copying
        let data0 = reader.get_packet_data(0).unwrap();
        assert_eq!(data0, b"Packet1");

        let data1 = reader.get_packet_data(1).unwrap();
        assert_eq!(data1, b"Packet2Data");

        // Out of bounds access
        assert!(reader.get_packet_data(2).is_err());
    }

    #[test]
    fn test_mmap_matches_streaming_reader() {
        use crate::PcapFileReader;

        let pcap_file = create_test_pcap();

        // Read via streaming reader
        let mut streaming_reader = PcapFileReader::open(pcap_file.path()).unwrap();
        let streaming_packets = streaming_reader.read_all_packets().unwrap();

        // Read via mmap reader
        let mmap_reader = MmapPcapReader::open(pcap_file.path()).unwrap();

        assert_eq!(mmap_reader.packet_count(), streaming_packets.len());

        for (i, streaming_packet) in streaming_packets.iter().enumerate() {
            let mmap_data = mmap_reader.get_packet_data(i).unwrap();
            assert_eq!(
                mmap_data, streaming_packet.data,
                "packet {} data should match between streaming and mmap readers",
                i
            );

            let location = &mmap_reader.packet_locations()[i];
            assert_eq!(
                location.timestamp_us, streaming_packet.timestamp_us,
                "packet {} timestamp should match",
                i
            );
            assert_eq!(
                location.linktype, streaming_packet.linktype,
                "packet {} linktype should match",
                i
            );
        }
    }

    #[test]
    fn test_mmap_iterator() {
        let pcap_file = create_test_pcap();
        let reader = MmapPcapReader::open(pcap_file.path()).unwrap();

        let packets: Vec<_> = reader.iter_packets().collect();
        assert_eq!(packets.len(), 2);

        assert_eq!(packets[0].1, b"Packet1");
        assert_eq!(packets[1].1, b"Packet2Data");
    }
}
