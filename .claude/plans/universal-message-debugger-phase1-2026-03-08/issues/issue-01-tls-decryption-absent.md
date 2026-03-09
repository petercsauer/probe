---
id: "1"
title: "TLS Decryption Absent"
risk: 8/10
addressed_by_subsections: [3]
---

# Issue 1: TLS Decryption Absent

**Core Problem:**
The plan lists gRPC (HTTP/2) as a primary TCP protocol but never mentions TLS. gRPC traffic is almost always TLS-encrypted. Without TLS session key import, the entire gRPC-from-PCAP pipeline produces opaque ciphertext.

**Root Cause:**
The original plan assumes plaintext captures or does not consider the encryption layer between TCP and HTTP/2.

**Proposed Fix:**
Add SSLKEYLOGFILE / NSS Key Log Format support to the PCAP ingest pipeline. Implement a `TlsKeyLog` loader that reads key material and a `TlsDecryptor` that decrypts TLS 1.2/1.3 record-layer payloads before passing them to protocol decoders. Expose via `prb ingest capture.pcap --tls-keylog keys.log`.

**Existing Solutions Evaluated:**
- `rustls` (crates.io, 28M+ downloads, actively maintained) -- implements TLS 1.2/1.3 but as a live connection library, not a passive decryptor. Not directly usable for offline PCAP decryption.
- `tls-parser` (crates.io, part of Rusticata, 150K+ downloads) -- parses TLS record layer, handshake messages, and extensions. Pure parser, no decryption. Useful for identifying TLS sessions and extracting metadata.
- Wireshark's TLS dissector (C, GPL) -- reference implementation for SSLKEYLOGFILE-based decryption. Architecture is well-documented in Wireshark developer docs.
- `boring` / `openssl` crates -- provide raw crypto primitives (AES-GCM, ChaCha20-Poly1305) needed for actual record decryption once keys are derived.

**Recommendation:** Build a custom TLS record decryptor using `tls-parser` for record parsing + `ring` or `boring` for symmetric decryption. Key derivation follows RFC 8446 (TLS 1.3) and RFC 5246 (TLS 1.2) key schedule. This is the same approach Wireshark uses internally.

**Alternatives Considered:**
- Require users to capture plaintext traffic only. Rejected: unrealistic for production gRPC debugging.
- Shell out to `tshark -o ssl.keylog_file` for decryption. Rejected: adds a heavyweight external dependency and breaks the self-contained CLI design.

**Pre-Mortem -- What Could Go Wrong:**
- TLS 1.3 key schedule is complex; incorrect key derivation silently produces garbage plaintext.
- Captures that start mid-session lack the handshake; without the handshake, key log entries cannot be matched to sessions.
- Key log files may be incomplete (missing entries for some sessions).
- Performance: decrypting every TLS record adds CPU overhead proportional to capture size.

**Risk Factor:** 8/10

**Evidence for Optimality:**
- External evidence: Wireshark's SSLKEYLOGFILE approach is the de facto standard for offline TLS decryption (documented at wiki.wireshark.org/TLS).
- Existing solutions: `tls-parser` from the Rusticata project provides battle-tested TLS record parsing in Rust, avoiding the need to hand-roll ASN.1/TLS parsing.

**Blast Radius:**
- Direct: network capture pipeline (new TLS decryption module)
- Ripple: protocol decoders must accept both plaintext and decrypted-ciphertext byte streams identically
