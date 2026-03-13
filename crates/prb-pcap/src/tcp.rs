//! TCP stream reassembly using smoltcp's Assembler.
//!
//! This module provides stateful TCP stream reassembly, handling:
//! - Out-of-order segment delivery
//! - Retransmissions
//! - Bidirectional streams (client-to-server and server-to-client)
//! - Connection state tracking (SYN/FIN/RST)
//! - Packet loss tolerance with gap skipping
//! - Mid-connection start (captures without SYN)
//! - Connection timeout and cleanup

use crate::error::PcapError;
use crate::normalize::{NormalizedPacket, OwnedNormalizedPacket, TcpFlags, TransportInfo};
use smoltcp::storage::Assembler;
use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Range;

/// Default connection timeout in microseconds (30 seconds).
const DEFAULT_TIMEOUT_US: u64 = 30_000_000;

/// Direction of TCP stream data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamDirection {
    /// Client to server (initiator to responder).
    ClientToServer,
    /// Server to client (responder to initiator).
    ServerToClient,
}

/// A unique key identifying a TCP connection (4-tuple).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConnectionKey {
    src_ip: IpAddr,
    src_port: u16,
    dst_ip: IpAddr,
    dst_port: u16,
}

impl ConnectionKey {
    const fn new(src_ip: IpAddr, src_port: u16, dst_ip: IpAddr, dst_port: u16) -> Self {
        Self {
            src_ip,
            src_port,
            dst_ip,
            dst_port,
        }
    }

    /// Returns the reverse key (swap src/dst).
    const fn reverse(&self) -> Self {
        Self {
            src_ip: self.dst_ip,
            src_port: self.dst_port,
            dst_ip: self.src_ip,
            dst_port: self.src_port,
        }
    }
}

/// State of a single direction in a TCP connection.
struct DirectionState {
    assembler: Assembler,
    initial_seq: Option<u32>,
    last_activity_us: u64,
    first_packet_timestamp_us: Option<u64>,
    fin_seen: bool,
    bytes_buffered: usize,
    /// Buffer storing actual payload bytes, indexed by relative sequence number
    payload_buffer: HashMap<usize, Vec<u8>>,
    /// Offset of the next byte to extract (tracks consumed position)
    consumed_offset: usize,
}

impl DirectionState {
    fn new() -> Self {
        Self {
            assembler: Assembler::new(),
            initial_seq: None,
            last_activity_us: 0,
            first_packet_timestamp_us: None,
            fin_seen: false,
            bytes_buffered: 0,
            payload_buffer: HashMap::new(),
            consumed_offset: 0,
        }
    }

    /// Returns the relative sequence number (offset from ISN).
    const fn relative_seq(&self, seq: u32) -> usize {
        match self.initial_seq {
            Some(isn) => seq.wrapping_sub(isn) as usize,
            None => 0,
        }
    }

    /// Computes missing ranges (gaps) in the buffered data.
    fn get_missing_ranges(&self) -> Vec<Range<u64>> {
        if self.payload_buffer.is_empty() {
            return Vec::new();
        }

        // Collect all (offset, end_offset) pairs from buffered chunks
        let mut ranges: Vec<(usize, usize)> = self
            .payload_buffer
            .iter()
            .map(|(offset, data)| (*offset, *offset + data.len()))
            .collect();

        // Sort by start offset
        ranges.sort_by_key(|r| r.0);

        // Find gaps between consumed_offset and buffered ranges
        let mut gaps = Vec::new();

        // Check for gap from consumed_offset to first buffered chunk
        if let Some(&(first_start, _)) = ranges.first() {
            if self.consumed_offset < first_start {
                gaps.push(Range {
                    start: self.consumed_offset as u64,
                    end: first_start as u64,
                });
            }
        }

        // Find gaps between consecutive ranges
        for i in 0..ranges.len().saturating_sub(1) {
            let (_start1, end1) = ranges[i];
            let (start2, _end2) = ranges[i + 1];

            // If there's a gap between end1 and start2
            if end1 < start2 {
                gaps.push(Range {
                    start: end1 as u64,
                    end: start2 as u64,
                });
            }
        }

        gaps
    }
}

