---
segment: 13
title: API Documentation
depends_on: []
risk: 4
complexity: Medium
cycle_budget: 8
estimated_lines: All crates modified
---

# Segment 13: API Documentation & Architecture Decision Records

## Context

Complete API documentation for all public interfaces and create Architecture Decision Records (ADRs) documenting key design choices. Google-level codebases have 100% documentation coverage for public APIs.

## Current State

- Documentation exists but incomplete
- `cargo doc --workspace` shows 2 warnings (unclosed HTML tags)
- No `#![warn(missing_docs)]` lint enforced
- No Architecture Decision Records
- Unknown percentage of documented public APIs

## Goal

Achieve 100% documentation coverage for public APIs, fix all rustdoc warnings, and document key architectural decisions.

## Exit Criteria

1. [ ] All crates have `#![warn(missing_docs)]` in lib.rs
2. [ ] All public functions, structs, enums, traits documented
3. [ ] `cargo doc --workspace` builds without warnings
4. [ ] Code examples in docs tested via doctests
5. [ ] Architecture Decision Records created for key decisions
6. [ ] `docs/architecture.md` updated with current state
7. [ ] Manual review: Documentation is clear and helpful
8. [ ] CI doc job passes (from S03)

## Implementation Plan

### Step 1: Enable missing_docs Lint

Add to each crate's `src/lib.rs`:

```rust
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
```

This will produce warnings for any undocumented public items.

### Step 2: Fix Existing Rustdoc Warnings

From S01 analysis, fix:
- Unclosed HTML tags in prb-pcap (`<PacketLocation>`, `<TlsKeyLog>`)

### Step 3: Document prb-core (Priority 1)

Target: ~93 public items

Example documentation additions:

```rust
/// Core event type representing a decoded protocol message.
///
/// A `DebugEvent` captures all metadata about a network event including
/// timestamp, source/destination, transport protocol, and decoded payload.
///
/// # Examples
///
/// ```
/// use prb_core::{DebugEvent, EventId, Timestamp};
///
/// let event = DebugEvent::builder()
///     .id(EventId::from_raw(1))
///     .timestamp(Timestamp::now())
///     .build();
/// ```
pub struct DebugEvent {
    // ...
}

/// Unique identifier for a debug event.
///
/// Event IDs are monotonically increasing within a session but may not
/// be sequential due to parallel processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(u64);

impl EventId {
    /// Create an EventId from a raw u64 value.
    ///
    /// # Examples
    ///
    /// ```
    /// use prb_core::EventId;
    ///
    /// let id = EventId::from_raw(42);
    /// assert_eq!(id.as_u64(), 42);
    /// ```
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value of this EventId.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Protocol decoder trait for implementing custom protocol support.
///
/// Implement this trait to add support for decoding new protocols.
///
/// # Examples
///
/// ```
/// use prb_core::{ProtocolDecoder, DebugEvent};
///
/// struct MyDecoder;
///
/// impl ProtocolDecoder for MyDecoder {
///     fn protocol_name(&self) -> &str {
///         "my-protocol"
///     }
///
///     fn decode(&self, data: &[u8]) -> Result<DebugEvent, Box<dyn std::error::Error>> {
///         // Decode logic here
///         todo!()
///     }
/// }
/// ```
pub trait ProtocolDecoder {
    /// Returns the name of the protocol this decoder handles.
    fn protocol_name(&self) -> &str;

    /// Decode raw bytes into a DebugEvent.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed or cannot be decoded.
    fn decode(&self, data: &[u8]) -> Result<DebugEvent, Box<dyn std::error::Error>>;
}
```

### Step 4: Document Protocol Decoders

For prb-grpc, prb-zmq, prb-dds:

```rust
/// gRPC protocol decoder.
///
/// Decodes gRPC messages over HTTP/2. Supports:
/// - Unary RPCs
/// - Streaming RPCs (client, server, bidirectional)
/// - gRPC status codes and metadata
///
/// # Examples
///
/// ```no_run
/// use prb_grpc::GrpcDecoder;
/// use prb_core::ProtocolDecoder;
///
/// let decoder = GrpcDecoder::new();
/// let event = decoder.decode(&grpc_bytes)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct GrpcDecoder {
    // ...
}
```

### Step 5: Document Storage & Export

For prb-storage, prb-export:

```rust
/// MCAP session writer for persisting debug events.
///
/// Writes events to MCAP (MessagePack Archive) format for efficient
/// storage and replay.
///
/// # Examples
///
/// ```no_run
/// use prb_storage::SessionWriter;
/// use std::path::Path;
///
/// let mut writer = SessionWriter::new(Path::new("capture.mcap"))?;
/// writer.write_event(&event)?;
/// writer.flush()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SessionWriter {
    // ...
}
```

### Step 6: Create Architecture Decision Records

Create `/Users/psauer/probe/docs/adr/` directory with ADRs:

**ADR-0001: Workspace Structure**
`docs/adr/0001-workspace-structure.md`:

```markdown
# ADR 0001: Cargo Workspace Structure

