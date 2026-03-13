---
segment: 10
title: "Document Core APIs"
depends_on: [3, 4, 5, 6, 7, 8, 9]
risk: 2/10
complexity: Medium
cycle_budget: 18
status: pending
commit_message: "docs(core): Add comprehensive API documentation with examples"
---

# Segment 10: Document Core APIs

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove `#![allow(missing_docs)]` from prb-core and add comprehensive documentation with examples to all 93 public API items.

**Depends on:** Segments 3-9 (bug fixes complete, APIs stable for documentation)

## Context: Issue 10 - Missing API Documentation

**Core Problem:** prb-core has `#![allow(missing_docs)]` suppressing warnings. The public API (DebugEvent, ProtocolDecoder trait, CaptureAdapter trait, etc.) lacks doc comments and examples.

**Proposed Fix:**
1. Remove `#![allow(missing_docs)]` from `lib.rs`
2. Add module-level docs (`//!`) with crate overview
3. Add item-level docs (`///`) with examples for all public items
4. Add doctests (compilable code examples)

## Scope
- **Crate:** prb-core only (93 public items)
- **Files:** `src/lib.rs`, `src/event.rs`, `src/decode.rs`, `src/capture.rs`, `src/error.rs`

## Implementation Approach

### Step 1: Update lib.rs module docs
```rust
//! # prb-core
//!
//! The foundational crate for the probe network debugging toolkit.
//!
//! This crate provides core types and traits used throughout the probe ecosystem:
//! - [`DebugEvent`]: The universal event type representing protocol messages
//! - [`ProtocolDecoder`]: Trait for implementing protocol decoders
//! - [`CaptureAdapter`]: Trait for packet capture sources
//!
//! # Examples
//! ```
//! use prb_core::{DebugEvent, DebugEventBuilder, TransportKind};
//! let event = DebugEventBuilder::new()
//!     .transport(TransportKind::Grpc)
//!     .build();
//! ```
```

### Step 2: Document DebugEvent
```rust
/// A protocol message captured from network traffic.
///
/// `DebugEvent` is the universal event type used throughout probe.
/// All adapters (pcap, fixture, etc.) produce `DebugEvent`s.
///
/// # Examples
/// ```
/// use prb_core::DebugEvent;
/// let event = DebugEvent::builder().build();
/// assert_eq!(event.warnings.len(), 0);
/// ```
pub struct DebugEvent { /* ... */ }
```

### Step 3: Document trait methods
Add `# Examples` sections to all public trait methods (ProtocolDecoder::decode, CaptureAdapter::ingest, etc.)

### Step 4: Remove #![allow(missing_docs)]
Delete line from `src/lib.rs`, exposing any remaining undocumented items as warnings.

## Build and Test Commands

**Build:** `cargo doc --package prb-core --no-deps`

**Test (targeted):** `cargo test --package prb-core --doc` (runs doctests)

**Test (regression):** `cargo test --package prb-core`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** All doctests compile and pass (`cargo test --doc -p prb-core`)
2. **Regression tests:** prb-core tests pass (documentation doesn't affect behavior)
3. **Full build gate:** `cargo doc --workspace` builds without warnings
4. **Full test suite:** All workspace tests pass
5. **Self-review:** No remaining `#![allow(missing_docs)]`, all pub items documented
6. **Scope verification:** Only prb-core/src/*.rs modified (no other crates)
