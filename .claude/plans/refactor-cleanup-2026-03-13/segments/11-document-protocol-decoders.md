---
segment: 11
title: "Document Protocol Decoders"
depends_on: [3]
risk: 2/10
complexity: Low
cycle_budget: 12
status: pending
commit_message: "docs(protocols): Add comprehensive documentation to decoder crates"
---

# Segment 11: Document Protocol Decoders

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Remove `#![allow(missing_docs)]` and document prb-grpc, prb-zmq, prb-dds decoder APIs.

**Depends on:** Segment 3 (decoder refactoring complete)

## Context: Issue 11 - Missing Protocol Decoder Docs

**Core Problem:** Protocol decoder crates have basic crate-level docs but `#![allow(missing_docs)]` suppresses API documentation warnings.

## Scope
- **Crates:** prb-grpc, prb-zmq, prb-dds

## Implementation Approach

For each crate:
1. Remove `#![allow(missing_docs)]`
2. Add module-level docs with protocol overview
3. Add struct/enum docs with examples
4. Add method docs

Example:
```rust
//! # prb-grpc
//!
//! gRPC protocol decoder for probe.
//!
//! # Example
//! ```
//! use prb_grpc::GrpcDecoder;
//! let decoder = GrpcDecoder::new();
//! ```

/// Decodes gRPC messages from HTTP/2 streams.
pub struct GrpcDecoder { /* ... */ }
```

## Build and Test Commands

**Build:** `cargo doc --package prb-grpc --package prb-zmq --package prb-dds --no-deps`

**Test (targeted):** `cargo test --doc -p prb-grpc -p prb-zmq -p prb-dds`

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** All doctests pass
2. **Regression tests:** All tests pass
3. **Full build gate:** `cargo doc --workspace` succeeds without warnings
4. **Full test suite:** All tests pass
5. **Self-review:** No `#![allow(missing_docs)]`, all public APIs documented
6. **Scope verification:** Only protocol decoder crates modified
