---
segment: 4
title: "TLS Key Import and Record Decryption"
depends_on: [3, 1]
risk: 8/10
complexity: High
cycle_budget: 20
status: pending
commit_message: "feat(pcap): add TLS 1.2/1.3 offline decryption with SSLKEYLOGFILE support"
---

# Segment 4: TLS Key Import and Record Decryption

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Decrypt TLS-encrypted TCP streams using SSLKEYLOGFILE key material or pcapng-embedded DSB keys, supporting TLS 1.2 and TLS 1.3 AEAD cipher suites.

**Depends on:** Segment 3 (reassembled TCP streams), Segment 1 (DSB key extraction)

## Context: Issues Addressed

**S3-4 (TLS Offline Decryption):** gRPC traffic is almost always TLS-encrypted. Without decryption, the pipeline produces opaque ciphertext. Implementation requires: key log parsing, TLS handshake tracking, key derivation (different for TLS 1.2 and 1.3), and symmetric record decryption. **Proposed fix:** Build a TLS decryption module following `pcapsql-core`'s architecture (validated reference) with `thiserror` errors. Four sub-modules: keylog (SSLKEYLOGFILE + DSB parser), session (handshake parsing, session identification), kdf (TLS 1.2 PRF, TLS 1.3 HKDF-Expand-Label), decrypt (AEAD record decryption). Use `tls-parser` for parsing, `ring` for crypto. Scope to AEAD suites only (AES-GCM, ChaCha20-Poly1305); CBC-mode deferred. **Pre-mortem risks:** TLS 1.3 HKDF-Expand-Label string encoding is fiddly ("tls13 " prefix); captures starting mid-TLS-session lack handshake; incomplete key logs cause silent failures; AES-GCM nonce construction errors cause auth failures; memory doubles for decrypted streams.

## Scope

- `prb-pcap` crate, module `tls` (with sub-modules `keylog`, `session`, `kdf`, `decrypt`)

## Key Files and Context

Reference implementation: `pcapsql-core` v0.3.1 `tls/` module (MIT) demonstrates the full architecture: `keylog.rs` (SSLKEYLOGFILE parser), `kdf.rs` (key derivation), `session.rs` (session tracking), `decrypt.rs` (record decryption). Our implementation follows the same architecture with `thiserror` errors instead of `anyhow`.

Key libraries:
- `tls-parser` v0.12.2 for TLS record/handshake parsing (`parse_tls_plaintext()`, `parse_tls_encrypted()`).
- `ring` v0.17+ for crypto: `ring::aead` (AES-128-GCM, AES-256-GCM, ChaCha20-Poly1305), `ring::hkdf` (TLS 1.3 key derivation), `ring::hmac` (TLS 1.2 PRF).

SSLKEYLOGFILE format (RFC 9850): text file with lines like `CLIENT_RANDOM <hex_client_random> <hex_master_secret>` (TLS 1.2) or `CLIENT_TRAFFIC_SECRET_0 <hex_client_random> <hex_traffic_secret>` (TLS 1.3).

TLS 1.2 key derivation: `PRF(master_secret, "key expansion", server_random + client_random)` yields `client_write_key + server_write_key + client_write_IV + server_write_IV`.

TLS 1.3 key derivation: `HKDF-Expand-Label(traffic_secret, "key", "", key_len)` and `HKDF-Expand-Label(traffic_secret, "iv", "", 12)`.

Per-record nonce: XOR of IV with 64-bit sequence number (big-endian, zero-padded to 12 bytes). AAD for AES-GCM: TLS record header (5 bytes: content_type + version + length).

When no key material is available for a session, pass through the encrypted stream with metadata flag `encrypted: true`.

## Implementation Approach

Four sub-modules:
1. `tls::keylog` -- parse SSLKEYLOGFILE, merge with DSB keys from Segment 1, store in `HashMap<[u8; 32], KeyMaterial>` keyed by client_random.
2. `tls::session` -- fed reassembled TCP bytes, uses `tls-parser` to identify TLS handshake messages, extracts client_random, server_random, cipher suite, looks up key material, initializes decryption context.
3. `tls::kdf` -- implements TLS 1.2 PRF and TLS 1.3 HKDF-Expand-Label key schedule.
4. `tls::decrypt` -- takes initialized context + encrypted TLS records, performs AEAD decryption, yields plaintext.

The module presents the same interface as a passthrough for non-TLS streams: `fn process_stream(&mut self, stream: ReassembledStream) -> DecryptedStream`.

## Alternatives Ruled Out

- Requiring plaintext captures only (unrealistic for production gRPC).
- Shelling out to tshark (external dependency, breaks self-contained design).
- Adopting pcapsql-core wholesale (anyhow in library code violates thiserror-in-libs convention).
- Using `boring` instead of `ring` (ring has better Rust-native API).
- Supporting CBC-mode cipher suites (MAC-then-encrypt complexity deferred to later phase).

## Pre-Mortem Risks

- TLS 1.3 HKDF-Expand-Label string encoding is fiddly (label prefixed with "tls13 "). Wrong label strings produce garbage silently. Build test with RFC 8448 known test vectors.
- Captures starting mid-TLS-session have no handshake -- cannot decrypt, must fall through gracefully.
- Incomplete key log files cause silent decryption failures for some sessions.
- AES-GCM nonce construction errors cause authentication tag failures on every record.
- TLS session resumption (PSK) requires `EARLY_TRAFFIC_SECRET` label which may not be in all key logs.
- Memory: decrypted streams temporarily double memory usage.

## Build and Test Commands

- Build: `cargo build -p prb-pcap`
- Test (targeted): `cargo test -p prb-pcap -- tls`
- Test (regression): `cargo test -p prb-pcap -- reader normalize tcp`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_keylog_parse`: parses SSLKEYLOGFILE with TLS 1.2 and 1.3 entries correctly
   - `test_keylog_merge_dsb`: merges DSB-extracted keys with file-based keys without duplicates
   - `test_tls12_key_derivation`: derives correct keys using RFC 5246 test vectors
   - `test_tls13_key_derivation`: derives correct keys using RFC 8448 test vectors
   - `test_aes128gcm_decrypt`: decrypts a TLS record with AES-128-GCM using known key+nonce+ciphertext
   - `test_aes256gcm_decrypt`: decrypts a TLS record with AES-256-GCM using known vectors
   - `test_chacha20poly1305_decrypt`: decrypts a TLS record with ChaCha20-Poly1305 using known vectors
   - `test_session_identification`: extracts client_random and cipher suite from TLS handshake bytes
   - `test_no_key_passthrough`: stream without matching key material passes through as `encrypted: true`
   - `test_end_to_end_tls12`: full decrypt of a captured TLS 1.2 session with known test vectors
   - `test_end_to_end_tls13`: full decrypt of a captured TLS 1.3 session with known test vectors
2. **Regression tests:** `cargo test -p prb-pcap -- reader normalize tcp`
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no secrets in test fixtures (use synthetic test vectors only).
6. **Scope verification gate:** Changes in `prb-pcap/src/tls/` and test fixtures only.
