---
segment: 2
title: "Extract PCAP Test Helpers"
depends_on: []
risk: 2/10
complexity: Low
cycle_budget: 12
status: pending
commit_message: "refactor(pcap): Extract common PCAP test helpers to reduce duplication"
---

# Segment 2: Extract PCAP Test Helpers

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Extract 375 LOC of duplicated PCAP test utility code (write_pcap_file, create_tcp_segment, create_udp_packet) into shared helpers module.

**Depends on:** None (independent of S1)

## Context: Issue 02 - PCAP Test Helper Duplication

**Core Problem:** 375 lines of duplicated PCAP test utilities across 4 test files. Functions like `write_pcap_file()` (95-130 lines each) and `create_tcp_segment()` (11-52 lines each) are copy-pasted with minor variations.

**Proposed Fix:** Create `crates/prb-pcap/tests/helpers/mod.rs` with:
- `write_pcap_file()` - Write test PCAP to temp file
- `create_tcp_segment()` - Build TCP packet bytes
- `create_udp_packet()` - Build UDP packet bytes
- `create_ip_packet()` - Build IP header

## Scope
- **Files:** `crates/prb-pcap/tests/helpers/mod.rs` (new), migrate 4 test files

## Implementation Approach

Create helpers module following existing pattern in `crates/prb-decode/tests/helpers/`:

```rust
// crates/prb-pcap/tests/helpers/mod.rs
use std::path::Path;
use std::fs::File;
use std::io::Write;

pub fn write_pcap_file(path: &Path, packets: &[Vec<u8>]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    // Write PCAP global header
    // Write packet headers + data
    Ok(())
}

pub fn create_tcp_segment(
    src_ip: &str,
    dst_ip: &str,
    src_port: u16,
    dst_port: u16,
    data: &[u8]
) -> Vec<u8> {
    // Build IP header + TCP header + data
    vec![]
}
```

Migrate these test files:
- `crates/prb-pcap/tests/pipeline_tests.rs`
- `crates/prb-pcap/tests/tcp_reassembly_test.rs`
- `crates/prb-pcap/tests/fragment_test.rs`
- `crates/prb-pcap/tests/checksum_test.rs`

## Build and Test Commands

**Build:** `cargo build --package prb-pcap`

**Test (targeted):** `cargo test --package prb-pcap --test pipeline_tests`

**Test (regression):** `cargo test --package prb-pcap`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** Migrated test files pass with new helpers
2. **Regression tests:** All prb-pcap tests pass
3. **Full build gate:** `cargo build --workspace` succeeds
4. **Full test suite:** All workspace tests pass
5. **Self-review:** No code duplication in test files, helpers are reusable
6. **Scope verification:** Only prb-pcap/tests/ modified
