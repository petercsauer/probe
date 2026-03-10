//! Packet normalization: converts raw link-layer packets into normalized
//! network-layer tuples (IP header + transport + payload).
//!
//! Handles multiple linktypes (Ethernet, SLL, SLL2, Raw IP, Loopback),
//! VLAN stripping, and IP fragment reassembly.

use crate::error::PcapError;
use etherparse::{NetSlice, SlicedPacket, TransportSlice};
use std::net::IpAddr;

/// TCP segment metadata for stream reassembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpSegmentInfo {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq: u32,
    pub ack: u32,
    pub flags: TcpFlags,
}

/// TCP flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpFlags {
    pub syn: bool,
    pub ack: bool,
    pub fin: bool,
    pub rst: bool,
    pub psh: bool,
}

impl TcpFlags {
    /// Parse TCP flags from the flags byte (lower 6 bits).
    pub fn from_byte(flags: u8) -> Self {
        Self {
            fin: flags & 0x01 != 0,
            syn: flags & 0x02 != 0,
            rst: flags & 0x04 != 0,
            psh: flags & 0x08 != 0,
            ack: flags & 0x10 != 0,
        }
    }
}

/// Transport protocol information extracted from a packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportInfo {
    /// TCP segment with full metadata for reassembly.
    Tcp(TcpSegmentInfo),
    /// UDP datagram with source and destination ports.
    Udp { src_port: u16, dst_port: u16 },
    /// Other transport protocol (e.g., ICMP, IGMP) or unknown.
    Other(u8),
}

/// A normalized packet with IP and transport layer information extracted.
#[derive(Debug, Clone)]
pub struct NormalizedPacket<'a> {
    /// Packet timestamp in microseconds since UNIX epoch.
    pub timestamp_us: u64,
    /// Source IP address (IPv4 or IPv6).
    pub src_ip: IpAddr,
    /// Destination IP address (IPv4 or IPv6).
    pub dst_ip: IpAddr,
    /// Transport protocol information.
    pub transport: TransportInfo,
    /// VLAN ID if present (supports single or double VLAN tags).
    pub vlan_id: Option<u16>,
    /// Raw payload (transport layer payload, e.g., TCP data or UDP payload).
    pub payload: &'a [u8],
}

/// Owned variant of `NormalizedPacket` for cross-thread transfer.
///
/// This type is `Send + Sync` and can be moved across thread boundaries,
/// making it suitable for parallel pipeline processing with rayon.
#[derive(Debug, Clone)]
pub struct OwnedNormalizedPacket {
    /// Packet timestamp in microseconds since UNIX epoch.
    pub timestamp_us: u64,
    /// Source IP address (IPv4 or IPv6).
    pub src_ip: IpAddr,
    /// Destination IP address (IPv4 or IPv6).
    pub dst_ip: IpAddr,
    /// Transport protocol information.
    pub transport: TransportInfo,
    /// VLAN ID if present (supports single or double VLAN tags).
    pub vlan_id: Option<u16>,
    /// Owned payload bytes.
    pub payload: Vec<u8>,
}

impl OwnedNormalizedPacket {
    /// Creates an owned packet from a borrowed packet.
    ///
    /// This copies the payload bytes to create an owned variant.
    pub fn from_borrowed(packet: &NormalizedPacket<'_>) -> Self {
        Self {
            timestamp_us: packet.timestamp_us,
            src_ip: packet.src_ip,
            dst_ip: packet.dst_ip,
            transport: packet.transport.clone(),
            vlan_id: packet.vlan_id,
            payload: packet.payload.to_vec(),
        }
    }

    /// Alias for from_borrowed for convenience.
    pub fn from_normalized(packet: &NormalizedPacket<'_>) -> Self {
        Self::from_borrowed(packet)
    }

    /// Creates a borrowed NormalizedPacket from this owned packet.
    ///
    /// The returned packet borrows the payload from `self`.
    pub fn as_normalized(&self) -> NormalizedPacket<'_> {
        NormalizedPacket {
            timestamp_us: self.timestamp_us,
            src_ip: self.src_ip,
            dst_ip: self.dst_ip,
            transport: self.transport.clone(),
            vlan_id: self.vlan_id,
            payload: &self.payload,
        }
    }
}

/// Maximum age for incomplete IP fragment trains (5 seconds in microseconds).
const DEFRAG_TIMEOUT_US: u64 = 5_000_000;

/// How often to run defrag cleanup (every 1000 packets).
const DEFRAG_CLEANUP_INTERVAL: u64 = 1000;

