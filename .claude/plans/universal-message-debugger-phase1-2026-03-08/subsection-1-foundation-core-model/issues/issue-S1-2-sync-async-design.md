---
id: "S1-2"
title: "Core Trait Sync/Async Design Decision Missing"
risk: 3/10
addressed_by_segments: [2]
---
# Issue S1-2: Core Trait Sync/Async Design Decision Missing

**Core Problem:**
The parent plan lists 5 core traits (CaptureAdapter, ProtocolDecoder, SchemaResolver, EventNormalizer, CorrelationStrategy) but does not address whether they should be synchronous or asynchronous. This is a foundational architectural decision because: (a) async fn in traits is stable (Rust 1.75) only for static dispatch -- trait objects (`Box<dyn Trait>`) are NOT supported, (b) the CaptureAdapter will be implemented by both sync adapters (JSON fixture) and potentially async adapters (live capture in Phase 2), and (c) choosing wrong means refactoring every implementation.

**Root Cause:**
The parent plan deferred implementation details to deep-planning. The sync/async decision was not flagged as an architectural concern.

**Proposed Fix:**
Make all 5 traits synchronous for Phase 1. Phase 1 is offline analysis only -- all I/O is file reads, which are blocking. The key trait signatures:

```rust
pub trait CaptureAdapter {
    fn name(&self) -> &str;
    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_>;
}

pub trait ProtocolDecoder {
    fn protocol(&self) -> TransportKind;
    fn decode_stream(&mut self, stream: &[u8], ctx: &DecodeContext)
        -> Result<Vec<DebugEvent>, CoreError>;
}

pub trait SchemaResolver {
    fn resolve(&self, name: &str) -> Result<Option<ResolvedSchema>, CoreError>;
    fn list_schemas(&self) -> Vec<String>;
}

pub trait EventNormalizer {
    fn normalize(&self, events: Vec<DebugEvent>) -> Result<Vec<DebugEvent>, CoreError>;
}

pub trait CorrelationStrategy {
    fn transport(&self) -> TransportKind;
    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError>;
}
```

CaptureAdapter uses `Box<dyn Iterator>` for streaming -- this is the standard Rust pattern for sync streaming and avoids loading entire captures into memory. The dynamic dispatch cost (~3ns per call) is negligible compared to I/O.

**Existing Solutions Evaluated:**
- N/A -- internal architectural decision. No external tool addresses "should our traits be async."

**Alternatives Considered:**
- Use `async-trait` crate for all traits. Rejected: adds heap allocation per trait method call, forces async runtime even for sync adapters, and Phase 1 has no async I/O needs. The `async-trait` crate is still at v0.1.x and the ecosystem is moving toward native async traits.
- Use native `async fn` in traits. Rejected: not dyn-compatible (cannot use `Box<dyn CaptureAdapter>`). While we don't need dyn dispatch for known adapters, keeping the option open is valuable for plugin architectures.
- Use `Stream` (from futures/tokio-stream) instead of `Iterator`. Rejected for Phase 1: adds async runtime dependency for no benefit. Can add `AsyncCaptureAdapter` in Phase 2 if needed.

**Pre-Mortem -- What Could Go Wrong:**
- Phase 2 live capture may require async traits, forcing a migration. Mitigation: the sync traits remain valid for offline analysis; async variants are additive, not replacements.
- `Box<dyn Iterator>` prevents the caller from knowing the concrete iterator type, which may limit optimizations. Mitigation: for hot paths, concrete types can bypass the trait.
- Large captures may block the thread during `ingest()`. Mitigation: acceptable for Phase 1 CLI; Phase 2 can use `spawn_blocking` or async adapters.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: The sans-I/O pattern (used by h2-sans-io, Python's sans-io protocol libraries) is inherently synchronous. The Rust PCAP ecosystem (pcap-parser, etherparse) is entirely sync. Matching the ecosystem avoids impedance mismatches.
- External evidence: Rust async-fn-in-traits stabilization blog post (blog.rust-lang.org, Dec 2023) explicitly recommends using sync traits when async is not needed, and adding async variants later as separate traits rather than forcing async everywhere.

**Blast Radius:**
- Direct changes: `crates/prb-core/src/traits.rs` (new file)
- Potential ripple: every adapter and decoder implementation in Subsections 2-5 implements these traits
