---
id: "11"
title: "Error Propagation Strategy Undefined"
risk: 2/10
addressed_by_subsections: [1]
---

# Issue 11: Error Propagation Strategy Undefined

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