/// State of a bidirectional TCP connection.
struct ConnectionState {
    client_to_server: DirectionState,
    server_to_client: DirectionState,
    rst_seen: bool,
}

impl ConnectionState {
    fn new() -> Self {
        Self {
            client_to_server: DirectionState::new(),
            server_to_client: DirectionState::new(),
            rst_seen: false,
        }
    }
}

/// A reassembled TCP stream segment.
#[derive(Debug, Clone)]
pub struct ReassembledStream {
    /// Connection 4-tuple (client perspective).
    pub src_ip: IpAddr,
    pub src_port: u16,
    pub dst_ip: IpAddr,
    pub dst_port: u16,
    /// Direction of this stream segment.
    pub direction: StreamDirection,
    /// Reassembled data.
    pub data: Vec<u8>,
    /// Whether the stream is complete (FIN or RST seen).
    pub is_complete: bool,
    /// Ranges of missing data (gaps in sequence space).
    pub missing_ranges: Vec<Range<u64>>,
    /// Capture timestamp of the first packet in this stream (microseconds since epoch).
    pub timestamp_us: u64,
}

/// Events produced by the TCP reassembler.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Data was reassembled and is ready for consumption.
    Data(ReassembledStream),
    /// A gap was detected and skipped (packet loss tolerance).
    GapSkipped {
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
        direction: StreamDirection,
        gap_size: usize,
    },
    /// Connection timed out and was flushed.
    Timeout {
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
    },
}

/// TCP stream reassembler.
pub struct TcpReassembler {
    connections: HashMap<ConnectionKey, ConnectionState>,
    timeout_us: u64,
}

