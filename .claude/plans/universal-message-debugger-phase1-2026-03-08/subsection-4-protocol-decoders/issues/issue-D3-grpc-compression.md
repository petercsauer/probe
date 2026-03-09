---
id: "D3"
title: "gRPC Compression Handling Missing"
risk: 3/10
addressed_by_segments: [1]
---
# Issue D3: gRPC Compression Handling Missing

**Core Problem:**
gRPC messages include a 5-byte Length-Prefixed-Message (LPM) header with a compression flag byte. When this flag is 1, the message payload is compressed using the algorithm specified in the `grpc-encoding` HTTP/2 header (typically gzip). The parent plan does not mention message-level compression at all.

**Root Cause:**
The plan focused on HTTP/2 framing and protobuf extraction without examining the gRPC message envelope format in detail.

**Proposed Fix:**
After extracting bytes from HTTP/2 DATA frames, parse the 5-byte LPM header:
```
compressed_flag: u8   // 0 = no compression, 1 = compressed
message_length:  u32  // big-endian network byte order
message:         [u8; message_length]
```
If `compressed_flag == 1`, read `grpc-encoding` from HEADERS and decompress using the appropriate algorithm. Support `gzip` (primary), `deflate`, and `identity`. Use `flate2` crate for gzip/deflate decompression.

**Existing Solutions Evaluated:**
- `flate2` (crates.io, 80M+ downloads, actively maintained) -- Standard Rust gzip/deflate library. **Adopted.**
- The LPM parsing itself is 5 bytes of trivial format; no library needed.

**Alternatives Considered:**
- Ignore compression and fail with an error message. Rejected: gRPC compression is common in production; ignoring it makes the tool useless for many real captures.

**Pre-Mortem -- What Could Go Wrong:**
- gRPC messages may span multiple HTTP/2 DATA frames. The LPM header and compressed payload must be reassembled across frame boundaries.
- Unusual compression algorithms (snappy, zstd) are not handled by flate2. These are rare but possible.
- Decompression of large messages could spike memory usage.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: gRPC spec (PROTOCOL-HTTP2.md at grpc/grpc) defines the exact 5-byte LPM format and compression semantics.
- Existing solutions: `flate2` is the standard Rust compression library with 80M+ downloads.

**Blast Radius:**
- Direct: gRPC decoder (LPM parsing + decompression step)
- Ripple: Cargo.toml (add `flate2` dependency)