/// Packet normalizer that handles linktype dispatch and IP fragment reassembly.
pub struct PacketNormalizer {
    defrag_pool: etherparse::defrag::IpDefragPool<u64, ()>,
    packet_count: u64,
    last_timestamp_us: u64,
}

impl PacketNormalizer {
    /// Creates a new packet normalizer.
    pub fn new() -> Self {
        Self {
            defrag_pool: etherparse::defrag::IpDefragPool::new(),
            packet_count: 0,
            last_timestamp_us: 0,
        }
    }

    /// Normalizes a raw packet based on its linktype.
    ///
    /// Returns `Ok(Some(packet))` if a complete packet is available (may be reassembled),
    /// `Ok(None)` if the packet is a fragment waiting for more data, or `Err` on parse error.
    pub fn normalize<'a>(
        &'a mut self,
        linktype: u32,
        timestamp_us: u64,
        data: &'a [u8],
    ) -> Result<Option<NormalizedPacket<'a>>, PcapError> {
        self.packet_count += 1;
        self.last_timestamp_us = timestamp_us;

        // Periodically evict stale incomplete fragment trains to bound memory
        if self.packet_count.is_multiple_of(DEFRAG_CLEANUP_INTERVAL) {
            let cutoff = timestamp_us.saturating_sub(DEFRAG_TIMEOUT_US);
            self.defrag_pool.retain(|ts| *ts >= cutoff);
        }

        // Dispatch based on linktype
        let sliced = match linktype {
            0 => {
                // Loopback/Null (4-byte AF header)
                self.parse_loopback(data)?
            }
            1 => {
                // Ethernet
                SlicedPacket::from_ethernet(data).map_err(|e| {
                    PcapError::Parse(format!("failed to parse Ethernet packet: {:?}", e))
                })?
            }
            101 => {
                // Raw IP (no link layer)
                SlicedPacket::from_ip(data).map_err(|e| {
                    PcapError::Parse(format!("failed to parse Raw IP packet: {:?}", e))
                })?
            }
            113 => {
                // Linux SLL (cooked capture)
                SlicedPacket::from_linux_sll(data).map_err(|e| {
                    PcapError::Parse(format!("failed to parse SLL packet: {:?}", e))
                })?
            }
            276 => {
                // Linux SLL2 (not supported by etherparse, use custom parser)
                self.parse_sll2(data)?
            }
            _ => {
                return Err(PcapError::InvalidLinktype(format!(
                    "unsupported linktype: {}",
                    linktype
                )))
            }
        };

        // Extract VLAN ID if present (use first VLAN ID if multiple)
        let vlan_id = sliced.vlan_ids().first().map(|v| v.value());

        // Extract IP layer and check for fragmentation
        let net = match &sliced.net {
            Some(net) => net,
            None => {
                return Err(PcapError::Parse(
                    "no network layer found in packet".to_string(),
                ))
            }
        };

        // Check if packet is fragmented - if so, use defragmentation pool
        let is_fragmented = match net {
            NetSlice::Ipv4(ipv4) => ipv4.payload().fragmented,
            NetSlice::Ipv6(ipv6) => ipv6.payload().fragmented,
            NetSlice::Arp(_) => {
                return Err(PcapError::Parse(
                    "ARP packets not supported for normalization".to_string(),
                ))
            }
        };

        if is_fragmented {
            // Extract source and destination IPs before passing to defrag
            let (src_ip, dst_ip) = match net {
                NetSlice::Ipv4(ipv4) => (
                    IpAddr::V4(ipv4.header().source_addr()),
                    IpAddr::V4(ipv4.header().destination_addr()),
                ),
                NetSlice::Ipv6(ipv6) => (
                    IpAddr::V6(ipv6.header().source_addr()),
                    IpAddr::V6(ipv6.header().destination_addr()),
                ),
                NetSlice::Arp(_) => unreachable!(), // Already handled above
            };

            // Feed into defragmentation pool
            match self
                .defrag_pool
                .process_sliced_packet(&sliced, timestamp_us, ())
            {
                Ok(Some(reassembled)) => {
                    // Reassembly complete, return reassembled payload
                    return self.create_normalized_packet_from_reassembled(
                        timestamp_us,
                        src_ip,
                        dst_ip,
                        vlan_id,
                        reassembled,
                    );
                }
                Ok(None) => {
                    // Waiting for more fragments
                    return Ok(None);
                }
                Err(e) => {
                    return Err(PcapError::Parse(format!("IP defrag error: {:?}", e)));
                }
            }
        }

        // Non-fragmented packet path - extract all info from sliced packet
        let (src_ip, dst_ip, ip_payload_slice) = match net {
            NetSlice::Ipv4(ipv4) => (
                IpAddr::V4(ipv4.header().source_addr()),
                IpAddr::V4(ipv4.header().destination_addr()),
                ipv4.payload(),
            ),
            NetSlice::Ipv6(ipv6) => (
                IpAddr::V6(ipv6.header().source_addr()),
                IpAddr::V6(ipv6.header().destination_addr()),
                ipv6.payload(),
            ),
            NetSlice::Arp(_) => unreachable!(), // Already handled above
        };

        // Parse transport layer from sliced packet (non-fragmented path)
        let (transport, payload) = match sliced.transport {
            Some(TransportSlice::Tcp(tcp)) => {
                let src_port = tcp.source_port();
                let dst_port = tcp.destination_port();
                let seq = tcp.sequence_number();
                let ack = tcp.acknowledgment_number();
                let header = tcp.to_header();
                let flags = TcpFlags {
                    syn: header.syn,
                    ack: header.ack,
                    fin: header.fin,
                    rst: header.rst,
                    psh: header.psh,
                };
                let payload = tcp.payload();
                (
                    TransportInfo::Tcp(TcpSegmentInfo {
                        src_port,
                        dst_port,
                        seq,
                        ack,
                        flags,
                    }),
                    payload,
                )
            }
            Some(TransportSlice::Udp(udp)) => {
                let src_port = udp.source_port();
                let dst_port = udp.destination_port();
                let payload = udp.payload();
                (TransportInfo::Udp { src_port, dst_port }, payload)
            }
            Some(TransportSlice::Icmpv4(icmp)) => {
                (TransportInfo::Other(1), icmp.payload())
            }
            Some(TransportSlice::Icmpv6(icmp)) => {
                (TransportInfo::Other(58), icmp.payload())
            }
            None => {
                // No transport layer or unknown protocol
                (
                    TransportInfo::Other(ip_payload_slice.ip_number.0),
                    ip_payload_slice.payload,
                )
            }
        };

        Ok(Some(NormalizedPacket {
            timestamp_us,
            src_ip,
            dst_ip,
            transport,
            vlan_id,
            payload,
        }))
    }

    /// Creates a normalized packet from reassembled IP fragment data.
    fn create_normalized_packet_from_reassembled(
        &self,
        timestamp_us: u64,
        src_ip: IpAddr,
        dst_ip: IpAddr,
        vlan_id: Option<u16>,
        reassembled: etherparse::defrag::IpDefragPayloadVec,
    ) -> Result<Option<NormalizedPacket<'static>>, PcapError> {
        let data = reassembled.payload;
        let (transport, payload) = Self::parse_transport_from_bytes(&data)?;

        // Convert to owned data since reassembled is owned
        let payload_owned = payload.to_vec();
        let payload_static: &'static [u8] = Box::leak(payload_owned.into_boxed_slice());

        Ok(Some(NormalizedPacket {
            timestamp_us,
            src_ip,
            dst_ip,
            transport,
            vlan_id,
            payload: payload_static,
        }))
    }

    /// Parses transport layer from raw bytes (used for reassembled packets).
    fn parse_transport_from_bytes(data: &[u8]) -> Result<(TransportInfo, &[u8]), PcapError> {
        if data.is_empty() {
            return Ok((TransportInfo::Other(0), data));
        }

        // Try to parse as TCP (minimum 20 bytes)
        if data.len() >= 20 {
            // TCP header: src_port (2) + dst_port (2) + seq (4) + ack (4) + offset+flags (2) + ...
            let src_port = u16::from_be_bytes([data[0], data[1]]);
            let dst_port = u16::from_be_bytes([data[2], data[3]]);
            let seq = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            let ack = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
            let offset_flags = u16::from_be_bytes([data[12], data[13]]);
            let data_offset = ((offset_flags >> 12) * 4) as usize;
            let flags = TcpFlags::from_byte((offset_flags & 0xFF) as u8);

            if data_offset >= 20 && data_offset <= data.len() {
                // Valid TCP header
                let payload = &data[data_offset..];
                return Ok((
                    TransportInfo::Tcp(TcpSegmentInfo {
                        src_port,
                        dst_port,
                        seq,
                        ack,
                        flags,
                    }),
                    payload,
                ));
            }
        }

        // Try to parse as UDP (minimum 8 bytes)
        if data.len() >= 8 {
            let src_port = u16::from_be_bytes([data[0], data[1]]);
            let dst_port = u16::from_be_bytes([data[2], data[3]]);
            let length = u16::from_be_bytes([data[4], data[5]]) as usize;

            // UDP length includes 8-byte header
            if length >= 8 && length <= data.len() {
                let payload = &data[8..];
                return Ok((TransportInfo::Udp { src_port, dst_port }, payload));
            }
        }

        // Unknown or malformed transport layer
        Ok((TransportInfo::Other(0), data))
    }

    /// Parses Loopback/Null packets (linktype 0).
    ///
    /// Format: 4-byte AF family header + IP packet.
    /// AF values differ by OS:
    /// - AF_INET (IPv4): 2 (all platforms)
    /// - AF_INET6 (IPv6): 30 (macOS/BSD), 10 (Linux)
    fn parse_loopback<'a>(&self, data: &'a [u8]) -> Result<SlicedPacket<'a>, PcapError> {
        if data.len() < 4 {
            return Err(PcapError::Parse(
                "loopback packet too short (need 4-byte header)".to_string(),
            ));
        }

        // Read AF family value (little-endian u32)
        let af_family = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        // Validate AF family and parse IP packet
        match af_family {
            2 => {
                // AF_INET (IPv4)
                SlicedPacket::from_ip(&data[4..]).map_err(|e| {
                    PcapError::Parse(format!("failed to parse IPv4 in loopback packet: {:?}", e))
                })
            }
            10 | 30 => {
                // AF_INET6 (IPv6): 10 on Linux, 30 on macOS/BSD
                SlicedPacket::from_ip(&data[4..]).map_err(|e| {
                    PcapError::Parse(format!("failed to parse IPv6 in loopback packet: {:?}", e))
                })
            }
            _ => Err(PcapError::Parse(format!(
                "unsupported AF family in loopback packet: {}",
                af_family
            ))),
        }
    }

    /// Parses SLL2 (Linux cooked capture v2, linktype 276).
    ///
    /// etherparse does not support SLL2, so we parse the header manually.
    /// SLL2 header format (20 bytes):
    /// - protocol_type: u16 (BE) at offset 0
    /// - reserved: u16 at offset 2
    /// - interface_index: u32 (BE) at offset 4
    /// - arphrd_type: u16 (BE) at offset 8
    /// - packet_type: u8 at offset 10
    /// - link_layer_addr_len: u8 at offset 11
    /// - link_layer_addr: [u8; 8] at offset 12
    ///
    /// Protocol type is EtherType (e.g., 0x0800 for IPv4, 0x86dd for IPv6).
    fn parse_sll2<'a>(&self, data: &'a [u8]) -> Result<SlicedPacket<'a>, PcapError> {
        if data.len() < 20 {
            return Err(PcapError::Parse(
                "SLL2 packet too short (need 20-byte header)".to_string(),
            ));
        }

        // Extract protocol type (EtherType)
        let protocol_type = u16::from_be_bytes([data[0], data[1]]);

        // Parse payload based on protocol type
        let payload = &data[20..];
        match protocol_type {
            0x0800 | 0x86dd => {
                // IPv4 or IPv6
                SlicedPacket::from_ip(payload).map_err(|e| {
                    PcapError::Parse(format!(
                        "failed to parse IP packet in SLL2 frame: {:?}",
                        e
                    ))
                })
            }
            _ => Err(PcapError::Parse(format!(
                "unsupported protocol type in SLL2 frame: 0x{:04x}",
                protocol_type
            ))),
        }
    }
}