impl TcpReassembler {
    /// Creates a new TCP reassembler with default timeout (30 seconds).
    #[must_use]
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_TIMEOUT_US)
    }

    /// Creates a new TCP reassembler with a custom timeout in microseconds.
    #[must_use]
    pub fn with_timeout(timeout_us: u64) -> Self {
        Self {
            connections: HashMap::new(),
            timeout_us,
        }
    }

    /// Processes a TCP segment from a normalized packet.
    ///
    /// Returns a vector of stream events (data, gaps, timeouts).
    pub fn process_segment(
        &mut self,
        packet: &NormalizedPacket,
    ) -> Result<Vec<StreamEvent>, PcapError> {
        let tcp_info = match &packet.transport {
            TransportInfo::Tcp(info) => info,
            _ => return Ok(Vec::new()), // Not a TCP packet
        };

        self.process_segment_inner(
            packet.src_ip,
            tcp_info.src_port,
            packet.dst_ip,
            tcp_info.dst_port,
            tcp_info.seq,
            tcp_info.ack,
            tcp_info.flags,
            packet.payload,
            packet.timestamp_us,
        )
    }

    /// Creates a flush event from a direction state (static method to avoid borrow issues).
    fn create_flush_event(
        key: &ConnectionKey,
        dir_state: &DirectionState,
        direction: StreamDirection,
    ) -> Option<ReassembledStream> {
        if dir_state.bytes_buffered == 0 {
            return None;
        }

        let (src_ip, src_port, dst_ip, dst_port) = match direction {
            StreamDirection::ClientToServer => (key.src_ip, key.src_port, key.dst_ip, key.dst_port),
            StreamDirection::ServerToClient => (key.dst_ip, key.dst_port, key.src_ip, key.src_port),
        };

        // Extract all buffered data (in sequence order)
        let mut sorted_offsets: Vec<_> = dir_state.payload_buffer.keys().copied().collect();
        sorted_offsets.sort_unstable();

        let mut data = Vec::new();
        for offset in sorted_offsets {
            if let Some(chunk) = dir_state.payload_buffer.get(&offset) {
                data.extend_from_slice(chunk);
            }
        }

        Some(ReassembledStream {
            src_ip,
            src_port,
            dst_ip,
            dst_port,
            direction,
            data,
            is_complete: dir_state.fin_seen,
            missing_ranges: dir_state.get_missing_ranges(),
            timestamp_us: dir_state
                .first_packet_timestamp_us
                .unwrap_or(dir_state.last_activity_us),
        })
    }

    /// Processes a TCP segment from an owned normalized packet.
    ///
    /// Semantically identical to `process_segment` but works with owned data.
    /// This is useful for parallel processing pipelines where packets are moved
    /// across thread boundaries.
    ///
    /// Returns a vector of stream events (data, gaps, timeouts).
    pub fn process_owned_segment(
        &mut self,
        packet: &OwnedNormalizedPacket,
    ) -> Result<Vec<StreamEvent>, PcapError> {
        let tcp_info = match &packet.transport {
            TransportInfo::Tcp(info) => info,
            _ => return Ok(Vec::new()), // Not a TCP packet
        };

        self.process_segment_inner(
            packet.src_ip,
            tcp_info.src_port,
            packet.dst_ip,
            tcp_info.dst_port,
            tcp_info.seq,
            tcp_info.ack,
            tcp_info.flags,
            &packet.payload,
            packet.timestamp_us,
        )
    }

    /// Core TCP segment processing logic shared by both borrowed and owned variants.
    #[allow(clippy::too_many_arguments)]
    fn process_segment_inner(
        &mut self,
        src_ip: IpAddr,
        src_port: u16,
        dst_ip: IpAddr,
        dst_port: u16,
        seq: u32,
        _ack: u32,
        flags: TcpFlags,
        payload: &[u8],
        timestamp_us: u64,
    ) -> Result<Vec<StreamEvent>, PcapError> {
        let mut events = Vec::new();

        // Create connection key (client perspective: lower port is typically client)
        let key = ConnectionKey::new(src_ip, src_port, dst_ip, dst_port);

        // Check if this is the reverse direction
        let reverse_key = key.reverse();
        let (canonical_key, direction) = if self.connections.contains_key(&key) {
            (key, StreamDirection::ClientToServer)
        } else if self.connections.contains_key(&reverse_key) {
            (reverse_key, StreamDirection::ServerToClient)
        } else {
            // New connection - assume lower port is client (heuristic)
            if src_port < dst_port {
                (key, StreamDirection::ClientToServer)
            } else {
                (reverse_key, StreamDirection::ServerToClient)
            }
        };

        // Handle RST flag - need to do this in a scope to drop the borrow
        if flags.rst {
            // Get or create connection state
            let conn_state = self
                .connections
                .entry(canonical_key.clone())
                .or_insert_with(ConnectionState::new);
            conn_state.rst_seen = true;
            // Flush both directions
            let c2s_data = Self::create_flush_event(
                &canonical_key,
                &conn_state.client_to_server,
                StreamDirection::ClientToServer,
            );
            let s2c_data = Self::create_flush_event(
                &canonical_key,
                &conn_state.server_to_client,
                StreamDirection::ServerToClient,
            );
            if let Some(data) = c2s_data {
                events.push(StreamEvent::Data(data));
            }
            if let Some(data) = s2c_data {
                events.push(StreamEvent::Data(data));
            }
            // Exit the scope to drop conn_state borrow, then remove
        }
        if flags.rst {
            self.connections.remove(&canonical_key);
            return Ok(events);
        }

        // Get or create connection state for normal processing
        let conn_state = self
            .connections
            .entry(canonical_key.clone())
            .or_insert_with(ConnectionState::new);

        // Select the direction state
        let dir_state = match direction {
            StreamDirection::ClientToServer => &mut conn_state.client_to_server,
            StreamDirection::ServerToClient => &mut conn_state.server_to_client,
        };

        // Initialize sequence number on first segment
        if dir_state.initial_seq.is_none() {
            // If this is a SYN packet, the ISN should be seq+1 because SYN consumes one sequence number
            let isn = if flags.syn { seq.wrapping_add(1) } else { seq };
            dir_state.initial_seq = Some(isn);
            dir_state.first_packet_timestamp_us = Some(timestamp_us);
        }

        // Update last activity timestamp
        dir_state.last_activity_us = timestamp_us;

        // Handle FIN flag
        if flags.fin {
            dir_state.fin_seen = true;
        }

        // Feed data into assembler if there's payload
        if !payload.is_empty() {
            let rel_seq = dir_state.relative_seq(seq);

            // Store the actual payload bytes
            dir_state.payload_buffer.insert(rel_seq, payload.to_vec());

            // Add range to assembler
            let _ = dir_state.assembler.add(rel_seq, rel_seq + payload.len());
            dir_state.bytes_buffered += payload.len();

            // Check if we have contiguous data starting from consumed_offset
            // The assembler tracks ranges, so we need to check from our consumption point
            let available_from_base = dir_state.assembler.peek_front();
            let contig_len = if dir_state.consumed_offset == 0 {
                available_from_base
            } else {
                // After consuming some bytes, check if new data extends contiguously
                // by seeing if the payload_buffer has data at consumed_offset
                if dir_state
                    .payload_buffer
                    .contains_key(&dir_state.consumed_offset)
                {
                    // Calculate how much contiguous data we have from consumed_offset
                    let mut len = 0;
                    let mut check_offset = dir_state.consumed_offset;
                    while let Some(chunk) = dir_state.payload_buffer.get(&check_offset) {
                        len += chunk.len();
                        check_offset += chunk.len();
                    }
                    len
                } else {
                    0
                }
            };

            if contig_len > 0 {
                // Extract contiguous data from buffer starting from consumed_offset
                let mut data = Vec::with_capacity(contig_len);
                let start_offset = dir_state.consumed_offset;
                let end_offset = start_offset + contig_len;
                let mut offset = start_offset;

                while offset < end_offset {
                    if let Some(chunk) = dir_state.payload_buffer.remove(&offset) {
                        data.extend_from_slice(&chunk);
                        offset += chunk.len();
                    } else {
                        break;
                    }
                }

                dir_state.consumed_offset += data.len();
                dir_state.bytes_buffered = dir_state.bytes_buffered.saturating_sub(data.len());

                // Emit reassembled data
                let (src_ip_out, src_port_out, dst_ip_out, dst_port_out) = match direction {
                    StreamDirection::ClientToServer => (
                        canonical_key.src_ip,
                        canonical_key.src_port,
                        canonical_key.dst_ip,
                        canonical_key.dst_port,
                    ),
                    StreamDirection::ServerToClient => (
                        canonical_key.dst_ip,
                        canonical_key.dst_port,
                        canonical_key.src_ip,
                        canonical_key.src_port,
                    ),
                };

                events.push(StreamEvent::Data(ReassembledStream {
                    src_ip: src_ip_out,
                    src_port: src_port_out,
                    dst_ip: dst_ip_out,
                    dst_port: dst_port_out,
                    direction,
                    data,
                    is_complete: dir_state.fin_seen,
                    missing_ranges: dir_state.get_missing_ranges(),
                    timestamp_us: dir_state.first_packet_timestamp_us.unwrap_or(timestamp_us),
                }));
            }
        }

        Ok(events)
    }

    /// Flushes all active connections, emitting any buffered data.
    ///
    /// Called at end of shard processing to ensure all pending data is emitted.
    /// Connections are removed after flushing.
    ///
    /// Returns stream events for all flushed connections.
    pub fn flush_all(&mut self) -> Vec<StreamEvent> {
        let keys: Vec<ConnectionKey> = self.connections.keys().cloned().collect();
        let mut events = Vec::new();

        for key in keys {
            if let Some(state) = self.connections.remove(&key) {
                if let Some(c2s) = Self::create_flush_event(
                    &key,
                    &state.client_to_server,
                    StreamDirection::ClientToServer,
                ) {
                    events.push(StreamEvent::Data(c2s));
                }
                if let Some(s2c) = Self::create_flush_event(
                    &key,
                    &state.server_to_client,
                    StreamDirection::ServerToClient,
                ) {
                    events.push(StreamEvent::Data(s2c));
                }
            }
        }

        events
    }

    /// Cleans up idle connections that have exceeded the timeout.
    pub fn cleanup_idle_connections(&mut self, current_time_us: u64) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        let timeout_us = self.timeout_us;

        self.connections.retain(|key, state| {
            let c2s_idle = current_time_us.saturating_sub(state.client_to_server.last_activity_us);
            let s2c_idle = current_time_us.saturating_sub(state.server_to_client.last_activity_us);
            let idle_time = c2s_idle.min(s2c_idle);

            if idle_time > timeout_us {
                events.push(StreamEvent::Timeout {
                    src_ip: key.src_ip,
                    src_port: key.src_port,
                    dst_ip: key.dst_ip,
                    dst_port: key.dst_port,
                });
                false // Remove connection
            } else {
                true // Keep connection
            }
        });

        events
    }

    /// Returns the number of active connections.
    #[must_use]
    pub fn active_connections(&self) -> usize {
        self.connections.len()
    }
}

