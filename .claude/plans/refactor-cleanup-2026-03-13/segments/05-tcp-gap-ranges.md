---
segment: 5
title: "Expose TCP Gap Ranges"
depends_on: [1, 2]
risk: 5/10
complexity: Medium
cycle_budget: 16
status: pending
commit_message: "fix(pcap): Expose TCP gap ranges in reassembly metadata"
---

# Segment 5: Expose TCP Gap Ranges

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Fix TCP reassembly to expose actual gap ranges instead of returning empty Vec. Users need gap information for debugging packet loss.

**Depends on:** Segments 1 (test utils), 2 (PCAP helpers)

## Context: Issue 05 - TCP Gap Ranges Not Exposed

**Core Problem:** `crates/prb-pcap/src/tcp.rs:239,448` returns `missing_ranges: Vec::new()` despite having gap information. Code comment says "TODO: Actually compute missing ranges from gaps".

**Evidence:**
```rust
// tcp.rs:239
missing_ranges: Vec::new(), // TODO: Actually compute missing ranges from gaps

// tcp.rs:448
missing_ranges: Vec::new(), // TODO: Track actual gap ranges
```

**Proposed Fix:** Compute gap ranges from `self.gaps` HashMap:
```rust
let mut missing_ranges = Vec::new();
for (start_seq, end_seq) in &self.gaps {
    missing_ranges.push((*start_seq, *end_seq));
}
missing_ranges.sort_by_key(|r| r.0);
```

## Scope
- **Files:** `crates/prb-pcap/src/tcp.rs` (lines 239, 448)

## Implementation Approach

Modify `TcpReassembler` to track and expose gap ranges:

```rust
impl TcpReassembler {
    fn get_missing_ranges(&self) -> Vec<(u32, u32)> {
        let mut ranges: Vec<(u32, u32)> = self.gaps
            .iter()
            .map(|(start, end)| (*start, *end))
            .collect();
        ranges.sort_by_key(|r| r.0);
        ranges
    }
}

// In reassembly metadata:
missing_ranges: self.get_missing_ranges(),
```

Add test using prb-test-utils and PCAP helpers from S1, S2:
```rust
#[test]
fn test_tcp_gaps_exposed() {
    let packets = vec![
        create_tcp_segment(...seq=100, len=50),  // 100-150
        create_tcp_segment(...seq=200, len=50),  // 200-250 (gap: 150-200)
    ];
    let gaps = reassemble(packets).missing_ranges;
    assert_eq!(gaps, vec![(150, 200)]);
}
```

## Build and Test Commands

**Build:** `cargo build --package prb-pcap`

**Test (targeted):** `cargo test --package prb-pcap --lib tcp::tests::test_tcp_gaps_exposed`

**Test (regression):** `cargo test --package prb-pcap --lib tcp`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** New test verifies gaps are exposed
2. **Regression tests:** All TCP tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** TODO comments removed, gap tracking complete
6. **Scope verification:** Only tcp.rs modified
