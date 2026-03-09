# Universal Message Debugger -- Phase 1 Decomposition Plan

**Goal:** Decompose the Phase 1 CLI message debugger into 5 independently plannable subsections, incorporating gaps found during research verification of the original plan.
**Generated:** 2026-03-08
**Rules version:** 2026-03-08
**Entry point:** B (Enrich Existing Plan)
**Status:** Planning

---

## Overview

This plan restructures the original 12-phase Universal Message Debugger Phase 1 plan into 5 subsections based on a research-verified gap analysis. Each subsection is scoped to receive its own deep-plan for implementation-level segment decomposition. The ordering follows dependency-order (topological) because each subsection builds directly on the previous one's output types and interfaces.

Key adjustments from the original plan:
- IPC/shared memory protocols removed from Phase 1 (architecturally incoherent with offline analysis)
- TLS decryption and HPACK statefulness added to the network pipeline
- `h2` crate replaced with `h2-sans-io` for offline HTTP/2 frame parsing
- pcapng format explicitly supported alongside legacy pcap
- Schema-less protobuf decode scoped with documented limitations
- TCP reassembly delegated to an existing library rather than built from scratch
- Replay target defined as structured stdout with timing (not protocol re-emission)
- Correlation engine given per-protocol strategy definitions
- Error propagation strategy defined (thiserror in libs, anyhow in CLI binary)
- IP fragmentation, linktype handling, and VLAN stripping added to network pipeline

---

## Dependency Diagram

```
Subsection 1: Foundation & Core Model
        │
        ▼
Subsection 2: Storage & Schema Engine
        │
        ▼
Subsection 3: Network Capture Pipeline
        │
        ▼
Subsection 4: Protocol Decoders
        │
        ▼
Subsection 5: Analysis & Replay
```

Subsections are strictly sequential. Each one defines types, traits, or capabilities consumed by the next. No parallelization at the subsection level is possible because each depends on the full output of its predecessor.

Within each subsection, the subsequent deep-plan will identify parallelizable segments.

---

## Gap Analysis Summary (Issues from Research Verification)

The original plan was verified against current library documentation, crate registries, protocol specifications, and community best practices. 16 gaps were identified, categorized by severity.

### Issue 1: TLS Decryption Absent

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

---

### Issue 2: HPACK Statefulness Breaks Mid-Stream Capture

**Core Problem:**
HTTP/2 uses HPACK header compression with a stateful dynamic table built incrementally from connection start. If a PCAP starts mid-stream, the tool cannot decode HTTP/2 headers because the dynamic table context is missing. The original plan does not mention HPACK at all.

**Root Cause:**
The plan treats HTTP/2 frame parsing as stateless, but HPACK is inherently stateful and requires observing the full connection from the initial SETTINGS frame.

**Proposed Fix:**
Document the requirement for full-connection captures in gRPC mode. Implement graceful degradation: when HPACK context is missing, log a warning and fall back to payload-only analysis (protobuf bodies are not HPACK-compressed, only headers are). Add a `--hpack-tolerant` flag that substitutes raw header bytes when decompression fails instead of aborting.

**Existing Solutions Evaluated:**
- `hpack` crate (crates.io, ~100K downloads) -- HPACK encoder/decoder. Can be used for header decompression when context is available.
- `h2-sans-io` (crates.io) -- includes HPACK decompression support as part of its HTTP/2 frame codec.
- `fluke-h2-parse` (crates.io) -- nom-based HTTP/2 frame parser. Does not handle HPACK; only parses frame structure.

**Recommendation:** Use `h2-sans-io` which bundles HPACK decompression with frame parsing. Fall back to raw bytes when decompression fails.

**Alternatives Considered:**
- Require full-connection captures only. Rejected: too restrictive for real-world use where captures often start after connections are established.
- Reconstruct HPACK state heuristically. Rejected: impossible without the initial dynamic table entries.

**Pre-Mortem -- What Could Go Wrong:**
- Users expect header decoding to always work and file bugs when it doesn't.
- Graceful degradation might hide real parsing bugs (masking errors as "missing HPACK context").
- Some gRPC metadata (method names, authority) is in headers; losing it degrades correlation quality.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- External evidence: Wireshark has the same limitation and documents it explicitly (HTTP/2 dissector docs note that mid-stream captures produce "HPACK - Could Not Decode" warnings).
- Existing solutions: `h2-sans-io` provides integrated HPACK support, avoiding the need to wire up a separate HPACK library.

**Blast Radius:**
- Direct: gRPC protocol adapter
- Ripple: correlation engine (may lack method names for mid-stream captures)

---

### Issue 3: Wrong HTTP/2 Library for Offline Parsing

**Core Problem:**
The plan specifies the `h2` crate for HTTP/2 frame parsing in the gRPC adapter. `h2` is an async client/server implementation that expects to participate in a live connection. It cannot passively parse captured frames from a byte buffer.

**Root Cause:**
Library selection was based on name recognition ("h2 = HTTP/2") without evaluating whether the crate's API supports passive/offline parsing.

**Proposed Fix:**
Replace `h2` with `h2-sans-io` (synchronous, sans-I/O HTTP/2 frame codec) for the gRPC protocol adapter. `h2-sans-io` accepts raw bytes, parses frames, handles CONTINUATION assembly, and decompresses HPACK headers -- all without requiring an async runtime or active connection.