impl Default for TcpReassembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    /// Helper to create a TCP segment packet for testing.
    fn create_tcp_segment<'a>(
        src_port: u16,
        dst_port: u16,
        seq: u32,
        payload: &'a [u8],
        timestamp_us: u64,
    ) -> NormalizedPacket<'a> {
        NormalizedPacket {
            timestamp_us,
            src_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)),
            transport: TransportInfo::Tcp(crate::normalize::TcpSegmentInfo {
                src_port,
                dst_port,
                seq,
                ack: 0,
                flags: TcpFlags {
                    syn: false,
                    ack: false,
                    fin: false,
                    rst: false,
                    psh: false,
                },
            }),
            vlan_id: None,
            payload,
        }
    }

    #[test]
    fn test_tcp_gaps_exposed() {
        let mut reassembler = TcpReassembler::new();

        // Create segments with a gap: 0-50, then 100-150 (gap: 50-100)
        let pkt1 = create_tcp_segment(12345, 80, 1000, &[0x01; 50], 1000);
        let pkt2 = create_tcp_segment(12345, 80, 1100, &[0x02; 50], 2000);

        // Process first segment (0-50 relative)
        let events1 = reassembler.process_segment(&pkt1).unwrap();
        assert_eq!(events1.len(), 1, "Should emit reassembled data");

        // Process second segment (100-150 relative), which creates a gap
        let _events2 = reassembler.process_segment(&pkt2).unwrap();

        // Second segment should not emit data yet (out of order)
        // But when we flush, we should see the gap
        let flush_events = reassembler.flush_all();

        // Find the stream with buffered data
        let mut found_gap = false;
        for event in flush_events {
            if let StreamEvent::Data(stream) = event {
                if !stream.missing_ranges.is_empty() {
                    // We should have a gap from 50 to 100
                    assert_eq!(stream.missing_ranges.len(), 1);
                    assert_eq!(stream.missing_ranges[0].start, 50);
                    assert_eq!(stream.missing_ranges[0].end, 100);
                    found_gap = true;
                }
            }
        }

        assert!(found_gap, "Should have found a gap in the TCP stream");
    }

    #[test]
    fn test_tcp_no_gaps_when_contiguous() {
        let mut reassembler = TcpReassembler::new();

        // Create contiguous segments: 0-50, then 50-100
        let pkt1 = create_tcp_segment(12345, 80, 1000, &[0x01; 50], 1000);
        let pkt2 = create_tcp_segment(12345, 80, 1050, &[0x02; 50], 2000);

        // Process both segments
        let events1 = reassembler.process_segment(&pkt1).unwrap();
        let events2 = reassembler.process_segment(&pkt2).unwrap();

        // Verify no gaps in emitted events
        for event in events1.iter().chain(events2.iter()) {
            if let StreamEvent::Data(stream) = event {
                assert!(
                    stream.missing_ranges.is_empty(),
                    "Contiguous data should have no gaps"
                );
            }
        }
    }
}
