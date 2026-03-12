---
segment: 16
title: "Real-Data Tests: DNS and DHCP Captures"
depends_on: [11]
risk: 2
complexity: Low
cycle_budget: 3
status: pending
commit_message: "test(prb-pcap,prb-decode): add real-data DNS and DHCP protocol tests"
---

# Segment 16: Real-Data Tests — DNS and DHCP Captures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world DNS and DHCP captures and write tests that verify correct decoding of these common protocols.

**Depends on:** Segment 11 (integration tests)

## Data Sources

### From Wireshark Wiki SampleCaptures
All available at `https://wiki.wireshark.org/SampleCaptures`:
1. **dns.cap** — DNS query/response pairs
2. **dns-remoteshell.pcap** — DNS traffic with tunneling (stress test)
3. **dns_port.pcap** — DNS on non-standard ports
4. **dns-zone-transfer-axfr.pcap** — DNS zone transfer (AXFR)
5. **dhcp.pcap** — DHCP DORA (Discover, Offer, Request, Acknowledge) sequence
6. **dhcp-and-dnsupdate.pcap** — DHCP followed by dynamic DNS update
7. **dhcp-auth.pcap.gz** — DHCP with authentication option

### From Malware Traffic Analysis
8. DNS exfiltration captures — `https://malware-traffic-analysis.net/` (recent 2026 captures)
   - These test DNS parser robustness with unusual query patterns

Store fixtures in `tests/fixtures/captures/dns/` and `tests/fixtures/captures/dhcp/`.

## Scope

- `tests/fixtures/captures/dns/*.pcap` — DNS captures
- `tests/fixtures/captures/dhcp/*.pcap` — DHCP captures
- `crates/prb-pcap/tests/real_data_dns_tests.rs` or `crates/prb-decode/tests/real_data_dns_tests.rs` — New test file

## Implementation Approach

### DNS decode tests
```rust
#[test]
fn test_dns_query_response_real_capture() {
    // Load dns.cap
    // Run through full pipeline
    // Assert: events include DNS queries and responses
    // Assert: domain names are correctly decoded
    // Assert: record types (A, AAAA, CNAME, MX) are present
}

#[test]
fn test_dns_zone_transfer() {
    // Load dns-zone-transfer-axfr.pcap
    // Assert: AXFR query detected
    // Assert: multiple DNS resource records in transfer
}

#[test]
fn test_dns_nonstandard_port() {
    // Load dns_port.pcap
    // Assert: protocol detection finds DNS on non-53 port
}

#[test]
fn test_dns_exfiltration_pattern() {
    // Load DNS tunneling/exfiltration capture
    // Assert: parser handles unusually long domain names without panic
    // Assert: all packets produce valid events
}
```

### DHCP decode tests
```rust
#[test]
fn test_dhcp_dora_sequence() {
    // Load dhcp.pcap
    // Assert: DISCOVER, OFFER, REQUEST, ACK messages decoded
    // Assert: client MAC, offered IP, lease time are extractable
}

#[test]
fn test_dhcp_with_dns_update() {
    // Load dhcp-and-dnsupdate.pcap
    // Assert: both DHCP and DNS events produced from same capture
}
```

## Pre-Mortem Risks

- DNS captures may include EDNS0 extensions or DNSSEC records that the decoder doesn't handle — verify graceful degradation
- DHCP over IPv6 (DHCPv6) may appear in some captures — ensure parser handles or skips gracefully
- Compressed DNS names (pointer compression) must be handled correctly

## Build and Test Commands

- Build: `cargo check -p prb-pcap -p prb-decode`
- Test (targeted): `cargo nextest run -E 'test(real_data_dns) | test(real_data_dhcp)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 5 tests with real DNS/DHCP captures, all passing
2. **Fixture files:** At least 3 DNS + 2 DHCP capture files committed
3. **Protocol coverage:** DNS (standard query, zone transfer, non-standard port) + DHCP (DORA)
4. **Regression tests:** `cargo nextest run --workspace` — no regressions
5. **Full build gate:** `cargo build --workspace`
6. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
7. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