impl Default for PacketNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of stateless packet normalization.
///
/// Either a complete normalized packet or a fragment marker requiring
/// stateful reassembly.
#[derive(Debug)]
pub enum NormalizeResult {
    /// Successfully normalized non-fragmented packet.
    Packet(OwnedNormalizedPacket),
    /// IP fragment detected; requires stateful defrag pool.
    Fragment {
        timestamp_us: u64,
        linktype: u32,
        data_len: usize,
    },
}

/// Normalizes a single non-fragmented packet (stateless, thread-safe).
///
/// This function extracts IP and transport layer information from a raw packet
/// without maintaining any state. It returns `NormalizeResult::Fragment` for
/// IP fragments that require stateful reassembly.
///
/// This function is safe to call from multiple threads in parallel.
///
/// # Arguments
///
/// * `linktype` - PCAP linktype (e.g., 1=Ethernet, 113=SLL, 276=SLL2)
/// * `timestamp_us` - Packet timestamp in microseconds since UNIX epoch
/// * `data` - Raw packet bytes including link layer header
pub fn normalize_stateless(
    linktype: u32,
    timestamp_us: u64,
    data: &[u8],
) -> Result<NormalizeResult, PcapError> {
    // Dispatch based on linktype
    let sliced = match linktype {
        0 => parse_loopback_static(data)?,
        1 => SlicedPacket::from_ethernet(data)
            .map_err(|e| PcapError::Parse(format!("Ethernet parse: {:?}", e)))?,
        101 => SlicedPacket::from_ip(data)
            .map_err(|e| PcapError::Parse(format!("Raw IP parse: {:?}", e)))?,
        113 => SlicedPacket::from_linux_sll(data)
            .map_err(|e| PcapError::Parse(format!("SLL parse: {:?}", e)))?,
        276 => parse_sll2_static(data)?,
        _ => {
            return Err(PcapError::InvalidLinktype(format!(
                "unsupported: {}",
                linktype
            )))
        }
    };

    let vlan_id = sliced.vlan_ids().first().map(|v| v.value());

    let net = sliced.net.as_ref().ok_or_else(|| {
        PcapError::Parse("no network layer".into())
    })?;

    let is_fragmented = match net {
        NetSlice::Ipv4(ipv4) => ipv4.payload().fragmented,
        NetSlice::Ipv6(ipv6) => ipv6.payload().fragmented,
        NetSlice::Arp(_) => {
            return Err(PcapError::Parse("ARP not supported".into()))
        }
    };

    if is_fragmented {
        return Ok(NormalizeResult::Fragment {
            timestamp_us,
            linktype,
            data_len: data.len(),
        });
    }

    // Non-fragmented: extract everything
    let (src_ip, dst_ip) = extract_ips(net);
    let (transport, payload): (TransportInfo, &[u8]) = extract_transport(&sliced)?;

    Ok(NormalizeResult::Packet(OwnedNormalizedPacket {
        timestamp_us,
        src_ip,
        dst_ip,
        transport,
        vlan_id,
        payload: Vec::from(payload),
    }))
}

