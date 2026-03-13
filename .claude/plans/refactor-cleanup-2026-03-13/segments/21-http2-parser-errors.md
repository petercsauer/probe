---
segment: 21
title: "Improve HTTP/2 Parser Errors"
depends_on: [11]
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(grpc): Improve HTTP/2 parser error messages with context"
---

# Segment 21: Improve HTTP/2 Parser Errors

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add context to HTTP/2 parser errors (frame type, stream ID, position).

**Depends on:** Segment 11 (protocol decoders documented)

## Context: Issue 21 - Unclear HTTP/2 Parser Errors

**Core Problem:** HTTP/2 parsing errors don't include frame context. Hard to debug malformed streams.

## Scope
- **Files:** `crates/prb-grpc/src/h2.rs`

## Implementation Approach

Enhance error types with context:
```rust
#[derive(Debug, thiserror::Error)]
enum H2Error {
    #[error("Invalid frame type {frame_type} at position {position}")]
    InvalidFrame { frame_type: u8, position: usize },

    #[error("Invalid stream ID {stream_id} in {frame_type} frame")]
    InvalidStreamId { stream_id: u32, frame_type: String },

    // ...
}
```

## Build and Test Commands

**Build:** `cargo build --package prb-grpc`

**Test (targeted):** `cargo test --package prb-grpc --lib h2`

**Test (regression):** `cargo test --package prb-grpc`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** Error messages include context
2. **Regression tests:** All HTTP/2 tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** Errors are actionable and informative
6. **Scope verification:** Only h2.rs modified
