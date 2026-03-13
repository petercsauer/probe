---
segment: 2
title: "TLS Decryption RFC and Wycheproof Vectors"
depends_on: [1]
risk: 9/10
complexity: High
cycle_budget: 20
status: merged
commit_message: "test(pcap-tls): Add RFC and Wycheproof test vectors for AEAD decryption"
---

# Segment 2: TLS Decryption RFC and Wycheproof Vectors

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Increase decrypt.rs coverage from 25.58% to 75%+ using authoritative RFC and Wycheproof test vectors.

**Depends on:** Segment 1 (fuzzing infrastructure can be reused)

## Context: Issues Addressed

**Core Problem:** TLS decryption has cryptographic correctness issues with only 25.58% coverage. Lines 96-97 silently create zero-keys `(vec![0u8; len], vec![0u8; 12])` instead of returning an error - a serious security issue. Only AES-128/256-GCM tested, missing ChaCha20-Poly1305 and SHA384 variants. Multi-record streams with sequence number progression are untested.

**Proposed Fix:** Replace silent failure with proper error, add RFC test vectors (RFC 5869 HKDF, RFC 8446 TLS 1.3, RFC 5246 TLS 1.2), add Wycheproof AEAD test vectors (~500 cases), test multi-record streams with sequence progression, add property tests for encrypt/decrypt roundtrips.

**Pre-Mortem Risks:**
- Wycheproof vectors might expose bugs in `ring` library - document as external issue, report upstream
- 500+ test cases could slow CI - mitigate with nextest parallelization (already in workspace)
- RFC vectors might not match implementation due to spec ambiguity - cross-reference with rustls behavior

## Scope

- `crates/prb-pcap/src/tls/decrypt.rs` (404 lines)
- `crates/prb-pcap/src/tls/kdf.rs` (261 lines)
- `crates/prb-pcap/tests/rfc_vectors/` - New RFC test vectors
- `crates/prb-pcap/tests/fixtures/wycheproof/` - New Wycheproof vectors
- `crates/prb-pcap/tests/tls_multi_record_test.rs` - New multi-record tests
- `crates/prb-pcap/tests/tls_property_test.rs` - New property tests

## Key Files and Context

**`crates/prb-pcap/src/tls/decrypt.rs`** (404 lines):
- Lines 96-97: Silent zero-key creation instead of error - security issue
- Lines 238-325: AEAD decryption with nonce/AAD construction for TLS 1.2 and 1.3
- Lines 284-286: Nonce XOR assumes 8-byte boundary for TLS 1.2 explicit nonce
- Uses `ring::aead` for AES-GCM and ChaCha20-Poly1305

**`crates/prb-pcap/src/tls/kdf.rs`** (261 lines):
- TLS 1.2 PRF using HMAC-based pseudo-random function
- TLS 1.3 HKDF-Expand-Label for key derivation
- SHA256 and SHA384 variants

**`crates/prb-pcap/tests/tls_tests.rs`**:
- Existing synthetic encrypt/decrypt helpers using `ring::aead::SealingKey`
- Currently only AES-128-GCM and AES-256-GCM tested
- Multi-record streams with sequence number progression untested

## Implementation Approach

1. **Fix silent failure** at lines 96-97:
   ```rust
   return Err(TlsError::MissingKeyMaterial(format!(
       "No {} key material found for client_random",
       if direction == Direction::ClientToServer { "client" } else { "server" }
   )));
   ```

2. **Add RFC test vectors** in `tests/rfc_vectors/`:
   - RFC 5869 Appendix A: HKDF test vectors (7 cases covering SHA256/SHA384, various IKM/salt/info lengths)
     - Create `tests/rfc_vectors/rfc5869_hkdf.json` with test cases
   - RFC 8446 Appendix B: TLS 1.3 key derivation (3 cases for traffic secrets)
     - Create `tests/rfc_vectors/rfc8446_tls13.json`
   - RFC 5246 Appendix A.5: TLS 1.2 PRF (2 cases)
     - Create `tests/rfc_vectors/rfc5246_tls12.json`
   - Create parser in `tests/rfc_vectors/mod.rs` to load and execute JSON test vectors

3. **Add Wycheproof AEAD vectors**:
   - Download from github.com/google/wycheproof:
     - `aes_gcm_test.json` (~300 test cases)
     - `chacha20_poly1305_test.json` (~200 test cases)
   - Place in `tests/fixtures/wycheproof/`
   - Create `tests/wycheproof_runner.rs` to execute vectors
   - Test cases cover: valid encryption, invalid auth tags, edge case nonce/AAD lengths, zero-length messages

4. **Add multi-record tests** in `tests/tls_multi_record_test.rs`:
   - Test sequence number progression (0, 1, 2, ..., 100)
   - Test sequence wrap at 2^64-1 (implementation should handle or error)
   - Test record boundaries: multiple records in single stream, partial records
   - Test out-of-order records (should fail authentication)

5. **Property tests** with proptest in `tests/tls_property_test.rs`:
   ```rust
   proptest! {
       #[test]
       fn encrypt_decrypt_roundtrip(
           key in prop::collection::vec(any::<u8>(), 16..=32),
           nonce in prop::collection::vec(any::<u8>(), 12),
           plaintext in prop::collection::vec(any::<u8>(), 0..1000)
       ) {
           let ciphertext = encrypt(&key, &nonce, &plaintext);
           let decrypted = decrypt(&key, &nonce, &ciphertext);
           assert_eq!(decrypted, plaintext);
       }
   }
   ```

## Alternatives Ruled Out

- **Only RFC vectors without Wycheproof:** Rejected - misses adversarial edge cases like tampered auth tags
- **Only synthetic tests without official vectors:** Rejected - doesn't validate standards compliance with authoritative sources

## Pre-Mortem Risks

- Wycheproof vectors might expose bugs in `ring` library: Document as external issue, report to rust-crypto/ring project
- 500+ test cases could slow CI: Mitigate with nextest parallelization (already in workspace)
- RFC vectors might not match implementation due to spec ambiguity: Cross-reference with rustls behavior as canonical implementation

## Build and Test Commands

- Build: `cargo build -p prb-pcap --features tls`
- Test (targeted): `cargo test -p prb-pcap rfc_vectors wycheproof multi_record tls_property`
- Test (regression): `cargo test -p prb-pcap tls`
- Test (full gate): `cargo nextest run -p prb-pcap`

## Exit Criteria

1. **Targeted tests:**
   - `rfc_vectors` - 12 RFC cases pass (7 HKDF + 3 TLS 1.3 + 2 TLS 1.2)
   - `wycheproof_aead` - 500+ cases pass (300 AES-GCM + 200 ChaCha20)
   - `multi_record_sequence` - sequence progression tests pass
   - `tls_property` - proptest passes (100+ generated roundtrips)

2. **Regression tests:** All existing TLS tests pass including `tests/tls_tests.rs`, `tests/tls_decrypt_edge_tests.rs`

3. **Full build gate:** `cargo build -p prb-pcap --features tls` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-pcap` passes

5. **Self-review gate:**
   - Silent failure at lines 96-97 replaced with error
   - No test-only code in production paths
   - Wycheproof JSON documented in tests/fixtures/README or inline comments

6. **Scope verification gate:** Only modified:
   - `decrypt.rs` - error handling fix
   - `kdf.rs` - if needed for test support
   - Test files in `tests/` directory
   - Added `tests/rfc_vectors/`, `tests/fixtures/wycheproof/`