/// Extracts IP addresses from a network slice (helper for stateless normalization).
fn extract_ips(net: &NetSlice) -> (IpAddr, IpAddr) {
    match net {
        NetSlice::Ipv4(ipv4) => (
            IpAddr::V4(ipv4.header().source_addr()),
            IpAddr::V4(ipv4.header().destination_addr()),
        ),
        NetSlice::Ipv6(ipv6) => (
            IpAddr::V6(ipv6.header().source_addr()),
            IpAddr::V6(ipv6.header().destination_addr()),
        ),
        NetSlice::Arp(_) => unreachable!("ARP handled earlier"),
    }
}

/// Extracts transport layer information from a sliced packet (helper for stateless normalization).
fn extract_transport<'a>(sliced: &'a SlicedPacket<'a>) -> Result<(TransportInfo, &'a [u8]), PcapError> {
    let net = sliced.net.as_ref().ok_or_else(|| {
        PcapError::Parse("no network layer".into())
    })?;

    let ip_payload_slice = match net {
        NetSlice::Ipv4(ipv4) => ipv4.payload(),
        NetSlice::Ipv6(ipv6) => ipv6.payload(),
        NetSlice::Arp(_) => return Err(PcapError::Parse("ARP not supported".into())),
    };

    match &sliced.transport {
        Some(TransportSlice::Tcp(tcp)) => {
            let header = tcp.to_header();
            Ok((
                TransportInfo::Tcp(TcpSegmentInfo {
                    src_port: tcp.source_port(),
                    dst_port: tcp.destination_port(),
                    seq: tcp.sequence_number(),
                    ack: tcp.acknowledgment_number(),
                    flags: TcpFlags {
                        syn: header.syn,
                        ack: header.ack,
                        fin: header.fin,
                        rst: header.rst,
                        psh: header.psh,
                    },
                }),
                tcp.payload(),
            ))
        }
        Some(TransportSlice::Udp(udp)) => Ok((
            TransportInfo::Udp {
                src_port: udp.source_port(),
                dst_port: udp.destination_port(),
            },
            udp.payload(),
        )),
        Some(TransportSlice::Icmpv4(icmp)) => {
            Ok((TransportInfo::Other(1), icmp.payload()))
        }
        Some(TransportSlice::Icmpv6(icmp)) => {
            Ok((TransportInfo::Other(58), icmp.payload()))
        }
        None => Ok((
            TransportInfo::Other(ip_payload_slice.ip_number.0),
            ip_payload_slice.payload,
        )),
    }
}

