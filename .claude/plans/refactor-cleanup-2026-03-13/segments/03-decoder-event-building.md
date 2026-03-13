---
segment: 3
title: "Refactor Decoder Event Building"
depends_on: []
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "refactor(core): Add DecodeContext::create_event_builder helper method"
---

# Segment 3: Refactor Decoder Event Building

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add `DecodeContext::create_event_builder()` helper to eliminate 75 LOC of duplicated event building logic across protocol decoders (prb-grpc, prb-zmq, prb-dds).

**Depends on:** None (independent)

## Context: Issue 03 - Decoder Event Building Duplication

**Core Problem:** Protocol decoders (grpc, zmq, dds) have 20-25 line event building blocks that are 90% identical. Only protocol-specific fields differ. Duplication: ~75 LOC total.

**Proposed Fix:** Add helper method to `DecodeContext` in prb-core:

```rust
// crates/prb-core/src/decode.rs
impl DecodeContext {
    pub fn create_event_builder(&self, transport: TransportKind) -> DebugEventBuilder {
        let mut builder = DebugEventBuilder::new()
            .transport(transport);

        if let Some(ref src) = self.src_addr {
            if let Some(ref dst) = self.dst_addr {
                builder = builder.source(EventSource {
                    adapter: "decoder".into(),
                    origin: format!("{}", transport),
                    network: Some(NetworkAddr {
                        src: src.clone(),
                        dst: dst.clone(),
                    }),
                });
            }
        }

        if let Some(ts) = self.timestamp {
            builder = builder.timestamp(ts);
        }

        builder
    }
}
```

Then refactor decoders to use it:
```rust
// Before (25 lines):
let event = DebugEvent {
    id: EventId::generate(),
    timestamp: ctx.timestamp.unwrap_or_else(Timestamp::now),
    source: EventSource { /* ... 10 lines ... */ },
    transport: TransportKind::Grpc,
    // ... rest of fields
};

// After (5 lines):
let event = ctx.create_event_builder(TransportKind::Grpc)
    .direction(Direction::Request)
    .payload(payload)
    .build();
```

## Scope
- **Modified:** `crates/prb-core/src/decode.rs` (~30 lines)
- **Refactored:** `crates/prb-grpc/src/decoder.rs`, `crates/prb-zmq/src/decoder.rs`, `crates/prb-dds/src/decoder.rs`

## Build and Test Commands

**Build:** `cargo build --package prb-core --package prb-grpc --package prb-zmq --package prb-dds`

**Test (targeted):**
```bash
cargo test --package prb-core --lib decode
cargo test --package prb-grpc --lib decoder
cargo test --package prb-zmq --lib decoder
cargo test --package prb-dds --lib decoder
```

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** All decoder tests pass with refactored event building
2. **Regression tests:** No behavior changes (events identical to before)
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** Event building centralized in DecodeContext helper
6. **Scope verification:** Only prb-core/decode.rs and 3 decoder files modified