**Existing Solutions Evaluated:**
- `h2` (crates.io, 65M+ downloads) -- full async HTTP/2 client/server. Cannot parse offline captures. Rejected.
- `h2-sans-io` (crates.io) -- synchronous, no-I/O HTTP/2 codec with HPACK. Purpose-built for this use case. Adopted.
- `fluke-h2-parse` (crates.io) -- nom-based frame parser. Lighter than h2-sans-io but lacks HPACK decompression. Could be used as a fallback if h2-sans-io proves too heavy.

**Recommendation:** Adopt `h2-sans-io` as primary. Keep `fluke-h2-parse` as a noted fallback.

**Alternatives Considered:**
- Write a custom HTTP/2 frame parser. Rejected: HTTP/2 framing is well-specified but has many edge cases (CONTINUATION, padding, priority); existing libraries handle these correctly.

**Pre-Mortem -- What Could Go Wrong:**
- `h2-sans-io` is newer and less battle-tested than `h2`. May have edge-case bugs with unusual frame sequences.
- API may not expose enough control for our use case (e.g., handling malformed frames gracefully).

**Risk Factor:** 3/10

**Evidence for Optimality:**
- Existing solutions: `h2-sans-io` is explicitly designed for the sans-I/O pattern needed by offline analysis tools and WASM environments.
- External evidence: The Rust ecosystem consensus (docs.rs documentation, crate descriptions) is that `h2` is for active connections and sans-I/O variants are for passive parsing.

