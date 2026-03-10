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
    fn new(src_ip: IpAddr, src_port: u16, dst_ip: IpAddr, dst_port: u16) -> Self {
        Self {
            src_ip,
            src_port,
            dst_ip,
            dst_port,
        }
    }

    /// Returns the reverse key (swap src/dst).
    fn reverse(&self) -> Self {
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
    fn relative_seq(&self, seq: u32) -> usize {
        match self.initial_seq {
            Some(isn) => seq.wrapping_sub(isn) as usize,
            None => 0,
        }
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
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_TIMEOUT_US)
    }

    /// Creates a new TCP reassembler with a custom timeout in microseconds.
    pub fn with_timeout(timeout_us: u64) -> Self {
        Self {
            connections: HashMap::new(),
            timeout_us,
        }
    }

    /// Processes a TCP segment from a normalized packet.
    ///
    /// Returns a vector of stream events (data, gaps, timeouts).
    pub fn process_segment(&mut self, packet: &NormalizedPacket) -> Result<Vec<StreamEvent>, PcapError> {
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
            missing_ranges: Vec::new(), // TODO: Extract gap ranges from assembler if needed
            timestamp_us: dir_state.first_packet_timestamp_us.unwrap_or(dir_state.last_activity_us),
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
            let isn = if flags.syn {
                seq.wrapping_add(1)
            } else {
                seq
            };
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
                if dir_state.payload_buffer.contains_key(&dir_state.consumed_offset) {
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
                    missing_ranges: Vec::new(), // TODO: Extract gap ranges from assembler if needed
                    timestamp_us: dir_state
                        .first_packet_timestamp_us
                        .unwrap_or(timestamp_us),
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
    pub fn active_connections(&self) -> usize {
        self.connections.len()
    }
}

impl Default for TcpReassembler {
    fn default() -> Self {
        Self::new()
    }
}
