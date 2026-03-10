---
segment: 10
title: "Hex Dump Pane"
depends_on: [6]
risk: 2
complexity: Low
cycle_budget: 2
status: pending
commit_message: "feat(prb-tui): add hex dump pane with cross-highlighting from decode tree"
---

# Subsection 5: Hex Dump Pane

## Purpose

Classic hex dump of the selected event's raw payload bytes. 16 bytes per line
with offset, hex columns, and ASCII sidebar. Supports cross-highlighting when
a decode tree node is selected.

## Layout

```
Offset   00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f  ASCII
─────────────────────────────────────────────────────────────────
00000000 50 52 49 20 2a 20 48 54  54 50 2f 32 2e 30 0d 0a  PRI * HT TP/2.0..
00000010 0d 0a 53 4d 0d 0a 0d 0a  00 00 12 04 00 00 00 00  ..SM.... ........
00000020 00 00 03 00 00 00 64 00  04 00 01 00 00 00 05 00  ......d. ........
```

When a decode tree node with a byte range is selected, the corresponding hex
bytes are highlighted (reverse video) and the view auto-scrolls to show them.

---

## Segment S5.1: Hex Dump Renderer

Custom ratatui widget:
- Input: `&[u8]` payload, `scroll_offset`, optional `highlight: Option<(usize, usize)>`
- Renders 16 bytes per line with offset column
- ASCII sidebar: printable chars shown, non-printable as '.'
- Scrollbar for payloads exceeding visible area
- j/k scrolls when focused

## Segment S5.2: Cross-Highlighting

When the user navigates the decode tree and selects a node with a `byte_range`:
- The hex pane receives `Action::HighlightBytes { offset, len }`
- The highlighted byte range renders with `Style::reversed()`
- The hex pane auto-scrolls so the highlighted range is visible
- If no byte range, highlight is cleared
