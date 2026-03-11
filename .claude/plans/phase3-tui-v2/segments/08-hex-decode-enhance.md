---
segment: 08
title: Hex Dump & Decode Tree Enhancements
depends: [05]
risk: 4
complexity: Medium
cycle_budget: 7
estimated_lines: 450
---

# Segment 08: Hex Dump & Decode Tree Enhancements

## Context

Hex dump and decode tree are basic. Need search, diff highlighting, expand-all, and better navigation.

## Goal

Add search in hex dump, diff mode highlighting, expand-all in decode tree, and improved keyboard navigation.

## Exit Criteria

1. [ ] Hex dump search with `/` key
2. [ ] Highlight search matches in hex and ASCII
3. [ ] Next/previous match navigation (n/N)
4. [ ] Decode tree expand-all with `E` key
5. [ ] Decode tree collapse-all with `C` key
6. [ ] Diff mode: highlight byte differences in hex dump
7. [ ] Copy selected bytes to clipboard
8. [ ] Jump to offset in hex dump with `:goto <offset>`
9. [ ] Manual test: search, expand-all, diff mode

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/panes/hex_dump.rs` (~250 lines)
  - Search functionality
  - Match highlighting
  - Navigation (n/N)
  - Jump to offset
- `crates/prb-tui/src/panes/decode_tree.rs` (~200 lines)
  - Expand-all/collapse-all
  - Better tree navigation

### Search in Hex Dump

```rust
struct HexDumpPane {
    search_query: Option<Vec<u8>>,
    search_matches: Vec<usize>,
    current_match: Option<usize>,
}

fn search(&mut self, query: &str) {
    // Parse hex query (e.g., "48 65 6C 6C 6F")
    let bytes = parse_hex_query(query);
    self.search_matches = find_matches(&self.data, &bytes);
}
```

## Test Plan

1. Open hex dump
2. Press `/` and search for bytes
3. Navigate with n/N
4. Test expand-all in decode tree
5. Test diff mode byte highlighting
6. Run test suite

## Blocked By

- S05 (Schema Decode) - better decode tree requires schema awareness

## Blocks

None - enhancements are additive.

## Rollback Plan

Feature-gate enhancements behind config flag.

## Success Metrics

- Search is fast and accurate
- Highlighting is clear
- Expand-all doesn't crash on large trees
- Good keyboard navigation
- Zero regressions
