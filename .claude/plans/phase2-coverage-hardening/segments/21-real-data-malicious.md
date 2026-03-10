---
segment: 21
title: "Real-Data Tests: Malicious Traffic and Fuzzing Robustness"
depends_on: [15, 16, 17]
risk: 5
complexity: High
cycle_budget: 6
status: pending
commit_message: "test(prb-pcap): add adversarial traffic tests and parser robustness validation"
---

# Segment 21: Real-Data Tests — Malicious Traffic and Fuzzing Robustness

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download known-malicious and adversarial network captures. Verify the pipeline never panics, never hangs, and degrades gracefully on malformed input.

**Depends on:** Segments 15 (TCP/IP real data), 16 (DNS real data), 17 (HTTP real data)

## Data Sources

### From Malware Traffic Analysis
1. **2026 captures** — `https://malware-traffic-analysis.net/2026/index.html`
   - Pick 2-3 recent captures with known malware families
   - These include: HTTP C2 traffic, DNS beaconing, SSL/TLS with self-signed certs
2. **Traffic analysis exercises** — Full pcap with mixed legitimate + malicious traffic

### From Wireshark Wiki SampleCaptures (attack patterns)
3. **teardrop.cap** — Overlapping IP fragment attack
4. **dns-remoteshell.pcap** — DNS tunneling / exfiltration
5. **Land attack captures** — src==dst IP
6. **Ping of Death** — Oversized ICMP
7. **SYN flood** captures — High packet count, partial connections

### From public CTF / security datasets
8. **UNSW-NB15 dataset** — Network intrusion detection pcaps
   - `https://research.unsw.edu.au/projects/unsw-nb15-dataset`
9. **CICIDS dataset** — Canadian Institute for Cybersecurity
   - `https://www.unb.ca/cic/datasets/ids-2017.html`
10. **Malware-Traffic-Analysis.net exercises** — Realistic multi-protocol malware traffic

### From Wireshark fuzz corpus
11. **wireshark/test/captures/** — Wireshark's own fuzz test corpus
    - `https://gitlab.com/wireshark/wireshark/-/tree/master/test/captures`
    - Contains intentionally malformed captures for parser testing

Store fixtures in `tests/fixtures/captures/adversarial/`.

## Scope

- `tests/fixtures/captures/adversarial/*.pcap` — Malicious/malformed captures
- `crates/prb-pcap/tests/real_data_adversarial_tests.rs` — New test file
- `crates/prb-core/tests/robustness_tests.rs` — Core engine robustness

## Implementation Approach

### No-panic guarantee tests
```rust
#[test]
fn test_teardrop_attack_no_panic() {
    // Load teardrop.cap (overlapping IP fragments)
    // Assert: pipeline completes without panic
    // Assert: some events are produced (even if marked as errors)
}

#[test]
fn test_dns_tunneling_no_panic() {
    // Load dns-remoteshell.pcap
    // Assert: pipeline completes
    // Assert: DNS events have unusually long domain names
}

#[test]
fn test_malware_c2_traffic() {
    // Load recent malware traffic capture
    // Assert: pipeline completes without panic
    // Assert: HTTP/DNS/TLS events produced
    // Assert: no infinite loops (completes within timeout)
}

#[test]
fn test_syn_flood_performance() {
    // Load SYN flood capture
    // Assert: pipeline handles thousands of half-open connections
    // Assert: memory usage stays bounded
    // Assert: completes in reasonable time
}
```

### Malformed packet handling
```rust
#[test]
fn test_truncated_packets_no_panic() {
    // Capture with truncated/snap-length packets
    // Assert: pipeline handles gracefully
    // Assert: partial decode events still useful
}

#[test]
fn test_oversized_headers_no_panic() {
    // Packets with abnormally large header fields
    // Assert: no buffer overflow or OOM
}

#[test]
fn test_zero_length_payloads() {
    // Packets with Content-Length: 0 or empty UDP datagrams
    // Assert: no divide-by-zero, no index-out-of-bounds
}
```

### Mixed legitimate + malicious
```rust
#[test]
fn test_mixed_traffic_capture() {
    // Load CTF exercise capture with mixed traffic
    // Assert: legitimate traffic decoded correctly
    // Assert: malicious traffic doesn't corrupt legitimate events
    // Assert: event count > 0 for multiple protocols
}
```

### Resource exhaustion guards
```rust
#[test]
fn test_large_capture_memory_bounded() {
    // Load largest available capture (>10MB if available)
    // Assert: peak memory stays under 1GB
    // Assert: pipeline streams events, doesn't buffer all in memory
    // Use #[ignore] tag — run with: cargo nextest run -E 'test(memory_bounded)' -- --ignored
}
```

## Pre-Mortem Risks

- Malware captures may trigger antivirus — add exclusions for test fixture dirs
- Some CTF datasets require registration — prefer publicly downloadable captures
- Large captures (100MB+) should NOT be committed — use `#[ignore]` and document download URLs
- Fuzz-generated captures may not be valid pcap format — handle reader errors gracefully

## Build and Test Commands

- Build: `cargo check -p prb-pcap -p prb-core`
- Test (targeted): `cargo nextest run -E 'test(real_data_adversarial)'`
- Test (ignored/large): `cargo nextest run -E 'test(memory_bounded)' -- --ignored`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 8 adversarial/robustness tests, all passing
2. **Fixture files:** At least 4 adversarial capture files (fragments, tunneling, malware, flood)
3. **No-panic guarantee:** Zero panics across all adversarial inputs
4. **Performance bounds:** SYN flood + large capture tests complete within timeout
5. **Regression tests:** `cargo nextest run --workspace` — no regressions
6. **Full build gate:** `cargo build --workspace`
7. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
8. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
