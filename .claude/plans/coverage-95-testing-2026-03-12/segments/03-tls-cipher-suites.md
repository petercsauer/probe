---
segment: 3
title: "TLS Cipher Suite Coverage"
depends_on: [2]
risk: 6/10
complexity: Low
cycle_budget: 10
status: merged
commit_message: "test(pcap-tls): Add coverage for all 9 cipher suites including ChaCha20"
---

# Segment 3: TLS Cipher Suite Coverage

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Test all 9 supported cipher suites (currently only 3-4 tested).

**Depends on:** Segment 2 (AEAD testing framework established)

## Context: Issues Addressed

**Core Problem:** Only 3-4 of 9 supported cipher suites are tested. ChaCha20-Poly1305 variants and SHA384-based cipher suites are completely untested in production scenarios. This creates risk for mobile clients (which prefer ChaCha20) and high-security contexts (which use SHA384).

**Proposed Fix:** Create comprehensive test matrix with rstest covering all 9 cipher suites with their hex IDs. Generate synthetic test data for each using ring::aead. Optionally add real PCAP fixtures if available.

**Pre-Mortem Risks:**
- ChaCha20-Poly1305 might have `ring` API differences from GCM - pin ring version 0.17 and document API usage
- Real PCAP fixtures might be hard to find - synthetic tests sufficient for coverage, real fixtures optional enhancement

## Scope

- Test coverage for all cipher suites (no production code changes expected)
- `crates/prb-pcap/tests/cipher_suite_coverage_test.rs` - New test file
- Optional: `crates/prb-pcap/tests/fixtures/captures/tls/` - Real PCAP fixtures if available

## Key Files and Context

**`crates/prb-pcap/src/tls/session.rs`**:
- Lines 80-120: 9 cipher suites defined with hex IDs

**Supported cipher suites:**
- AES-128-GCM: 0x009C, 0x009E, 0xC02F, 0xC02B, 0x1301 (TLS 1.3)
- AES-256-GCM: 0x009D, 0x009F, 0xC030, 0xC02C, 0x1302 (TLS 1.3)
- ChaCha20-Poly1305: 0x1303 (TLS 1.3), 0xCCA8, 0xCCA9 (TLS 1.2)

**Currently tested:** AES-128-GCM (0x1301), AES-256-GCM (0x1302) only ~3 of 9

**Untested:** ChaCha20-Poly1305 variants, SHA384-based cipher suites

## Implementation Approach

1. **Create test matrix** with rstest in `tests/cipher_suite_coverage_test.rs`:
   ```rust
   #[rstest]
   #[case::aes128_gcm_0x009c(0x009C, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0x009e(0x009E, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0xc02f(0xC02F, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0xc02b(0xC02B, "AES-128-GCM", 16, 12)]
   #[case::aes128_gcm_0x1301(0x1301, "AES-128-GCM-TLS13", 16, 12)]
   #[case::aes256_gcm_0x009d(0x009D, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0x009f(0x009F, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0xc030(0xC030, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0xc02c(0xC02C, "AES-256-GCM", 32, 12)]
   #[case::aes256_gcm_0x1302(0x1302, "AES-256-GCM-TLS13", 32, 12)]
   #[case::chacha20_0x1303(0x1303, "ChaCha20-Poly1305-TLS13", 32, 12)]
   #[case::chacha20_0xcca8(0xCCA8, "ChaCha20-Poly1305", 32, 12)]
   #[case::chacha20_0xcca9(0xCCA9, "ChaCha20-Poly1305", 32, 12)]
   fn test_cipher_suite(
       #[case] cipher_id: u16,
       #[case] name: &str,
       #[case] key_len: usize,
       #[case] nonce_len: usize
   ) {
       // Generate synthetic key material
       // Create minimal TLS session with this cipher suite
       // Encrypt plaintext with ring::aead
       // Decrypt with TlsDecryptor
       // Verify plaintext matches
   }
   ```

2. **Generate synthetic test data** for each cipher suite:
   - Use `ring::aead::LessSafeKey` with each algorithm
   - Create test plaintext "Hello, TLS cipher suite test!"
   - Encrypt to produce valid ciphertext + auth tag

3. **Test end-to-end** decryption for each:
   - Parse cipher suite ID from session
   - Derive keys using KDF (TLS 1.2 or 1.3 as appropriate)
   - Decrypt and verify plaintext matches

4. **Add real PCAP fixtures** (optional enhancement):
   - If available, add ChaCha20 PCAP from mobile browser capture
   - If available, add SHA384 PCAP from high-security context
   - Place in `tests/fixtures/captures/tls/` with corresponding keylog files

## Alternatives Ruled Out

- **Testing only common cipher suites (AES-128/256):** Rejected - production uses all 9 (ChaCha20 for mobile, SHA384 for high-security), must validate all
- **Property testing across cipher suites:** Rejected - adds complexity without value, explicit per-suite tests more readable

## Pre-Mortem Risks

- ChaCha20-Poly1305 might have `ring` API differences from GCM: Pin ring version 0.17 and document API usage
- Real PCAP fixtures might be hard to find: Synthetic tests sufficient for coverage, real fixtures optional enhancement

## Build and Test Commands

- Build: `cargo build -p prb-pcap --features tls`
- Test (targeted): `cargo test -p prb-pcap cipher_suite_coverage`
- Test (regression): `cargo test -p prb-pcap tls`
- Test (full gate): `cargo nextest run -p prb-pcap`

## Exit Criteria

1. **Targeted tests:**
   - `cipher_suite_coverage` - 13 cipher suite tests pass (9 unique cipher suites + 4 hex ID variants)
   - Each test validates: cipher suite parsing, key derivation, encryption, decryption, plaintext match

2. **Regression tests:** All TLS tests pass

3. **Full build gate:** `cargo build -p prb-pcap --features tls` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-pcap` passes

5. **Self-review gate:**
   - No skipped tests
   - All 9 cipher suites validated
   - ChaCha20 thoroughly tested (3 test cases for its variants)

6. **Scope verification gate:** Only test files modified, no production code changes (cipher suite mapping already exists in session.rs)
