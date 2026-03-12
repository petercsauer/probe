//! Flow identification for TCP/UDP connection partitioning.

use crate::normalize::{OwnedNormalizedPacket, TransportInfo};
use std::hash::{Hash, Hasher};
use std::net::IpAddr;

/// Flow protocol identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowProtocol {
    /// TCP connection.
    Tcp,
    /// UDP flow.
    Udp,
}

/// Flow key for deterministic partitioning across shards.
///
/// Uses canonical ordering (lo, hi) to ensure bidirectional packets
/// in the same connection hash to the same value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FlowKey {
    /// Lower IP address (lexicographically).
    pub lo_ip: IpAddr,
    /// Lower port number.
    pub lo_port: u16,
    /// Higher IP address (lexicographically).
    pub hi_ip: IpAddr,
    /// Higher port number.
    pub hi_port: u16,
    /// Transport protocol.
    pub protocol: FlowProtocol,
}

impl FlowKey {
    /// Creates a canonical flow key from a packet.
    ///
    /// Returns `None` if the packet has no TCP/UDP transport info.
    ///
    /// # Canonical Ordering
    ///
    /// The flow key uses lexicographic ordering of (IP, port) tuples:
    /// ```text
    /// Packet: 10.0.0.1:8080 → 192.168.1.1:50051 (TCP)
    /// FlowKey: lo=(10.0.0.1, 8080), hi=(192.168.1.1, 50051)
    ///
    /// Reverse: 192.168.1.1:50051 → 10.0.0.1:8080 (TCP)
    /// FlowKey: lo=(10.0.0.1, 8080), hi=(192.168.1.1, 50051)  <-- same!
    /// ```
    #[must_use] 
    pub fn from_packet(packet: &OwnedNormalizedPacket) -> Option<Self> {
        let (src_port, dst_port, protocol) = match &packet.transport {
            TransportInfo::Tcp(tcp) => (tcp.src_port, tcp.dst_port, FlowProtocol::Tcp),
            TransportInfo::Udp { src_port, dst_port } => (*src_port, *dst_port, FlowProtocol::Udp),
            TransportInfo::Other(_) => return None,
        };

        let (lo_ip, lo_port, hi_ip, hi_port) =
            if (packet.src_ip, src_port) <= (packet.dst_ip, dst_port) {
                (packet.src_ip, src_port, packet.dst_ip, dst_port)
            } else {
                (packet.dst_ip, dst_port, packet.src_ip, src_port)
            };

        Some(Self {
            lo_ip,
            lo_port,
            hi_ip,
            hi_port,
            protocol,
        })
    }

    /// Computes a shard index for this flow.
    ///
    /// Returns a value in `[0, num_shards)` using deterministic hashing.
    /// Packets with the same `FlowKey` always map to the same shard.
    #[must_use] 
    pub fn shard_index(&self, num_shards: usize) -> usize {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        (hasher.finish() as usize) % num_shards
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::{TcpFlags, TcpSegmentInfo};
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn make_tcp_packet(
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
    ) -> OwnedNormalizedPacket {
        OwnedNormalizedPacket {
            timestamp_us: 1000000,
            src_ip,
            dst_ip,
            transport: TransportInfo::Tcp(TcpSegmentInfo {
                src_port,
                dst_port,
                seq: 100,
                ack: 0,
                flags: TcpFlags {
                    syn: true,
                    ack: false,
                    fin: false,
                    rst: false,
                    psh: false,
                },
            }),
            vlan_id: None,
            payload: vec![],
        }
    }

    fn make_udp_packet(
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
    ) -> OwnedNormalizedPacket {
        OwnedNormalizedPacket {
            timestamp_us: 1000000,
            src_ip,
            dst_ip,
            transport: TransportInfo::Udp { src_port, dst_port },
            vlan_id: None,
            payload: vec![],
        }
    }

    #[test]
    fn test_flow_key_canonical_ordering() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Forward direction: 10.0.0.1:8080 → 192.168.1.1:50051
        let pkt_forward = make_tcp_packet(ip1, 8080, ip2, 50051);
        let key_forward = FlowKey::from_packet(&pkt_forward).unwrap();

        // Reverse direction: 192.168.1.1:50051 → 10.0.0.1:8080
        let pkt_reverse = make_tcp_packet(ip2, 50051, ip1, 8080);
        let key_reverse = FlowKey::from_packet(&pkt_reverse).unwrap();

        // Both directions should produce the same flow key
        assert_eq!(key_forward, key_reverse);
        assert_eq!(key_forward.lo_ip, ip1);
        assert_eq!(key_forward.lo_port, 8080);
        assert_eq!(key_forward.hi_ip, ip2);
        assert_eq!(key_forward.hi_port, 50051);
        assert_eq!(key_forward.protocol, FlowProtocol::Tcp);
    }

    #[test]
    fn test_flow_key_different_flows() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip3 = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));

        let pkt1 = make_tcp_packet(ip1, 8080, ip2, 50051);
        let key1 = FlowKey::from_packet(&pkt1).unwrap();

        let pkt2 = make_tcp_packet(ip1, 8080, ip3, 50051);
        let key2 = FlowKey::from_packet(&pkt2).unwrap();

        // Different destination IPs should produce different keys
        assert_ne!(key1, key2);

        let pkt3 = make_tcp_packet(ip1, 9090, ip2, 50051);
        let key3 = FlowKey::from_packet(&pkt3).unwrap();

        // Different source ports should produce different keys
        assert_ne!(key1, key3);

        let pkt4 = make_udp_packet(ip1, 8080, ip2, 50051);
        let key4 = FlowKey::from_packet(&pkt4).unwrap();

        // Different protocols should produce different keys
        assert_ne!(key1, key4);
    }

    #[test]
    fn test_flow_key_shard_deterministic() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let pkt = make_tcp_packet(ip1, 8080, ip2, 50051);
        let key = FlowKey::from_packet(&pkt).unwrap();

        let num_shards = 8;

        // Same key should always produce the same shard index
        let shard1 = key.shard_index(num_shards);
        let shard2 = key.shard_index(num_shards);
        assert_eq!(shard1, shard2);
        assert!(shard1 < num_shards);

        // Reverse direction should map to same shard
        let pkt_reverse = make_tcp_packet(ip2, 50051, ip1, 8080);
        let key_reverse = FlowKey::from_packet(&pkt_reverse).unwrap();
        let shard_reverse = key_reverse.shard_index(num_shards);
        assert_eq!(shard1, shard_reverse);
    }

    #[test]
    fn test_flow_key_ipv6() {
        let ip1 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        let ip2 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2));

        let pkt_forward = make_tcp_packet(ip1, 8080, ip2, 50051);
        let key_forward = FlowKey::from_packet(&pkt_forward).unwrap();

        let pkt_reverse = make_tcp_packet(ip2, 50051, ip1, 8080);
        let key_reverse = FlowKey::from_packet(&pkt_reverse).unwrap();

        // IPv6 canonical ordering should work
        assert_eq!(key_forward, key_reverse);
    }

    #[test]
    fn test_flow_key_other_transport() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let pkt = OwnedNormalizedPacket {
            timestamp_us: 1000000,
            src_ip: ip1,
            dst_ip: ip2,
            transport: TransportInfo::Other(1), // ICMP
            vlan_id: None,
            payload: vec![],
        };

        // ICMP packets should return None
        let key = FlowKey::from_packet(&pkt);
        assert!(key.is_none());
    }
}
