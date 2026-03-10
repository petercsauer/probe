---
segment: 5
title: "Pane Zoom, Resize & Mouse"
depends_on: [1]
risk: 4
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): pane zoom/maximize, resizable splits, mouse click/scroll/drag"
---

# Segment 5: Pane Zoom, Resize & Mouse

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add pane maximize/zoom, resizable pane splits, and full mouse support (click to focus/select, scroll, drag borders).

**Depends on:** S01 (Visual Polish — pane focus indicators)

## Current State

- 4-pane layout with fixed split percentages
- Tab/BackTab cycles focus
- Mouse capture is enabled in crossterm but no mouse events are handled
- No way to zoom into a single pane or resize splits

## Scope

- `crates/prb-tui/src/app.rs` — Layout engine changes, zoom state, mouse event handling, split percentages
- `crates/prb-tui/src/panes/event_list.rs` — Mouse click to select row
- `crates/prb-tui/src/panes/mod.rs` — Optionally add `handle_mouse()` to PaneComponent

## Implementation

### 5.1 Pane Zoom / Maximize

Add zoom state to `App`:

```rust
struct App {
    zoomed_pane: Option<PaneId>,
    // ... existing fields
}
```

Press `z` to toggle zoom on the focused pane. When zoomed:

```rust
fn render_all(&mut self, frame: &mut Frame, area: Rect) {
    if let Some(zoomed) = self.zoomed_pane {
        // Render only the zoomed pane at full area
        self.render_single_pane(frame, area, zoomed, true);
        return;
    }
    // Normal 4-pane layout
    // ...
}
```

`z` again or `Esc` restores normal layout. Show `[ZOOMED]` in status bar.

### 5.2 Resizable Pane Splits

Track split percentages:

```rust
struct App {
    vertical_split: u16,   // event list height %, default 50
    horizontal_split: u16, // decode tree width %, default 50
    // ...
}
```

`+`/`-` keys adjust the focused pane's share by 5% increments (clamped to 20%-80%). The layout engine uses these percentages with `Constraint::Percentage`.

### 5.3 Mouse Click

Handle `Event::Mouse(MouseEvent)` in the event loop:

```rust
MouseEventKind::Down(MouseButton::Left) => {
    let (col, row) = (mouse.column, mouse.row);
    // Determine which pane was clicked based on stored pane rects
    if let Some(pane) = self.pane_at(col, row) {
        self.focus = pane;
        // If event list, calculate which row was clicked
        if pane == PaneId::EventList {
            let row_in_pane = row - self.pane_rects[&PaneId::EventList].y - 1; // -1 for border
            let event_idx = self.event_list.scroll_offset + row_in_pane as usize;
            self.process_action(Action::SelectEvent(event_idx));
        }
    }
}
```

Store `pane_rects: HashMap<PaneId, Rect>` during rendering to enable hit-testing.

### 5.4 Mouse Scroll

```rust
MouseEventKind::ScrollDown => {
    // Route scroll to focused pane
    match self.focus {
        PaneId::EventList => self.event_list.scroll_down(3),
        PaneId::HexDump => self.hex_dump.scroll_down(3),
        PaneId::DecodeTree => self.decode_tree.scroll_down(1),
        _ => {}
    }
}
MouseEventKind::ScrollUp => { /* similar */ }
```

### 5.5 Mouse Drag to Resize

Track drag state:

```rust
enum DragState {
    None,
    ResizingVertical(u16),   // dragging horizontal border
    ResizingHorizontal(u16), // dragging vertical border
}
```

On `MouseDown` near a border (within 1 cell), enter drag mode. On `MouseDrag`, update split percentages. On `MouseUp`, exit drag mode.

### 5.6 Jump-to-Event

Press `#` to open a "Go to event #" input:

```rust
InputMode::GoToEvent
```

Type an event ID, press Enter to jump, Esc to cancel. Reuse the filter input widget pattern.

## Key Files and Context

- `crates/prb-tui/src/app.rs` — Event loop (already captures mouse events), layout rendering
- `crates/prb-tui/src/panes/event_list.rs` — Row selection, scroll management
- `crates/prb-tui/src/panes/mod.rs` — PaneComponent trait

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Zoom:** `z` key maximizes focused pane to full screen, `z` again restores layout
2. **Resize:** `+`/`-` keys adjust pane split percentages, visible in layout changes
3. **Mouse click:** Click on event list row selects it, click on pane focuses it
4. **Mouse scroll:** Scroll wheel works in event list and hex dump
5. **Jump-to:** `#` opens event ID input, Enter jumps to that event
6. **Tests pass:** `cargo nextest run -p prb-tui` — no regressions
7. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
