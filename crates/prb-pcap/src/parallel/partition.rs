//! Flow-based packet partitioning for parallel TCP reassembly.

use crate::flow_key::FlowKey;
use crate::normalize::OwnedNormalizedPacket;

/// Partitions packets into shards by flow key for parallel processing.
///
/// Each shard processes a subset of flows, allowing independent TCP reassembly
/// per shard. The partitioning is deterministic: packets from the same flow
/// always map to the same shard.
pub struct FlowPartitioner {
    num_shards: usize,
}

impl FlowPartitioner {
    /// Creates a new flow partitioner with the specified number of shards.
    ///
    /// # Panics
    ///
    /// Panics if `num_shards` is 0.
    pub fn new(num_shards: usize) -> Self {
        assert!(num_shards > 0, "need at least 1 shard");
        Self { num_shards }
    }

    /// Partitions packets into shards by flow key.
    ///
    /// Packets without a recognized transport protocol (Other) go to shard 0.
    /// Within each shard, packets maintain their original relative order
    /// (stable partitioning). This is critical for TCP reassembly correctness.
    ///
    /// # Arguments
    ///
    /// * `packets` - Vector of owned normalized packets to partition
    ///
    /// # Returns
    ///
    /// A vector of packet vectors, one per shard. Some shards may be empty
    /// if no packets map to them.
    pub fn partition(
        &self,
        packets: Vec<OwnedNormalizedPacket>,
    ) -> Vec<Vec<OwnedNormalizedPacket>> {
        let mut shards: Vec<Vec<OwnedNormalizedPacket>> =
            (0..self.num_shards).map(|_| Vec::new()).collect();

        for packet in packets {
            let shard_idx = FlowKey::from_packet(&packet)
                .map(|k| k.shard_index(self.num_shards))
                .unwrap_or(0);
            shards[shard_idx].push(packet);
        }

        shards
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::{TcpFlags, TcpSegmentInfo, TransportInfo};
    use std::net::{IpAddr, Ipv4Addr};

    fn make_tcp_packet(
        timestamp_us: u64,
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
    ) -> OwnedNormalizedPacket {
        OwnedNormalizedPacket {
            timestamp_us,
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
        timestamp_us: u64,
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
    ) -> OwnedNormalizedPacket {
        OwnedNormalizedPacket {
            timestamp_us,
            src_ip,
            dst_ip,
            transport: TransportInfo::Udp { src_port, dst_port },
            vlan_id: None,
            payload: vec![],
        }
    }

    #[test]
    fn test_partition_single_flow() {
        let partitioner = FlowPartitioner::new(4);
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let packets = vec![
            make_tcp_packet(1000, ip1, 8080, ip2, 50051),
            make_tcp_packet(2000, ip2, 50051, ip1, 8080), // reverse direction
            make_tcp_packet(3000, ip1, 8080, ip2, 50051),
        ];

        let shards = partitioner.partition(packets);

        // All packets should be in the same shard
        let non_empty: Vec<_> = shards.iter().filter(|s| !s.is_empty()).collect();
        assert_eq!(non_empty.len(), 1);

        let shard = non_empty[0];
        assert_eq!(shard.len(), 3);
    }

    #[test]
    fn test_partition_two_flows() {
        let partitioner = FlowPartitioner::new(4);
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip3 = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));

        let packets = vec![
            make_tcp_packet(1000, ip1, 8080, ip2, 50051), // Flow 1
            make_tcp_packet(2000, ip1, 8080, ip3, 50052), // Flow 2
            make_tcp_packet(3000, ip1, 8080, ip2, 50051), // Flow 1
        ];

        let shards = partitioner.partition(packets);

        // Two different flows may be in different shards (or same, depending on hash)
        let non_empty: Vec<_> = shards.iter().filter(|s| !s.is_empty()).collect();
        assert!(!non_empty.is_empty() && non_empty.len() <= 2);

        // Total packets should be preserved
        let total: usize = shards.iter().map(|s| s.len()).sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn test_partition_preserves_order() {
        let partitioner = FlowPartitioner::new(4);
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let packets = vec![
            make_tcp_packet(1000, ip1, 8080, ip2, 50051),
            make_tcp_packet(2000, ip1, 8080, ip2, 50051),
            make_tcp_packet(3000, ip1, 8080, ip2, 50051),
        ];

        let shards = partitioner.partition(packets);

        // Find the shard containing these packets
        for shard in shards {
            if shard.len() == 3 {
                // Timestamps should be in order
                assert!(shard[0].timestamp_us < shard[1].timestamp_us);
                assert!(shard[1].timestamp_us < shard[2].timestamp_us);
            }
        }
    }

    #[test]
    fn test_partition_bidirectional() {
        let partitioner = FlowPartitioner::new(4);
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let packets = vec![
            make_tcp_packet(1000, ip1, 8080, ip2, 50051), // A → B
            make_tcp_packet(2000, ip2, 50051, ip1, 8080), // B → A
        ];

        let shards = partitioner.partition(packets);

        // Both directions should be in the same shard
        let non_empty: Vec<_> = shards.iter().filter(|s| !s.is_empty()).collect();
        assert_eq!(non_empty.len(), 1);
        assert_eq!(non_empty[0].len(), 2);
    }

    #[test]
    fn test_partition_empty() {
        let partitioner = FlowPartitioner::new(4);
        let shards = partitioner.partition(vec![]);
        assert_eq!(shards.len(), 4);
        assert!(shards.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn test_partition_other_transport() {
        let partitioner = FlowPartitioner::new(4);
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let packet = OwnedNormalizedPacket {
            timestamp_us: 1000,
            src_ip: ip1,
            dst_ip: ip2,
            transport: TransportInfo::Other(1), // ICMP
            vlan_id: None,
            payload: vec![],
        };

        let shards = partitioner.partition(vec![packet]);

        // Other transport goes to shard 0
        assert_eq!(shards[0].len(), 1);
        assert!(shards[1..].iter().all(|s| s.is_empty()));
    }

    #[test]
    fn test_partition_mixed_protocols() {
        let partitioner = FlowPartitioner::new(4);
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let packets = vec![
            make_tcp_packet(1000, ip1, 8080, ip2, 50051),
            make_udp_packet(2000, ip1, 9090, ip2, 60061),
            make_tcp_packet(3000, ip1, 8080, ip2, 50051),
        ];

        let shards = partitioner.partition(packets);

        // TCP and UDP with different 5-tuples should be separate flows
        let total: usize = shards.iter().map(|s| s.len()).sum();
        assert_eq!(total, 3);
    }

    #[test]
    #[should_panic(expected = "need at least 1 shard")]
    fn test_partition_zero_shards() {
        FlowPartitioner::new(0);
    }
}
