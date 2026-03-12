---
segment: 11
title: TUI panes (high-value) to 75%
depends_on: [5, 6, 7]
risk: 4
complexity: High
cycle_budget: 12
estimated_lines: ~350 test lines
---

# Segment 11: TUI Panes High-Value Coverage to 75%

## Context

**Target panes:**
- `hex_dump.rs` - 59.73% → 75% (260 lines uncovered)
- `decode_tree.rs` - 42.38% → 70% (468 lines uncovered)
- `timeline.rs` - 45.43% → 70% (239 lines uncovered)

**Already excellent:**
- `event_list.rs` - 95.26% ✅ (keep)

## Goal

Test data transformation logic in visualization panes.

## Implementation Plan

### Priority 1: Hex Dump Logic (~150 lines)

```rust
// crates/prb-tui/tests/hex_dump_logic_tests.rs

#[test]
fn test_hex_dump_byte_range_selection() {
    let data = vec![0u8; 1000];
    let hex_dump = HexDump::new(&data);
    let visible = hex_dump.visible_range(offset: 100, height: 20);
    assert_eq!(visible.len(), 20 * 16); // 20 rows * 16 bytes
}

#[test]
fn test_hex_dump_ascii_representation() {
    let data = b"Hello\x00World\xFF";
    let hex_dump = HexDump::new(data);
    let ascii = hex_dump.ascii_column(0);
    assert_eq!(ascii, "Hello.World.");
}
```

### Priority 2: Decode Tree Navigation (~120 lines)

Test tree expansion, node selection, path traversal.

### Priority 3: Timeline Calculations (~80 lines)

Test time range calculations, zoom levels, timestamp formatting.

## Success Metrics

- hex_dump: 59.73% → 75%+
- decode_tree: 42.38% → 70%+
- timeline: 45.43% → 70%+
- ~50 new tests
