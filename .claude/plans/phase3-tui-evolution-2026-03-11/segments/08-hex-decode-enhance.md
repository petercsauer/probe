---
segment: 8
title: "Hex Dump & Decode Tree Enhancements"
depends_on: [4]
risk: 4
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): hex search, byte grouping, decode expand-all, field copy, event diff"
---

# Segment 8: Hex Dump & Decode Tree Enhancements

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Elevate hex dump and decode tree from basic display to power-user tools with search, byte grouping, expand-all, field copy, and event diff.

**Depends on:** S04 (Schema-Aware Decode — enhanced tree content)

## Current State

- Hex dump: displays bytes with ASCII sidebar, scrolls, highlights from decode tree
- Decode tree: expandable tree via tui-tree-widget
- No search, no expand-all, no byte grouping, no diff

## Scope

- `crates/prb-tui/src/panes/hex_dump.rs` — Search, byte grouping, value inspector
- `crates/prb-tui/src/panes/decode_tree.rs` — Expand/collapse all, field copy, event diff

## Implementation

### 8.1 Hex Search

Press `/` in hex pane to search for hex bytes (`DE AD BE EF`) or ASCII (`"Hello"`). Store matches as byte offsets. `n`/`N` navigate matches. Highlight all matches with distinct color.

### 8.2 Byte Group Toggle

`b` cycles 1-byte / 2-byte / 4-byte grouping. Adjust render spacing accordingly.

### 8.3 Value Inspector

Show selected byte(s) interpreted as different types at bottom of hex pane:
```
Offset: 0x0010 | u8: 42 | u16le: 10794 | u16be: 42752 | ASCII: '*'
```

### 8.4 Jump to Offset

`g` in hex pane opens "Go to offset" input. Type hex offset, Enter jumps.

### 8.5 Expand/Collapse All

- `e` — expand all decode tree nodes
- `E` — collapse all nodes
Use `TreeState` methods.

### 8.6 Field Value Copy

`y` on decode tree node copies value via OSC 52 clipboard escape sequence.

### 8.7 Event Diff

Mark first event with `m`, navigate to second, press `D` for side-by-side diff overlay. Color: yellow=changed, green=added, red=removed fields.

### 8.8 Cross-Highlight Verification

Verify `Action::HighlightBytes` works end-to-end between decode tree and hex dump.

## Key Files and Context

- `crates/prb-tui/src/panes/hex_dump.rs` — HexDumpPane, highlight state
- `crates/prb-tui/src/panes/decode_tree.rs` — DecodeTreePane, TreeState
- `crates/prb-tui/src/panes/mod.rs` — Action::HighlightBytes
- `crates/prb-core/src/event.rs` — Payload variants

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Hex search:** `/` opens search, `n`/`N` navigate matches
2. **Byte grouping:** `b` cycles 1/2/4-byte groups
3. **Expand/collapse:** `e`/`E` in decode tree
4. **Field copy:** `y` copies decode tree value via OSC 52
5. **Event diff:** `m` marks, `D` shows side-by-side diff overlay
6. **Cross-highlight:** Decode tree → hex dump highlighting works
7. **Tests pass:** `cargo nextest run -p prb-tui`
8. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
