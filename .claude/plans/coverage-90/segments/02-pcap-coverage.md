---
segment: 02
title: prb-pcap to 95%
depends_on: []
risk: 4
complexity: High
cycle_budget: 10
estimated_lines: ~250 test lines
---

# Segment 02: prb-pcap Coverage to 95%

## Context

**Current:** 87.59% (89.05% regions)
**Target:** 95%
**Gap:** +7.41 percentage points (~474 uncovered lines)

**Critical gaps:**
- `src/normalize.rs` - 80.59% (91 lines uncovered) - protocol normalization edge cases
- `src/tls/decrypt.rs` - 82.56% (55 lines uncovered) - TLS decryption error paths
- `src/pipeline_core.rs` - 82.89% (40 lines uncovered) - pipeline error handling
- `src/pipeline.rs` - 77.14% (20 lines uncovered) - protocol override logic
- `src/reader.rs` - 81.86% (31 lines uncovered) - PCAP format edge cases

**Existing tests:** 413 lines added in S07 (partial), targeting error paths

## Goal

Comprehensive tests for TLS decryption, protocol normalization, and pipeline error paths.

## Exit Criteria

1. [ ] prb-pcap ≥95%
2. [ ] tls/decrypt.rs ≥90%
3. [ ] normalize.rs ≥88%
4. [ ] All tests pass
5. [ ] TLS key handling edge cases covered

## Implementation Plan

### Priority 1: TLS Decryption Edge Cases (~100 lines)

```rust
// crates/prb-pcap/tests/tls_decrypt_edge_tests.rs

#[test]
fn test_tls13_decrypt_with_missing_key() {
    let keylog = TlsKeyLog::new();
    let encrypted_record = create_tls13_record(cipher: AES_128_GCM_SHA256);
    let result = decrypt_tls13_record(&encrypted_record, &keylog);
    assert!(matches!(result, Err(TlsError::KeyNotFound(_))));
}

#[test]
fn test_tls12_decrypt_with_invalid_mac() {
    // Test MAC verification failure
}

#[test]
fn test_unsupported_cipher_suite() {
    // Test TLS_CHACHA20_POLY1305 (not yet supported)
}

#[test]
fn test_tls_key_derivation_edge_cases() {
    // Test HKDF with edge case inputs
}
```

### Priority 2: Normalize Protocol Variants (~80 lines)

```rust
// crates/prb-pcap/tests/normalize_edge_tests.rs

#[test]
fn test_normalize_tcp_with_options() {
    let packet = create_tcp_packet_with_options(vec![TcpOption::Timestamp, TcpOption::WindowScale]);
    let result = normalize_packet(&packet);
    assert!(result.is_ok());
}

#[test]
fn test_normalize_fragmented_ip() {
    // Test IP fragmentation handling
}

#[test]
fn test_normalize_vlan_tagged() {
    // Test 802.1Q VLAN tagging
}
```

### Priority 3: Pipeline Error Paths (~70 lines)

Test TCP reassembly failures, worker panics, channel closures.

## Test Plan

1. `cargo llvm-cov -p prb-pcap --html`
2. Target tls/decrypt.rs specific uncovered lines
3. Add normalize edge cases
4. Verify: `cargo llvm-cov -p prb-pcap --summary-only`
5. Commit: "test: Increase prb-pcap coverage to 95% (TLS + normalize edge cases)"

## Success Metrics

- prb-pcap: 87.59% → 95%+
- tls/decrypt.rs: 82.56% → 90%+
- ~30-35 new test functions

