---
id: "S3-4"
title: "TLS Offline Decryption"
risk: 8/10
addressed_by_segments: [4]
---

# Issue S3-4: TLS Offline Decryption

**Core Problem:**
gRPC traffic is almost always TLS-encrypted. Without TLS decryption, the gRPC pipeline produces opaque ciphertext. The parent plan correctly identifies SSLKEYLOGFILE as the approach but the implementation is complex: key log parsing, TLS handshake tracking, key derivation (different for TLS 1.2 and 1.3), and symmetric record decryption.

**Root Cause:**
TLS sits between TCP and application protocols, creating an opaque layer that must be decrypted before protocol decoders can function.

**Proposed Fix:**
Build a TLS decryption module following `pcapsql-core`'s architecture (validated reference implementation) but with `thiserror`-based error types:

1. **Key log parser:** Parse SSLKEYLOGFILE (RFC 9850) and pcapng DSB blocks. Support labels: `CLIENT_RANDOM` (TLS 1.2), `CLIENT_HANDSHAKE_TRAFFIC_SECRET`, `SERVER_HANDSHAKE_TRAFFIC_SECRET`, `CLIENT_TRAFFIC_SECRET_0`, `SERVER_TRAFFIC_SECRET_0` (TLS 1.3).
2. **Session identification:** Use `tls-parser` v0.12.2 to parse TLS handshake messages from the reassembled TCP stream. Extract `client_random` from ClientHello, `server_random` and cipher suite from ServerHello.
3. **Key derivation:** TLS 1.2: PRF(master_secret, "key expansion", server_random + client_random) using `ring::hmac`. TLS 1.3: HKDF-Expand-Label using `ring::hkdf` with traffic secrets from the key log.
4. **Record decryption:** Use `ring::aead` (AES-128-GCM, AES-256-GCM, ChaCha20-Poly1305). Construct per-record nonce from IV XOR sequence number (64-bit big-endian, zero-padded to 12 bytes). Validate authentication tag. AAD: TLS record header (5 bytes: content_type + version + length).
5. **Integration:** Transparent to downstream -- decrypted streams look identical to plaintext streams. When no key material is available for a session, pass through encrypted stream with metadata flag `encrypted: true`.

**Existing Solutions Evaluated:**
- `pcapsql-core` v0.3.1 TLS module (MIT) -- clean architecture: `keylog.rs` (SSLKEYLOGFILE parser), `kdf.rs` (key derivation), `session.rs` (session tracking), `decrypt.rs` (record decryption). Works. But uses `anyhow` in library code and brings heavy dependencies. Used as reference implementation, not adopted directly.
- `rustls` -- live TLS library, not usable for offline decryption. Provides HKDF implementations via `rustls::crypto::tls13` module that could be referenced.
- `tls-parser` v0.12.2 (Rusticata, 1.5M+ downloads) -- parses TLS record and handshake messages without decryption. Adopted for parsing only.
- `ring` v0.17+ (BoringSSL-derived, extensively audited) -- provides AEAD, HKDF, HMAC for all crypto operations. Adopted.
- `boring` crate -- alternative to ring with OpenSSL-compatible API. Rejected, ring has better Rust-native API and wider adoption.

**Alternatives Considered:**
- Require plaintext captures only -- rejected, unrealistic for production gRPC debugging.
- Shell out to `tshark -o ssl.keylog_file` for decryption -- rejected, adds heavyweight external dependency and breaks self-contained CLI design.
- Adopt `pcapsql-core` wholesale -- rejected, `anyhow` in library code violates project's thiserror-in-libs convention.

**Pre-Mortem -- What Could Go Wrong:**
- TLS 1.3 HKDF-Expand-Label string encoding is fiddly (label prefixed with "tls13 "). Wrong label strings produce garbage silently.
- Captures starting mid-TLS-session lack the handshake, making key matching impossible. Must fall through gracefully.
- Incomplete key log files cause silent decryption failures for some sessions.
- AES-GCM nonce construction errors cause authentication tag failures on every record.
- TLS session resumption (PSK, session tickets) requires additional key log labels not always present.
- CBC-mode cipher suites (TLS 1.2 without GCM) are significantly more complex (padding oracle concerns, MAC-then-encrypt) -- scoped to AEAD suites only for Phase 1.
- Memory: decrypted streams temporarily double memory usage since both encrypted and decrypted forms may coexist.

**Risk Factor:** 8/10

**Evidence for Optimality:**
- Existing solutions: `pcapsql-core`'s TLS module (MIT, v0.3.1) demonstrates the architecture works in Rust with `tls-parser` + `ring`, validating the approach.
- External evidence: RFC 9850 standardizes SSLKEYLOGFILE format. Wireshark's SSLKEYLOGFILE approach is the de facto standard for offline TLS decryption (wiki.wireshark.org/TLS).

**Blast Radius:**
- Direct: new TLS decryption module in `prb-pcap`
- Ripple: protocol decoders must accept both plaintext and decrypted-ciphertext byte streams identically