## Status

Accepted

## Context

PRB needs to support multiple protocol decoders (gRPC, ZMQ, DDS) with a clean
separation between core types, protocol-specific logic, and user interfaces.

## Decision

Use a Cargo workspace with 21 crates organized by responsibility:

- **Core**: prb-core (types, traits), prb-storage (persistence)
- **Ingestion**: prb-pcap (packet parsing), prb-capture (live capture), prb-fixture (test data)
- **Protocols**: prb-grpc, prb-zmq, prb-dds (protocol decoders)
- **Detection**: prb-detect (auto-detection), prb-schema, prb-decode (schema-based decoding)
- **Output**: prb-tui (terminal UI), prb-export (file export), prb-query (filtering)
- **Extensibility**: prb-plugin-api, prb-plugin-native, prb-plugin-wasm
- **Experimental**: prb-ai (LLM explanations)
- **CLI**: prb-cli (command-line interface)

## Consequences

**Positive:**
- Clear separation of concerns
- Independent versioning possible
- Parallel compilation
- Easy to add new protocols
- Optional dependencies for specific features

**Negative:**
- More Cargo.toml files to maintain
- Longer compile times for workspace
- More complex dependency graph

## Alternatives Considered

- Monolithic single-crate design: Rejected (poor modularity)
- Separate repositories: Rejected (harder to coordinate changes)
```

**ADR-0002: Error Handling Strategy**
`docs/adr/0002-error-handling.md`:

```markdown
# ADR 0002: Error Handling Strategy

## Status

Accepted

## Context

PRB parses untrusted network data which can be malformed. Need consistent
error handling across all decoders and layers.

## Decision

1. Use `thiserror` for library error types
2. Use `anyhow` for application errors in CLI
3. All decoders return `Result<T, CoreError>` for cross-crate boundaries
4. Non-fatal warnings stored in `DebugEvent::warnings` field
5. Never panic on malformed input (use Result instead)

## Consequences

**Positive:**
- Consistent error handling across crates
- Rich error context for debugging
- Warnings visible in output without failing
- Safe against malformed packets

**Negative:**
- More verbose than unwrap/expect
- Some error paths need testing
```

**ADR-0003: Protocol Detection Engine**
`docs/adr/0003-protocol-detection.md`:

**ADR-0004: Plugin Architecture**
`docs/adr/0004-plugin-architecture.md`:

### Step 7: Update Architecture Doc

Update `/Users/psauer/probe/docs/architecture.md` with:
- Current crate count and structure
- Data flow diagrams
- Key abstractions (DebugEvent, ProtocolDecoder, etc.)
- Reference to ADRs for design decisions

## Files to Modify

All 21 crate `src/lib.rs` files (~1 line each for lint)
Plus documentation additions:

- `prb-core/src/*.rs` (~200 doc lines)
- `prb-grpc/src/*.rs` (~100 doc lines)
- `prb-zmq/src/*.rs` (~80 doc lines)
- `prb-dds/src/*.rs` (~80 doc lines)
- `prb-storage/src/*.rs` (~60 doc lines)
- `prb-export/src/*.rs` (~60 doc lines)
- `prb-query/src/*.rs` (~50 doc lines)
- Other crates (~200 doc lines total)

ADRs:
- `docs/adr/0001-workspace-structure.md`
- `docs/adr/0002-error-handling.md`
- `docs/adr/0003-protocol-detection.md`
- `docs/adr/0004-plugin-architecture.md`

## Test Plan

1. Add `#![warn(missing_docs)]` to all lib.rs files
2. Run `cargo build --workspace` to see missing doc warnings
3. Document public APIs systematically by crate
4. Run `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
   - Should build without warnings
5. Test doctests:
   ```bash
   cargo test --doc --workspace
   ```
6. Create ADR directory and files
7. Update architecture.md
8. Review documentation with team
9. Commit: "docs: Complete API documentation and ADRs"

## Blocked By

None - documentation is independent work.

## Blocks

None - final polish segment.

## Success Metrics

- All public APIs documented
- Zero rustdoc warnings
- Doctests pass
- ADRs created for 4 key decisions
- Architecture doc updated
- CI doc job passes

## Notes

- This is the largest documentation effort
- Focus on public APIs first, internal items later
- Doctests should be compilable examples
- ADRs document "why" not "what" (code shows what)
- Use `#[doc(hidden)]` for truly internal public items
- Consider adding diagrams to architecture.md (mermaid)
- Documentation is a living artifact - update as code changes
