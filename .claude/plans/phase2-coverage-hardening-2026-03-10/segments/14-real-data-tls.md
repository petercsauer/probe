---
segment: 14
title: "Real-Data Tests: TLS Decryption with Keylogs"
depends_on: [4, 11]
risk: 5
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(prb-pcap): add real-data TLS decryption tests with keylog files"
---

# Segment 14: Real-Data Tests — TLS Decryption with Keylogs

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world TLS captures with matching SSLKEYLOGFILE data, and write tests that verify the full decrypt → decode pipeline.

**Depends on:** Segments 4 (pcap pipeline tests), 11 (integration tests)

## Data Sources

### From tex2e/openssl-playground (TLS version samples)
- **TLS 1.3 capture + keylog** — pcapng with matching key material
- **TLS 1.2 capture + keylog** — pcapng with matching key material
- Repo: `https://github.com/tex2e/openssl-playground`
- Contains captures for TLS 1.3, 1.2, 1.1, 1.0 with key material

### From lbirchler/tls-decryption
- Pre-made pcap + keylog pairs in `data/` directory
- Repo: `https://github.com/lbirchler/tls-decryption`

### From pan-unit42/wireshark-tutorial-decrypting-HTTPS-traffic
- Tutorial pcap files with matching key files
- Repo: `https://github.com/pan-unit42/wireshark-tutorial-decrypting-HTTPS-traffic`

### From Wireshark Wiki TLS page
- `https://wiki.wireshark.org/TLS` references sample captures with keys
- `dump.pcapng` + `premaster.txt` pair documented on wiki

### Self-generated (if needed)
Generate minimal TLS captures using:
```bash
# Terminal 1: start a simple TLS server
openssl s_server -accept 4433 -cert cert.pem -key key.pem -keylogfile /tmp/keylog.txt

# Terminal 2: capture + connect
tcpdump -i lo0 port 4433 -w tls_test.pcap &
curl -k --tlsv1.3 https://localhost:4433/
```

Store fixtures in `tests/fixtures/captures/tls/`.

## Scope

- `tests/fixtures/captures/tls/*.pcapng` — TLS capture files
- `tests/fixtures/captures/tls/*.keylog` — Matching SSLKEYLOGFILE data
- `crates/prb-pcap/tests/real_data_tls_tests.rs` — New test file

## Implementation Approach

### Test TLS 1.3 decryption
```rust
#[test]
fn test_tls13_decrypt_with_keylog() {
    // Load pcap + keylog
    // Run through pipeline with TLS decryption enabled
    // Assert: TLS sessions decrypted (not just encrypted blobs)
    // Assert: Decrypted payload contains recognizable HTTP/application data
}
```

### Test TLS 1.2 decryption
```rust
#[test]
fn test_tls12_decrypt_with_keylog() {
    // Same as above but with TLS 1.2 capture
    // Verify CLIENT_RANDOM key format parsing
}
```

### Test mixed TLS versions
```rust
#[test]
fn test_mixed_tls_versions_same_capture() {
    // If capture contains both TLS 1.2 and 1.3 sessions
    // Both should decrypt correctly
}
```

### Test failure paths
```rust
#[test]
fn test_tls_without_keylog_produces_encrypted_events() {
    // Same capture but NO keylog
    // Pipeline should still produce events, but payloads are encrypted
    // Assert: events exist but protocol is "tls" not "grpc" etc.
}

#[test]
fn test_tls_with_wrong_keylog() {
    // Mismatched keylog → decryption fails gracefully
    // Assert: no panic, events still created
}
```

### Test keylog file parsing edge cases
```rust
#[test]
fn test_keylog_with_comments_and_blank_lines() {
    // Keylog file with # comments, blank lines, trailing whitespace
}

#[test]
fn test_keylog_with_all_secret_types() {
    // CLIENT_RANDOM, CLIENT_HANDSHAKE_TRAFFIC_SECRET, SERVER_HANDSHAKE_TRAFFIC_SECRET,
    // CLIENT_TRAFFIC_SECRET_0, SERVER_TRAFFIC_SECRET_0, EARLY_TRAFFIC_SECRET
}
```

## Pre-Mortem Risks

- Some TLS captures may use cipher suites not supported by the prb TLS decryptor
- Key material format varies between TLS versions — ensure parser handles all NSS keylog formats
- Self-generated captures may be needed if downloaded ones have incompatible formats

## Build and Test Commands

- Build: `cargo check -p prb-pcap`
- Test (targeted): `cargo nextest run -p prb-pcap -E 'test(real_data_tls)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 6 TLS decryption tests with real captures, all passing
2. **Fixture files:** At least 2 pcap+keylog pairs committed
3. **TLS version coverage:** Tests for both TLS 1.2 and TLS 1.3
4. **Failure paths:** Tests for missing keylog, wrong keylog, unsupported cipher
5. **Regression tests:** `cargo nextest run --workspace` — no regressions
6. **Full build gate:** `cargo build --workspace`
7. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
8. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
