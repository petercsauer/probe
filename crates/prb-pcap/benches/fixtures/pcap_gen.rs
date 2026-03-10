//! Synthetic PCAP fixture generator for benchmarking.

use etherparse::PacketBuilder;
use std::io::Write;

/// Builder for generating deterministic synthetic PCAP files.
pub struct SyntheticPcapBuilder {
    packets: Vec<Vec<u8>>,
    linktype: u32,
}

impl SyntheticPcapBuilder {
    pub fn new() -> Self {
        Self {
            packets: Vec::new(),
            linktype: 1, // Ethernet
        }
    }

    /// Adds N TCP packets across M flows. Each flow has packets with
    /// incrementing sequence numbers and realistic payload sizes.
    pub fn tcp_flows(mut self, num_flows: usize, packets_per_flow: usize) -> Self {
        for flow_idx in 0..num_flows {
            let src_port = 10000 + (flow_idx as u16);
            let dst_port = 50051;
            let src_ip = [10, 0, (flow_idx >> 8) as u8, (flow_idx & 0xFF) as u8];
            let dst_ip = [10, 0, 0, 1];

            let mut seq = 1000u32;
            for pkt_idx in 0..packets_per_flow {
                let payload_size = 100 + (pkt_idx % 900); // 100-999 bytes
                let payload: Vec<u8> = (0..payload_size).map(|i| (i % 256) as u8).collect();

                let builder = PacketBuilder::ethernet2(
                    [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
                    [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
                )
                .ipv4(src_ip, dst_ip, 64)
                .tcp(src_port, dst_port, seq, 65535);

                let mut packet = Vec::new();
                builder.write(&mut packet, &payload).unwrap();
                self.packets.push(packet);

                seq = seq.wrapping_add(payload_size as u32);
            }
        }
        self
    }

    /// Adds N UDP packets (e.g., simulating DDS/RTPS).
    #[allow(dead_code)] // Used in tests
    pub fn udp_packets(mut self, count: usize) -> Self {
        for i in 0..count {
            let src_port = 7400 + (i % 100) as u16;
            let payload: Vec<u8> = vec![0x52, 0x54, 0x50, 0x53]; // "RTPS" magic
            let builder = PacketBuilder::ethernet2(
                [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
                [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            )
            .ipv4([10, 0, 0, 1], [239, 255, 0, 1], 64)
            .udp(src_port, 7400);

            let mut packet = Vec::new();
            builder.write(&mut packet, &payload).unwrap();
            self.packets.push(packet);
        }
        self
    }

    /// Writes the packets to a legacy pcap file in memory.
    pub fn build_pcap(&self) -> Vec<u8> {
        let mut pcap = Vec::new();

        // Global header (24 bytes)
        pcap.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes()); // magic
        pcap.extend_from_slice(&2u16.to_le_bytes()); // version major
        pcap.extend_from_slice(&4u16.to_le_bytes()); // version minor
        pcap.extend_from_slice(&0i32.to_le_bytes()); // thiszone
        pcap.extend_from_slice(&0u32.to_le_bytes()); // sigfigs
        pcap.extend_from_slice(&65535u32.to_le_bytes()); // snaplen
        pcap.extend_from_slice(&self.linktype.to_le_bytes()); // network

        let mut timestamp_us = 1_710_000_000_000_000u64; // ~March 2024
        for pkt in &self.packets {
            let ts_sec = (timestamp_us / 1_000_000) as u32;
            let ts_usec = (timestamp_us % 1_000_000) as u32;
            let len = pkt.len() as u32;

            // Packet record header (16 bytes)
            pcap.extend_from_slice(&ts_sec.to_le_bytes());
            pcap.extend_from_slice(&ts_usec.to_le_bytes());
            pcap.extend_from_slice(&len.to_le_bytes()); // incl_len
            pcap.extend_from_slice(&len.to_le_bytes()); // orig_len
            pcap.extend_from_slice(pkt);

            timestamp_us += 1000; // 1ms between packets
        }

        pcap
    }

    /// Writes to a temp file and returns the path.
    #[allow(dead_code)] // Used in tests
    pub fn write_to_tempfile(&self) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(&self.build_pcap()).unwrap();
        f
    }
}

impl Default for SyntheticPcapBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_synthetic_pcap_valid() {
        use super::SyntheticPcapBuilder;
        let pcap_data = SyntheticPcapBuilder::new()
            .tcp_flows(10, 100)
            .build_pcap();

        let tmpfile = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmpfile.path(), &pcap_data).unwrap();

        // Should be readable by PcapFileReader
        let mut reader = PcapFileReader::open(tmpfile.path()).unwrap();
        let packets = reader.read_all_packets().unwrap();
        assert_eq!(packets.len(), 1000); // 10 flows × 100 packets
    }

    #[test]
    fn test_synthetic_pcap_packet_count() {
        let pcap_data = SyntheticPcapBuilder::new()
            .tcp_flows(5, 20)
            .udp_packets(30)
            .build_pcap();

        let tmpfile = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmpfile.path(), &pcap_data).unwrap();

        let mut reader = PcapFileReader::open(tmpfile.path()).unwrap();
        let packets = reader.read_all_packets().unwrap();
        assert_eq!(packets.len(), 130); // (5 × 20) + 30
    }

    #[test]
    fn test_synthetic_pcap_deterministic() {
        let pcap1 = SyntheticPcapBuilder::new()
            .tcp_flows(3, 10)
            .build_pcap();

        let pcap2 = SyntheticPcapBuilder::new()
            .tcp_flows(3, 10)
            .build_pcap();

        // Same parameters should produce byte-identical output
        assert_eq!(pcap1, pcap2);
    }
}