/// Parses Loopback/Null packets (linktype 0) - static version for parallel processing.
fn parse_loopback_static(data: &[u8]) -> Result<SlicedPacket, PcapError> {
    if data.len() < 4 {
        return Err(PcapError::Parse(
            "loopback packet too short (need 4-byte header)".to_string(),
        ));
    }

    let af_family = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

    match af_family {
        2 => SlicedPacket::from_ip(&data[4..]).map_err(|e| {
            PcapError::Parse(format!("failed to parse IPv4 in loopback packet: {:?}", e))
        }),
        10 | 30 => SlicedPacket::from_ip(&data[4..]).map_err(|e| {
            PcapError::Parse(format!("failed to parse IPv6 in loopback packet: {:?}", e))
        }),
        _ => Err(PcapError::Parse(format!(
            "unsupported AF family in loopback packet: {}",
            af_family
        ))),
    }
}

/// Parses SLL2 packets (linktype 276) - static version for parallel processing.
fn parse_sll2_static(data: &[u8]) -> Result<SlicedPacket, PcapError> {
    if data.len() < 20 {
        return Err(PcapError::Parse(
            "SLL2 packet too short (need 20-byte header)".to_string(),
        ));
    }

    let protocol_type = u16::from_be_bytes([data[0], data[1]]);

    let payload = &data[20..];
    match protocol_type {
        0x0800 | 0x86dd => SlicedPacket::from_ip(payload).map_err(|e| {
            PcapError::Parse(format!(
                "failed to parse IP packet in SLL2 frame: {:?}",
                e
            ))
        }),
        _ => Err(PcapError::Parse(format!(
            "unsupported protocol type in SLL2 frame: 0x{:04x}",
            protocol_type
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a minimal Ethernet + IPv4 + TCP packet.
    fn create_ethernet_ipv4_tcp() -> Vec<u8> {
        use etherparse::PacketBuilder;

        let builder = PacketBuilder::ethernet2(
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55], // src MAC
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff], // dst MAC
        )
        .ipv4(
            [192, 168, 1, 1], // src IP
            [10, 0, 0, 1],    // dst IP
            64,               // TTL
        )
        .tcp(
            12345, // src port
            80,    // dst port
            1000,  // seq
            4096,  // window
        );

        let payload = b"Hello TCP";
        let mut packet = Vec::new();
        builder.write(&mut packet, payload).unwrap();
        packet
    }

    /// Helper to create a minimal Ethernet + IPv4 + UDP packet.
    fn create_ethernet_ipv4_udp() -> Vec<u8> {
        use etherparse::PacketBuilder;

        let builder = PacketBuilder::ethernet2(
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        )
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .udp(
            12345, // src port
            53,    // dst port (DNS)
        );

        let payload = b"Hello UDP";
        let mut packet = Vec::new();
        builder.write(&mut packet, payload).unwrap();
        packet
    }

    #[test]
    fn test_ethernet_ipv4_tcp() {
        let packet = create_ethernet_ipv4_tcp();
        let mut normalizer = PacketNormalizer::new();

        let result = normalizer.normalize(1, 1000000, &packet).unwrap();
        assert!(result.is_some());

        let normalized = result.unwrap();
        assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
        assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
        if let TransportInfo::Tcp(tcp_info) = &normalized.transport {
            assert_eq!(tcp_info.src_port, 12345);
            assert_eq!(tcp_info.dst_port, 80);
        } else {
            panic!("Expected TCP transport info");
        }
        assert_eq!(normalized.payload, b"Hello TCP");
        assert_eq!(normalized.vlan_id, None);
    }

    #[test]
    fn test_ethernet_ipv4_udp() {
        let packet = create_ethernet_ipv4_udp();
        let mut normalizer = PacketNormalizer::new();

        let result = normalizer.normalize(1, 1000000, &packet).unwrap();
        assert!(result.is_some());

        let normalized = result.unwrap();
        assert_eq!(normalized.src_ip, IpAddr::from([192, 168, 1, 1]));
        assert_eq!(normalized.dst_ip, IpAddr::from([10, 0, 0, 1]));
        assert_eq!(
            normalized.transport,
            TransportInfo::Udp {
                src_port: 12345,
                dst_port: 53
            }
        );
        assert_eq!(normalized.payload, b"Hello UDP");
    }

    // Additional tests will be added in a separate test file to meet exit criteria
}
