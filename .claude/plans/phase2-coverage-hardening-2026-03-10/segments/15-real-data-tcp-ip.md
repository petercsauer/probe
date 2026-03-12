---
segment: 15
title: "Real-Data Tests: TCP/IP Edge Cases and Reassembly"
depends_on: [4, 11]
risk: 3
complexity: Medium
cycle_budget: 4
status: pending
commit_message: "test(prb-pcap): add real-data TCP reassembly and IP normalization tests"
---

# Segment 15: Real-Data Tests — TCP/IP Edge Cases and Reassembly

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world captures with complex TCP/IP behavior (retransmissions, out-of-order, fragments, VLAN, IPv6) and verify the normalize + reassembly pipeline handles them correctly.

**Depends on:** Segments 4 (pcap pipeline tests), 11 (integration tests)

## Data Sources

### From Wireshark Wiki SampleCaptures
All available at `https://wiki.wireshark.org/SampleCaptures`:
1. **tcp-ecn-sample.pcap** — TCP with Explicit Congestion Notification
2. **200722_tcp_anon.pcapng** — TCP session with various edge cases
3. **vlan.cap.gz** — VLAN-tagged Ethernet frames (802.1Q)
4. **v6.pcap** — IPv6 traffic samples
5. **ipv6-ripng.pcap** — IPv6 with RIPng
6. **teardrop.cap** — Overlapping IP fragments (attack pattern)
7. **dns-remoteshell.pcap** — TCP session with data exfiltration (reassembly test)

### From Wireshark automated captures
- `https://www.wireshark.org/download/automated/captures/` — Various auto-generated captures

### Additional sources
8. **IP fragment reassembly** captures from Wireshark bugs (search for "ip fragment" in GitLab issues)
9. **TCP retransmission** captures — search for "tcp retransmission" on tshark.dev

Store fixtures in `tests/fixtures/captures/tcp/` and `tests/fixtures/captures/ip/`.

## Scope

- `tests/fixtures/captures/tcp/*.pcap` — TCP-focused captures
- `tests/fixtures/captures/ip/*.pcap` — IP-level captures (fragments, IPv6, VLAN)
- `crates/prb-pcap/tests/real_data_tcp_tests.rs` — New test file

## Implementation Approach

### TCP reassembly with real traffic
```rust
#[test]
fn test_tcp_reassembly_real_session() {
    // Load a real TCP capture with multi-packet payloads
    // Run through normalize → reassemble
    // Assert: reassembled stream length > individual packet payloads
    // Assert: stream data is contiguous
}

#[test]
fn test_tcp_retransmission_handling() {
    // Load capture with retransmissions
    // Assert: duplicates are handled (no double-counting)
    // Assert: final reassembled stream is correct
}

#[test]
fn test_tcp_out_of_order_packets() {
    // If available, test with out-of-order capture
    // Assert: reassembly produces correct ordering
}
```

### IP normalization edge cases
```rust
#[test]
fn test_vlan_tagged_frames() {
    // Load vlan.cap
    // Assert: VLAN tags are stripped during normalization
    // Assert: inner packets are correctly extracted
}

#[test]
fn test_ipv6_packet_normalization() {
    // Load v6.pcap
    // Assert: IPv6 packets normalized to NormalizedPacket
    // Assert: src/dst are IPv6 addresses
}

#[test]
fn test_ip_fragment_reassembly() {
    // Load teardrop.cap or similar
    // Assert: fragments are reassembled (or gracefully handled if malicious)
}
```

### Mixed traffic
```rust
#[test]
fn test_mixed_ipv4_ipv6_capture() {
    // Capture with both IPv4 and IPv6 traffic
    // Assert: both address families produce valid NormalizedPackets
}
```

## Pre-Mortem Risks

- Some captures may be very old (different pcap format versions) — verify reader handles them
- Malicious captures (teardrop) should be handled without panic
- Very large captures should use `#[ignore]` tag

## Build and Test Commands

- Build: `cargo check -p prb-pcap`
- Test (targeted): `cargo nextest run -p prb-pcap -E 'test(real_data_tcp)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 6 tests with real TCP/IP captures, all passing
2. **Fixture files:** At least 4 real-world capture files covering VLAN, IPv6, fragments, retransmission
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
