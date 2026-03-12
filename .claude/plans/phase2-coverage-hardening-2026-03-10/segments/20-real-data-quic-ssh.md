---
segment: 20
title: "Real-Data Tests: QUIC, SSH, and Modern Transport Captures"
depends_on: [11, 14]
risk: 4
complexity: Medium
cycle_budget: 4
status: pending
commit_message: "test(prb-pcap,prb-decode): add real-data QUIC, SSH, and modern transport protocol tests"
---

# Segment 20: Real-Data Tests — QUIC, SSH, and Modern Transport Captures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world QUIC (HTTP/3), SSH, WireGuard, and other modern transport captures. Verify protocol detection and decode pipeline handles them.

**Depends on:** Segments 11 (integration tests), 14 (TLS real data — related crypto handling)

## Data Sources

### QUIC / HTTP/3
1. **From Wireshark wiki**:
   - `quic_v1_handshake.pcapng` — QUIC version 1 handshake
   - Available at SampleCaptures page or via `https://wiki.wireshark.org/QUIC`
2. **From quicwg test vectors** — `https://github.com/quicwg/base-drafts/wiki/Implementations`
   - Multiple implementations provide test captures
3. **From Cloudflare blog captures** — Various QUIC captures for testing
4. **Self-generated**:
   ```bash
   tcpdump -i en0 'udp port 443' -w quic_sample.pcap &
   curl --http3 https://cloudflare-quic.com/
   ```

### SSH
5. **From Wireshark wiki**:
   - `ssh.pcap` or `ssh-1.pcap` — SSH session captures
6. **From Wireshark GitLab** — SSH dissector test captures
7. **Self-generated** (trivial):
   ```bash
   tcpdump -i lo0 port 22 -w ssh_session.pcap &
   ssh localhost 'echo hello'
   ```

### WireGuard
8. **From Wireshark wiki**:
   - `wireguard.pcap` — WireGuard handshake + data
9. **WireGuard test vectors** — `https://www.wireguard.com/protocol/`

### SCTP (Stream Control Transmission Protocol)
10. **From Wireshark wiki**:
    - `sctp.cap` — SCTP multi-streaming
    - `sctp-test.cap` — SCTP init/data/shutdown

Store fixtures in `tests/fixtures/captures/quic/`, `tests/fixtures/captures/ssh/`, `tests/fixtures/captures/modern/`.

## Scope

- `tests/fixtures/captures/quic/*.pcapng` — QUIC captures
- `tests/fixtures/captures/ssh/*.pcap` — SSH captures
- `tests/fixtures/captures/modern/*.pcap` — WireGuard, SCTP, etc.
- `crates/prb-pcap/tests/real_data_modern_transport_tests.rs` — New test file

## Implementation Approach

### QUIC tests
```rust
#[test]
fn test_quic_handshake_detection() {
    // Load QUIC capture
    // Assert: QUIC protocol detected (UDP port 443 with QUIC header)
    // Assert: Initial, Handshake, 1-RTT packet types identified
    // Assert: Connection IDs extracted
}

#[test]
fn test_quic_version_negotiation() {
    // If capture includes version negotiation
    // Assert: QUIC version field parsed correctly
}

#[test]
fn test_quic_multistream() {
    // QUIC multiplexes multiple streams in one connection
    // Assert: multiple stream IDs detected
}
```

### SSH tests
```rust
#[test]
fn test_ssh_handshake_real() {
    // Load SSH capture
    // Assert: SSH protocol banner exchange detected ("SSH-2.0-...")
    // Assert: KEX_INIT messages found
    // Assert: Algorithm negotiation (cipher, mac, compression) extractable
}

#[test]
fn test_ssh_session_detection() {
    // Assert: protocol detector identifies SSH on port 22
    // Assert: encrypted session data handled without panic
}

#[test]
fn test_ssh_on_nonstandard_port() {
    // If available, SSH on non-22 port
    // Assert: protocol detection via banner, not just port
}
```

### WireGuard tests
```rust
#[test]
fn test_wireguard_handshake_detection() {
    // Load WireGuard capture
    // Assert: WireGuard message types detected (init, response, cookie, data)
    // Assert: handshake initiation message identified
}
```

### SCTP tests
```rust
#[test]
fn test_sctp_multistream_real() {
    // Load SCTP capture
    // Assert: SCTP INIT, INIT_ACK, DATA chunks found
    // Assert: multiple streams within single association
}
```

## Pre-Mortem Risks

- QUIC payloads are encrypted — only header and connection-level metadata is decodable without keys
- SSH after key exchange is fully encrypted — test focuses on handshake and detection
- WireGuard captures may be rare — self-generation may be needed
- SCTP is less common; pcap may use different link layer types

## Build and Test Commands

- Build: `cargo check -p prb-pcap -p prb-decode`
- Test (targeted): `cargo nextest run -E 'test(real_data_modern_transport)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 7 tests across QUIC, SSH, WireGuard, SCTP, all passing
2. **Fixture files:** At least 1 capture per protocol committed
3. **Protocol detection:** Each protocol correctly identified by detector
4. **Encrypted handling:** Encrypted payloads handled gracefully (events produced, no panics)
5. **Regression tests:** `cargo nextest run --workspace` — no regressions
6. **Full build gate:** `cargo build --workspace`
7. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
8. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
