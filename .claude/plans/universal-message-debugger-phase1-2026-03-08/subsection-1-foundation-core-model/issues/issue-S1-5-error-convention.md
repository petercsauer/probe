---
id: "S1-5"
title: "Error Convention Requires Concrete Implementation"
risk: 2/10
addressed_by_segments: [1, 2]
---
# Issue S1-5: Error Convention Requires Concrete Implementation

**Core Problem:**
Parent plan Issue 11 defines the error convention (thiserror for libs, anyhow for CLI) but does not specify the concrete error types, variant names, or conversion chains. Each library crate needs its own error enum, and the CLI needs to convert all library errors into user-friendly messages.

**Root Cause:**
The parent plan defined the convention at the policy level but not at the implementation level.

**Proposed Fix:**
Define error types for the 3 crates created in Subsection 1:

```rust
// crates/prb-core/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("payload decode failed: {0}")]
    PayloadDecode(String),
    #[error("unsupported transport: {0}")]
    UnsupportedTransport(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

// crates/prb-fixture/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("fixture I/O error: {path}")]
    Io { path: String, #[source] source: std::io::Error },
    #[error("fixture parse error: {0}")]
    Parse(String),
    #[error("unsupported fixture version: {version} (supported: 1)")]
    UnsupportedVersion { version: u64 },
    #[error(transparent)]
    Core(#[from] CoreError),
}

// crates/prb-cli/src/main.rs uses anyhow::Result throughout
```

Conversion chain: `CoreError` -> `FixtureError` (via `#[from]`) -> `anyhow::Error` (via `Into` at CLI boundary).

**Existing Solutions Evaluated:**
- N/A -- internal architectural convention following standard Rust ecosystem practice.

**Alternatives Considered:**
- Single monolithic error enum for the whole workspace. Rejected per parent plan Issue 11 rationale: creates coupling between unrelated crates.
- Use `anyhow` everywhere including libraries. Rejected: library consumers lose the ability to match on specific error variants, violating the Rust API Guidelines.

**Pre-Mortem -- What Could Go Wrong:**
- Error variant names may need to change as more error conditions are discovered. Mitigation: thiserror enums are additive (new variants don't break existing matches unless using exhaustive patterns). Use `#[non_exhaustive]` on public error enums.
- Deep conversion chains (`CoreError` -> `FixtureError` -> `anyhow::Error`) may obscure the root cause in error messages. Mitigation: thiserror's `#[source]` attribute preserves the error chain; `anyhow`'s `{:#}` format prints the full chain.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: Rust API Guidelines (rust-lang.github.io/api-guidelines, C-GOOD-ERR) recommend typed errors for libraries with `#[non_exhaustive]` for public enums.
- External evidence: `thiserror` v2.0.18 README explicitly recommends this split: thiserror for libraries, anyhow for applications.

**Blast Radius:**
- Direct changes: `crates/prb-core/src/error.rs`, `crates/prb-fixture/src/error.rs`, `crates/prb-cli/src/main.rs`
- Potential ripple: every future crate follows this pattern; changing the convention after Subsection 1 is expensive