**Blast Radius:**
- Direct: gRPC protocol adapter dependency list
- Ripple: none (swap is contained to one crate's Cargo.toml and import paths)

---

### Issue 4: IPC/Shared Memory Protocols Incoherent with Offline Analysis

**Core Problem:**
The plan lists Iceoryx, Unix domain sockets, POSIX shared memory, and named pipes as Phase 1 protocol targets. Phase 1 is scoped to offline analysis of captured traffic, but there is no standard capture file format for IPC/shared memory traffic. You cannot open a `.pcap` of Iceoryx messages.

**Root Cause:**
The protocol coverage list was defined by "what transports exist" rather than "what transports produce capturable offline artifacts."

**Proposed Fix:**
Remove all IPC/shared memory protocols from Phase 1 scope. Move them to Phase 2, which should introduce a live capture agent that can intercept IPC traffic and serialize it into MCAP sessions. Phase 1 supports IPC data only if pre-serialized as JSON fixture files (which the fixture adapter already handles).

**Existing Solutions Evaluated:**
- Iceoryx2 has Rust bindings (eclipse-iceoryx/iceoryx2, pure Rust, actively maintained) that could support live capture in Phase 2.
- `iceoryx-rs` wraps the C++ iceoryx1 library.
- No existing tool captures IPC traffic into pcap-like files. The closest analog is `strace` for syscall-level tracing, which is too low-level.

**Recommendation:** Defer to Phase 2. Phase 1's fixture adapter already provides a path for users who can manually serialize IPC messages to JSON.

**Alternatives Considered:**
- Add a custom IPC capture format. Rejected: designing a capture format is a significant effort orthogonal to the core debugger.
- Wrap `strace`/`dtrace` to capture UDS traffic. Rejected: brittle, OS-specific, requires root, and produces syscall-level noise rather than message-level events.

**Pre-Mortem -- What Could Go Wrong:**
- Users expecting IPC support in Phase 1 are disappointed.
- Deferral creates pressure to rush IPC in Phase 2 without adequate design.

**Risk Factor:** 1/10 (removal is low-risk)

**Evidence for Optimality:**
- External evidence: No existing network analysis tool (Wireshark, tcpdump, tshark) supports shared memory capture. This is a known gap in the ecosystem.
- Existing solutions: Iceoryx2's Rust bindings exist but require live process attachment, not offline file analysis.

**Blast Radius:**
- Direct: protocol coverage list (removal)
- Ripple: none (no code exists yet for these adapters)

---

### Issue 5: pcapng Format Not Addressed

**Core Problem:**
The plan says "PCAP" throughout all phases but modern tools (Wireshark, tshark, dumpcap) default to pcapng format since Wireshark 1.8 (2012). Users will submit pcapng files. Additionally, pcapng can embed TLS key material and interface metadata, which the tool should exploit.

**Root Cause:**
The plan was written using the generic term "PCAP" without distinguishing between legacy pcap and the modern pcapng container format.

**Proposed Fix:**
Support both formats transparently. Auto-detect format from magic bytes (pcap: `0xa1b2c3d4` or `0xd4c3b2a1`; pcapng: `0x0a0d0d0a`). Use `pcap-parser` (Rusticata) as the primary parsing library, which handles both formats including pcapng's multiple-interface and multiple-section features. Extract embedded TLS keys from pcapng Decryption Secrets Blocks (DSB) when present.

**Existing Solutions Evaluated:**
- `pcap-parser` (crates.io, Rusticata project, ~20K downloads/month, last release Aug 2024) -- supports both pcap and pcapng with zero-copy parsing. Handles multiple sections, interfaces, endianness. Adopted.
- `pcap-file` (crates.io, 6.3M total downloads, last release 3+ years ago) -- older, less actively maintained. Still functional but `pcap-parser` is preferred for pcapng edge cases.

**Recommendation:** Use `pcap-parser` as the primary library. Drop `pcap-file` from the dependency list unless write support is needed (pcap-parser is read-only).

**Alternatives Considered:**
- Support only legacy pcap and ask users to convert. Rejected: poor UX; conversion loses pcapng-specific metadata.

**Pre-Mortem -- What Could Go Wrong:**
- pcapng files with multiple interfaces assign different link types per interface. Each packet must be decoded according to its interface's link type, not a global default.
- pcapng section headers can reset interface numbering. State management across sections is tricky.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- External evidence: pcapng is the IETF-specified format (draft-ietf-opsawg-pcapng) and Wireshark's default since 2012.
- Existing solutions: `pcap-parser` from the Rusticata project is purpose-built for this, handles both formats, and is actively maintained.

**Blast Radius:**
- Direct: PCAP ingest module
- Ripple: TLS decryption module (can receive embedded keys from pcapng DSB)

---

### Issue 6: Schema-less Protobuf Decode Oversold

**Core Problem:**
The plan lists "Schema-less decode" as a co-equal mode alongside schema-backed decode. Protobuf wire format has fundamental ambiguity: wire type 2 (length-delimited) is used for strings, bytes, nested messages, and packed repeated fields. Without a schema, these are indistinguishable.

**Root Cause:**
The plan does not account for the information-theoretic limitations of the protobuf wire format.

**Proposed Fix:**
Rename to "wire-format decode" and document its limitations explicitly. Implementation: parse raw wire format to extract field numbers, wire types, and raw values. For wire type 2, apply heuristic cascade: (1) try recursive sub-message parse, (2) try UTF-8 string decode, (3) fall back to hex dump. Always display field numbers, never field names. Output must clearly indicate this is best-effort.

**Existing Solutions Evaluated:**
- `protobuf-decode` (crates.io) -- attempts heuristic protobuf decoding without schemas. Small crate, limited maintenance.
- `prost-reflect` (crates.io, 0.16.3, actively maintained) -- provides `DynamicMessage` which requires a schema. Does not help for schema-less case.
- Wireshark's "Decode As... Protobuf" without schema -- applies similar heuristics to what we propose.

**Recommendation:** Build a small custom wire-format decoder. The protobuf wire format is simple (5 wire types, varint encoding). A custom implementation of ~200 lines is appropriate. Use heuristics for type 2 disambiguation.

**Alternatives Considered:**
- Remove schema-less mode entirely. Rejected: it's still useful for quick inspection even with limitations.
- Use `protobuf-decode` crate. Rejected: undermaintained; the wire format is simple enough that a custom implementation avoids a fragile dependency.

**Pre-Mortem -- What Could Go Wrong:**
- Heuristic cascade misidentifies a byte array as a sub-message, producing misleading output.
- Users mistake wire-format output for authoritative decode and file bugs about "wrong" field names.
- Recursive sub-message parsing on random binary data could produce false positives or infinite recursion.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: protobuf encoding spec (developers.google.com/protocol-buffers/docs/encoding) documents the wire type ambiguity explicitly.
- External evidence: Stack Overflow consensus (multiple high-vote answers) confirms that schema-less protobuf decode is inherently best-effort.

**Blast Radius:**
- Direct: protobuf decode engine
- Ripple: CLI output formatting (must indicate confidence level of schema-less decode)

---

### Issue 7: TCP Reassembly Underscoped

**Core Problem:**
Phase 9 describes TCP stream reassembly as one phase with three bullet points. Production-quality TCP reassembly is one of the hardest problems in network analysis: FIN/RST handling, simultaneous close, zero-window probing, segment overlap resolution, keep-alive detection, and more.

**Root Cause:**
The plan treats TCP reassembly as a simple ordered-merge problem rather than a full state machine.

**Proposed Fix:**
Use an existing TCP reassembly library rather than building from scratch. Primary candidate: `pcap_tcp_assembler` (GitHub: rus0000/pcap_tcp_assembler) -- designed specifically for PCAP log analysis, tolerates packet loss from capture tools, includes message boundary detection. Secondary candidate: extract and adapt the assembler from `smoltcp` (widely used embedded TCP stack).

**Existing Solutions Evaluated:**
- `pcap_tcp_assembler` (GitHub, MIT license) -- purpose-built for PCAP analysis. Tolerates capture-tool packet loss. Uses modified smoltcp assembler. Best fit.
- `protolens` (crates.io, v0.2.3) -- high-performance TCP reassembly (2-5 GiB/s). More capable than needed but well-maintained. Includes application-layer protocol parsers we don't need.
- `blatta-stream` (GitHub, MIT) -- thin wrapper around smoltcp assembler. Minimal but potentially too minimal.

**Recommendation:** Start with `pcap_tcp_assembler` for its PCAP-specific design. If it proves insufficient, evaluate `protolens` as a heavier but more battle-tested alternative.

**Alternatives Considered:**
- Build from scratch using the RFC 793 state machine. Rejected: months of work to reach the reliability of existing libraries; premature for Phase 1.
- Use `smoltcp` directly. Rejected: smoltcp is designed for embedded networking stacks, not passive analysis. Its assembler is useful but the full crate brings unwanted baggage.

**Pre-Mortem -- What Could Go Wrong:**
- `pcap_tcp_assembler` may not handle all edge cases (e.g., TCP timestamp options, SACK).
- Integrating a third-party assembler with our event model may require significant adapter code.
- Performance may not meet the 100k events/sec target for large captures.

**Risk Factor:** 6/10

**Evidence for Optimality:**
- Existing solutions: `pcap_tcp_assembler` is explicitly designed for the exact use case (offline PCAP TCP reassembly with tolerance for capture artifacts).
- External evidence: Production network analysis tools (Wireshark, Zeek, Suricata) all use dedicated TCP reassembly engines that took years to mature. Building from scratch is not justified for Phase 1.

**Blast Radius:**
- Direct: TCP reassembly module
- Ripple: all TCP-based protocol adapters depend on reassembled streams

---

### Issue 8: Replay Target Undefined

**Core Problem:**
Phase 12 says "replay normalized events" but never specifies where they're replayed to. "Replay" means fundamentally different things depending on the target: terminal dump with timing, protocol-faithful re-emission (requires client implementations for every protocol), or piped output for external tools.

**Root Cause:**
The replay feature was specified by analogy ("like replaying a recording") without defining the output interface.

**Proposed Fix:**
Define Phase 1 replay as structured output to stdout with original timing preserved. Events are emitted in chronological order with configurable speed multiplier (1x, 2x, 0.5x, max). Output format matches `prb inspect` output. This is useful for piping to other tools, visual debugging, and building muscle memory before Phase 2 adds protocol-level re-emission.

CLI: `prb replay session.mcap [--speed 2.0] [--filter 'transport=grpc'] [--format json|table]`

**Existing Solutions Evaluated:**
- N/A -- this is an internal design decision about output interface. No external tool solves "replay our custom event model."

**Alternatives Considered:**
- Protocol-faithful re-emission (actually send gRPC calls, ZMQ messages, etc.). Rejected for Phase 1: requires maintaining client implementations for every protocol, authentication handling, endpoint configuration. Suitable for Phase 2+.
- Write replayed events to a new MCAP file (time-filtered copy). Rejected as primary mode: useful but doesn't provide the real-time visual feedback that makes replay valuable.

**Pre-Mortem -- What Could Go Wrong:**
- Timing accuracy depends on tokio timer resolution; sub-millisecond event spacing may not replay accurately.
- High-throughput sessions (100k+ events/sec) may not be replayable in real-time due to stdout buffering.
- Users may expect protocol-level replay and be disappointed by text output.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: `tcpreplay` (the standard PCAP replay tool) started as a simple packet re-emitter before growing protocol-aware features. Starting simple is validated practice.
- External evidence: Wireshark's "Follow TCP Stream" is essentially a text-mode replay and is one of its most-used features.

**Blast Radius:**
- Direct: replay engine module
- Ripple: CLI command structure (adds `--speed`, `--filter`, `--format` flags)

---

### Issue 9: Correlation Engine Underspecified

**Core Problem:**
Phase 11 says "connect related messages" using "protocol identifiers, timestamps, correlation keys." Each protocol has completely different correlation semantics: gRPC uses HTTP/2 stream IDs, ZMQ pub/sub has no inherent request-response, DDS uses GUID prefix + entity ID matching. A single generic strategy cannot work.

**Root Cause:**
The plan treats correlation as a single algorithm when it's actually a per-protocol strategy that the core engine orchestrates.

**Proposed Fix:**
Define a `CorrelationStrategy` trait with per-protocol implementations:
- **gRPC:** correlate by (connection, HTTP/2 stream ID). Request and response share a stream. Map stream ID to method name from HEADERS frame.
- **ZMQ:** correlate by (topic, socket identity) for REQ/REP patterns. PUB/SUB has no correlation; group by topic only.
- **DDS/RTPS:** correlate by (domain ID, topic name, GUID prefix). Match DataWriter to DataReader subscriptions.
- **Generic fallback:** correlate by (source IP:port, dest IP:port, timestamp proximity).

The core engine dispatches to the appropriate strategy based on transport type detected during decode.

**Existing Solutions Evaluated:**
- N/A -- correlation logic is domain-specific to our event model. No generic library solves multi-protocol message correlation.

**Alternatives Considered:**
- Single timestamp-based correlation for all protocols. Rejected: too imprecise; concurrent messages on the same connection would be incorrectly grouped.
- User-defined correlation rules (regex on payload, header matching). Rejected for Phase 1: useful but adds significant complexity. Better as a Phase 2 feature.

**Pre-Mortem -- What Could Go Wrong:**
- gRPC stream IDs are reused after RST_STREAM; correlation must scope to a connection lifetime.
- ZMQ socket identity is optional; many deployments don't use it, breaking REQ/REP correlation.
- DDS GUID matching requires understanding RTPS SPDP/SEDP discovery protocol, which adds complexity.

**Risk Factor:** 6/10

**Evidence for Optimality:**
- External evidence: Wireshark's gRPC dissector correlates by HTTP/2 stream ID (documented in Wireshark gRPC wiki page).
- External evidence: The DDS specification (OMG DDS-RTPS v2.5) defines entity correlation through GUID prefixes, which is the canonical approach.

**Blast Radius:**
- Direct: correlation engine module
- Ripple: CLI output (flow display depends on correlation quality), protocol adapters (must emit correlation-relevant metadata)

---

### Issue 10: Missing Network Layer Handling

**Core Problem:**
The plan's PCAP pipeline jumps from "packet parsing" to "transport decoding" without addressing several network-layer concerns: (a) IP fragmentation reassembly (critical for large UDP/DDS messages), (b) pcap linktype detection (captures from different interfaces produce different link-layer frames), (c) VLAN tag and encapsulation stripping (common in enterprise captures).

**Root Cause:**
The plan models the network stack as Ethernet → IP → TCP/UDP, ignoring the real-world variations between the capture point and the transport layer.

**Proposed Fix:**
Add a network normalization layer between raw packet parsing and protocol decoding:
1. **Linktype dispatch:** Read pcap/pcapng link-layer header type. Support at minimum: Ethernet (1), Raw IP (101), Linux cooked capture SLL (113), SLL2 (276), Loopback/Null (0).
2. **VLAN stripping:** `etherparse` already handles 802.1Q tags. Expose VLAN ID as event metadata.
3. **IP fragment reassembly:** Implement a fragment reassembly buffer keyed by (src IP, dst IP, IP ID, protocol). Timeout incomplete fragments after a configurable window.

**Existing Solutions Evaluated:**
- `etherparse` (crates.io, actively maintained) -- handles Ethernet, VLAN, IPv4/IPv6, TCP, UDP. Supports "lax" parsing for truncated packets. Does not handle IP fragmentation (noted in docs as requiring allocation). Does handle 802.1Q.
- `pcap-parser` -- provides linktype from pcap/pcapng file headers but does not parse packets. Complementary to etherparse.
- No Rust crate specifically handles IP fragment reassembly for offline analysis. Must be built or adapted.

**Recommendation:** Use `etherparse` for link-through-transport parsing. Build a small IP fragment reassembly buffer (~150 lines) using a `HashMap<FragmentKey, FragmentBuffer>` with configurable timeout. Use `pcap-parser` linktype to determine the entry point for etherparse (skip Ethernet header for raw IP captures, etc.).

**Alternatives Considered:**
- Ignore IP fragmentation. Rejected: DDS/RTPS messages commonly exceed MTU and fragment; ignoring this produces corrupt protocol data.
- Use `pnet` crate for comprehensive packet handling. Rejected: `pnet` requires libpcap and is designed for live capture, not offline parsing.

**Pre-Mortem -- What Could Go Wrong:**
- Fragment reassembly buffer grows unbounded if captures contain many incomplete fragment trains.
- Unusual linktypes (e.g., USB capture, Bluetooth HCI) will cause opaque parse failures.
- VXLAN/GRE tunneling adds another encapsulation layer not handled by this fix.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- External evidence: `etherparse` docs explicitly list supported protocols and note the IP fragmentation limitation, confirming it must be handled separately.
- External evidence: Wireshark's packet dissection pipeline follows the exact same architecture: linktype dispatch → link layer → network layer (with defrag) → transport layer.

**Blast Radius:**
- Direct: PCAP ingest pipeline (new normalization layer)
- Ripple: all protocol decoders receive normalized TCP/UDP payloads instead of raw packets

---

### Issue 11: Error Propagation Strategy Undefined

**Core Problem:**
The plan lists both `thiserror` (typed library errors) and `anyhow` (erased CLI errors) but does not define which is used where across 12+ crates in the workspace.

**Root Cause:**
Error handling was listed as a dependency rather than an architectural decision.

**Proposed Fix:**
Define the convention: library crates (`core`, `storage`, `schema`, `decode`, `pcap`, `protocol-*`, `correlation`, `replay`) use `thiserror` with typed error enums. The CLI binary crate uses `anyhow` for top-level error reporting. Library crates never depend on `anyhow`. Error types are defined per-crate (not a single monolithic error enum). Cross-crate errors use `#[from]` derives for ergonomic conversion.

**Existing Solutions Evaluated:**
- N/A -- internal architectural convention. Standard Rust ecosystem practice.

**Alternatives Considered:**
- Single error enum for the whole workspace. Rejected: creates coupling between unrelated crates.
- Use `anyhow` everywhere. Rejected: library consumers lose the ability to match on specific error variants.

**Pre-Mortem -- What Could Go Wrong:**
- Developers add `anyhow` to library crates out of convenience, eroding typed error boundaries.
- Error conversion chains become deeply nested, making root cause hard to identify in error messages.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: The Rust API Guidelines (rust-lang.github.io/api-guidelines) recommend typed errors for libraries and anyhow/eyre for applications.
- External evidence: `thiserror` author (dtolnay) explicitly recommends this split in the crate's README.

**Blast Radius:**
- Direct: every crate's error types
- Ripple: none if established early; high if retrofitted

---

### Issue 12: Performance Targets Lack Methodology

**Core Problem:**
The plan states targets (100k events/sec ingest, 1M event sessions, <500ms session load) without specifying hardware baseline, measurement methodology, event size, or compression settings.

**Root Cause:**
Performance targets were stated aspirationally rather than derived from use cases or benchmarked against reference implementations.

**Proposed Fix:**
Define a benchmarking framework in Subsection 5:
- Use `criterion` for micro-benchmarks (decode latency, storage throughput).
- Define standard test scenarios: small (1K events, 100KB), medium (100K events, 50MB), large (1M events, 500MB).
- Specify a reference hardware class (e.g., "modern laptop: 8+ cores, 16GB RAM, NVMe SSD").
- Measure wall-clock time, peak RSS, and throughput.
- Targets become: "on reference hardware, the medium scenario completes ingest in <1s and loads in <500ms."

**Existing Solutions Evaluated:**
- `criterion` (crates.io, 28M+ downloads) -- standard Rust benchmarking library. Adopted.
- `divan` (crates.io) -- newer, attribute-macro-based benchmarking. Simpler API but less ecosystem adoption.

**Recommendation:** Use `criterion` for compatibility and ecosystem support.

**Alternatives Considered:**
- Skip formal benchmarks; use ad-hoc timing. Rejected: benchmarks without methodology are not reproducible.

**Pre-Mortem -- What Could Go Wrong:**
- Benchmark results vary wildly between CI and developer machines.
- Criterion's statistical model may be overkill for this project's needs.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: `criterion` is the standard Rust benchmarking library recommended by the Rust Performance Book.
- Existing solutions: MCAP's own benchmarks (visible in foxglove/mcap repo) use similar methodology with defined event sizes and hardware specs.

**Blast Radius:**
- Direct: benchmark harness (new crate or test directory)
- Ripple: CI pipeline (benchmarks should run but not gate PRs)

---

## Subsection Decomposition

### Subsection 1: Foundation & Core Model

**Scope:** Cargo workspace skeleton, core traits, canonical event model, error strategy, JSON fixture adapter, basic CLI skeleton with `prb ingest` and `prb inspect` for fixtures.

**Issues addressed:** Issue 4 (IPC removal from scope), Issue 11 (error propagation strategy)

**What this establishes:**
- The `DebugEvent` type that every later subsection produces or consumes
- Core traits: `CaptureAdapter`, `ProtocolDecoder`, `SchemaResolver`, `EventNormalizer`, `CorrelationStrategy`
- Error handling conventions enforced from day one
- A working end-to-end pipeline: JSON fixture → DebugEvent → CLI output
- Walking skeleton that validates the architecture

**Key library dependencies:**
- `tokio`, `tracing`, `tracing-subscriber`, `serde`, `serde_json`, `bytes`, `thiserror`, `anyhow` (CLI only), `camino`, `clap`
- Testing: `cargo-nextest`, `cargo-llvm-cov`, `insta`, `proptest`, `assert_cmd`, `predicates`, `tempfile`

**Original plan phases covered:** 1 (Workspace), 2 (Event Model), 3 (Fixture Adapter), 7 (CLI -- partial: ingest + inspect for fixtures only)

**Estimated complexity:** Medium
**Risk factor:** 3/10

**When to deep-plan:** First. This is the foundation. No other subsection can begin until this is complete and its trait interfaces are stable.

---

### Subsection 2: Storage & Schema Engine

**Scope:** MCAP storage layer (read/write sessions, metadata, schema storage), protobuf schema subsystem (descriptor set loading, message lookup), schema-backed protobuf decode, wire-format (schema-less) decode with documented limitations.

**Issues addressed:** Issue 6 (schema-less decode scoping)

**What this establishes:**
- Persistent session storage: events written to MCAP, readable later
- Schema registry: load `.desc` files, resolve message types by name
- Two decode paths: full decode (with schema) and wire-format decode (without)
- Extended CLI: `prb schemas`, `prb inspect` with decoded payloads

**Key library dependencies:**
- `mcap` (v0.24.0+), `prost`, `prost-types`, `prost-reflect`, `protox`

**Original plan phases covered:** 4 (Storage), 5 (Schema), 6 (Decode Engine), 7 (CLI -- extended with schemas/decode)

**Estimated complexity:** Medium
**Risk factor:** 4/10

**When to deep-plan:** After Subsection 1 is complete. Depends on stable `DebugEvent` and trait definitions.

---

### Subsection 3: Network Capture Pipeline

**Scope:** PCAP and pcapng file ingestion, linktype detection and dispatch, packet parsing (link → network → transport), VLAN stripping, IP fragment reassembly, TCP stream reassembly (using existing library), TLS session key import and record-layer decryption (AEAD cipher suites only), UDP payload extraction.

**Issues addressed:** Issue 1 (TLS decryption), Issue 5 (pcapng support), Issue 7 (TCP reassembly), Issue 10 (network layer handling)

**What this establishes:**
- A complete pipeline from raw capture file to reassembled byte streams
- TCP streams and UDP datagrams ready for protocol-specific decoding
- TLS decryption when key material is available (AEAD suites: AES-GCM, ChaCha20-Poly1305; CBC-mode deferred)
- Extended CLI: `prb ingest capture.pcapng [--tls-keylog keys.log]`

**Key library dependencies:**
- `pcap-parser` v0.17.0 (Rusticata, primary PCAP/pcapng reader with DSB support)
- `etherparse` v0.19.0+ (packet parsing, VLAN stripping, IP fragment reassembly via `defrag::IpDefragPool`)
- `tls-parser` v0.12.2 (Rusticata, TLS record/handshake parsing)
- `ring` v0.17+ (symmetric decryption: AES-GCM, ChaCha20-Poly1305; HKDF for TLS 1.3; HMAC for TLS 1.2 PRF)
- `smoltcp` v0.12+ (TCP stream reassembly via `storage::Assembler`, 12K+ GitHub stars, battle-tested)
- Reference: `pcapsql-core` v0.3.1 (MIT, validated reference architecture for TLS decryption module design)

**Research corrections applied (deep-plan verified):**
- IP fragment reassembly uses `etherparse` v0.19.0 `defrag::IpDefragPool` (not custom ~150-line implementation as originally planned)
- SLL2 (linktype 276) is NOT supported by etherparse; requires custom ~40-line parser + `pcap-parser` helpers
- TCP reassembly uses `smoltcp::storage::Assembler` (replaced `pcap_tcp_assembler` which has 0 stars and is not on crates.io)
- TLS decryption scoped to AEAD cipher suites only; CBC-mode deferred to later phase

**Original plan phases covered:** 8 (PCAP Ingest), 9 (TCP Reassembly) -- significantly expanded

**Estimated complexity:** High
**Risk factor:** 7/10

**Deep plan:** [`subsection-3-network-capture-pipeline.md`](universal-message-debugger-phase1-2026-03-08/subsection-3-network-capture-pipeline.md) -- 5 segments, confidence-first ordering
**Status:** Ready for execution

**When to deep-plan:** ~~After Subsection 2 is complete.~~ Deep plan complete. Depends on MCAP storage for persisting ingested events and schema subsystem for potential TLS certificate parsing.

**Known limitations (documented in deep plan):**
- VXLAN/GRE tunnel decapsulation not in scope
- IPv6 Jumbograms not supported by etherparse
- Compressed pcapng sections not supported by pcap-parser
- CBC-mode TLS cipher suites deferred (AEAD only in Phase 1)

---

### Subsection 4: Protocol Decoders

**Scope:** gRPC decoder (HTTP/2 frame parsing with h2-sans-io, HPACK decompression, gRPC message decompression via 5-byte LPM header, gRPC trailers/status extraction, protobuf payload extraction), ZMQ/ZMTP decoder (custom wire protocol parser -- the `zmtp` crate is dead), DDS/RTPS decoder (UDP packet inspection, RTPS message parsing, SEDP discovery tracking for topic name resolution). Protocol dispatch infrastructure (port/magic-byte detection with user override). TCP-based protocols consume reassembled streams from Subsection 3; UDP-based protocols receive extracted datagrams.

**Issues addressed:** Issue 2 (HPACK statefulness), Issue 3 (h2 library replacement), Issue 9 (correlation per-protocol metadata). Deep-plan research identified 6 additional issues: dead `zmtp` crate, h2-sans-io adoption risk (v0.1.0, 107 downloads, published 2026-02-15), missing gRPC message compression handling, missing gRPC trailers/status parsing, ZMTP mid-stream capture limitation (same class as HPACK), and DDS topic name extraction requiring SEDP discovery observation.

**What this establishes:**
- Protocol-specific decoders that transform byte streams into typed DebugEvents
- Per-protocol metadata extraction (gRPC method names and status codes, ZMQ socket types and topics, DDS domain/topic via SEDP discovery)
- Correlation-relevant fields populated in DebugEvent (stream IDs, topic names, GUIDs)
- gRPC call status (success/failure with grpc-status and grpc-message) per decoded call
- ZMTP socket type and security mechanism detection
- DDS topic name resolution when SEDP discovery data is present in capture
- Protocol dispatch: port-based and magic-byte-based auto-detection with `--protocol` user override
- Extended CLI: decoded protocol details in `prb inspect` output

**Key library dependencies:**
- `h2-sans-io` (=0.1.0, HTTP/2 frames + HPACK; fallback: `fluke-h2-parse` + `fluke-hpack`), `prost-reflect` (gRPC protobuf decode)
- `flate2` (gRPC message decompression when compressed flag is set)
- Custom ZMTP parser (~300 lines; the `zmtp` crate is dead since 2016 and must not be used)
- `rtps-parser` v0.1.1 (RTPS packet parsing; depends on `dust_dds` transitively)

**Original plan phases covered:** 10 (Protocol Adapters) -- minus IPC protocols, plus HPACK handling, gRPC compression/trailers, ZMTP custom parser, DDS discovery tracking, and library corrections

**Estimated complexity:** High
**Risk factor:** 7/10 (h2-sans-io newness is the primary driver; mitigated by documented fallback to fluke-h2-parse + fluke-hpack)

**When to deep-plan:** After Subsection 3 is complete. Depends on reassembled TCP streams and UDP datagrams as input.

**Deep plan:** See `subsection-4-protocol-decoders.md` in the plan subdirectory. Decomposed into 3 segments: (1) gRPC/HTTP2 decoder with dispatch infrastructure, (2) ZMTP decoder, (3) DDS/RTPS decoder. Segments 2 and 3 are independent and can run as parallel iterative-builder subagents after Segment 1 completes.

---

### Subsection 5: Analysis & Replay

**Scope:** Correlation engine with per-protocol strategies, full CLI command suite (`prb flows`, `prb replay`), replay engine (structured stdout with timing), performance benchmarking framework.

**Issues addressed:** Issue 8 (replay target), Issue 9 (correlation strategy), Issue 12 (performance methodology)

**What this establishes:**
- Message correlation: related events grouped into flows
- Complete CLI: all commands functional end-to-end
- Replay with timing: `prb replay session.mcap --speed 2.0`
- Benchmark suite: criterion-based, defined scenarios, reproducible

**Key library dependencies:**
- `tokio` (timers for replay), `criterion` (benchmarks)
- No new external protocol libraries

**Original plan phases covered:** 11 (Correlation), 12 (Replay) -- expanded with per-protocol strategies and benchmarks

**Estimated complexity:** Medium
**Risk factor:** 5/10

**When to deep-plan:** After Subsection 4 is complete. Depends on protocol decoders populating correlation-relevant metadata in DebugEvents.

---

## Revised Protocol Scope for Phase 1

### In scope (offline, PCAP-based):
- gRPC over HTTP/2 (TCP, with TLS decryption when key material available)
- ZMQ/ZMTP (TCP)
- DDS/RTPS (UDP)
- Raw TCP streams (generic, no protocol decoding)
- Raw UDP datagrams (generic)

### Moved to Phase 2 (requires live capture agent):
- Iceoryx shared memory
- Unix domain sockets
- POSIX shared memory
- Named pipes
- Custom protobuf pub/sub over IPC

### Moved to later phases (as originally planned):
- Kafka, MQTT, QUIC/HTTP3, eBPF capture

---

## Revised Library Dependencies

| Original Plan | Revised | Reason |
|---|---|---|
| `h2` | `h2-sans-io` | `h2` is async client/server; `h2-sans-io` is offline frame parser |
| `pcap-file` + `pcap-parser` | `pcap-parser` only | `pcap-parser` handles both formats; `pcap-file` is older and redundant for read-only use |
| (not mentioned) | `tls-parser` | TLS record parsing for decryption pipeline |
| (not mentioned) | `ring` v0.17+ | Symmetric crypto for TLS record decryption (AES-GCM, ChaCha20-Poly1305) and key derivation (HKDF, HMAC). `boring` rejected -- ring has better Rust-native API |
| (not mentioned) | `smoltcp` v0.12+ | TCP reassembly via `storage::Assembler` (12K+ stars, battle-tested). Replaces `pcap_tcp_assembler` (0 stars, not on crates.io) |
| (not mentioned) | `criterion` | Benchmarking framework |
| (not mentioned) | `rtps-parser` v0.1.1 | RTPS/DDS packet parsing (low adoption but correct API; depends on `dust_dds` transitively) |
| `zmtp` | Custom ZMTP parser | `zmtp` crate is dead (last updated 2016-06-19, 52 downloads/90d). ZMTP wire format is simple (~300 lines custom) |
| (not mentioned) | `flate2` | gRPC message decompression (compressed flag in 5-byte Length-Prefixed-Message header) |

---

## Execution Instructions

This is a two-level plan. For each subsection in order (1 through 5):

1. Enter a new deep-plan session scoped to that subsection.
2. The deep-plan produces implementation-level segments (iterative-builder handoff contracts) with exact file paths, function signatures, test cases, and build commands.
3. Switch to Agent Mode and execute segments per `orchestration-protocol.mdc`.
4. After all segments in a subsection pass, run deep-verify against that subsection's plan file.
5. If verification finds gaps, re-enter deep-plan on the unresolved items before proceeding to the next subsection.

Do not begin deep-planning a later subsection until the previous subsection's traits, types, and interfaces are implemented and stable. Later subsections depend on concrete types, not just planned interfaces.

---

## Execution Log

| Subsection | Est. Complexity | Risk | Segments | Status | Notes |
|---|---|---|---|---|---|
| 1: Foundation & Core Model | Medium | 3/10 | TBD (deep-plan pending) | -- | -- |
| 2: Storage & Schema Engine | Medium | 4/10 | TBD (deep-plan pending) | -- | -- |
| 3: Network Capture Pipeline | High | 7/10 | 5 segments | Deep plan ready | [subsection-3-network-capture-pipeline.md](universal-message-debugger-phase1-2026-03-08/subsection-3-network-capture-pipeline.md) |
| 4: Protocol Decoders | High | 7/10 | 3 segments (gRPC, ZMTP, DDS/RTPS) | Deep plan ready | [subsection-4-protocol-decoders.md](universal-message-debugger-phase1-2026-03-08/subsection-4-protocol-decoders.md) |
| 5: Analysis & Replay | Medium | 5/10 | TBD (deep-plan pending) | -- | -- |

**Deep-verify result:** --
**Follow-up plans:** --

---

## Risk Budget Assessment

Two subsections (3 and 4) are at risk 7/10. This is at the threshold of the risk budget guideline (no more than 2 at 8+). Mitigations:

- **Subsection 3** risk is driven by TLS decryption complexity. If TLS proves too complex for Phase 1, it can be deferred to a Phase 1.5 without breaking the rest of the pipeline (plaintext captures still work).
- **Subsection 4** risk is driven by protocol decoder breadth. The deep-plan for this subsection should split into one segment per protocol so that if DDS/RTPS proves intractable, gRPC and ZMQ still ship.

Both subsections have clear fallback paths that preserve overall project viability.

---

## Items Deliberately Not Addressed

These items from the original plan are confirmed correct and need no adjustment:
- MCAP as storage format (verified: well-maintained, 5.1M+ downloads, not robotics-specific)
- `prost-reflect` + `protox` for schema-backed protobuf decode (verified: actively maintained, correct API)
- `etherparse` for packet parsing (verified: zero-allocation, supports needed protocols)
- `insta` for snapshot testing, `proptest` for property testing (standard Rust choices)
- Public datasets strategy (Wireshark samples, MACCDC, MAWI -- all verified accessible)
- Platform targets: Linux + macOS
- Non-goals: GUI, TUI, live capture, remote agents
