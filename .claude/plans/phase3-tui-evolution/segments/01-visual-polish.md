---
segment: 1
title: "Visual Polish & Status Bar"
depends_on: []
risk: 2
complexity: Low
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): visual polish — zebra striping, focused pane indicators, contextual status bar"
---

# Segment 1: Visual Polish & Status Bar

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Transform the TUI from functional-but-plain to visually polished. Add zebra striping, improved focus indicators, and a context-aware status bar.

**Depends on:** None (Phase 0 char clipping fix is already merged)

## Current State

- `Theme::selected_row()` uses `fg(Black).bg(Cyan)` (fixed from bold clipping bug)
- Status bar shows event count + protocol breakdown + keybind hints
- Focus changes border color (cyan vs gray) but nothing else
- No row alternation; all non-selected rows share the same background

## Scope

- `crates/prb-tui/src/theme.rs` — Add zebra row style, focused/unfocused pane title styles
- `crates/prb-tui/src/app.rs` — Redesign status bar rendering, add contextual hints per pane
- `crates/prb-tui/src/panes/event_list.rs` — Wire zebra striping into row rendering
- `crates/prb-tui/src/panes/mod.rs` — Optionally add pane-specific hint text method to PaneComponent

## Implementation

### 1.1 Zebra Striping

Add alternating row backgrounds in `event_list.rs`:

```rust
// In theme.rs
pub fn zebra_row() -> Style {
    Style::default().bg(Color::Rgb(25, 25, 35))
}

pub fn normal_row() -> Style {
    Style::default()
}
```

In the render loop, alternate based on row index:

```rust
let base_style = if is_selected {
    Theme::selected_row()
} else if row_index % 2 == 1 {
    Theme::zebra_row()
} else {
    Theme::normal_row()
};
```

### 1.2 Focused Pane Indicators

Enhance focus visibility beyond just border color:

- Focused pane: `BorderType::Rounded` border, cyan, bold title with `" [*]"` suffix
- Unfocused pane: `BorderType::Plain` border, dark gray, dim title
- Dim unfocused pane content slightly by reducing foreground brightness

In `app.rs` where panes are rendered with blocks:

```rust
let block = if focused {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::focused_border())
        .title(format!(" {} [*] ", title))
        .title_style(Theme::focused_title())
} else {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Theme::unfocused_border())
        .title(format!(" {} ", title))
        .title_style(Theme::unfocused_title())
};
```

Add `focused_title()` and `unfocused_title()` to Theme.

### 1.3 Context-Aware Status Bar

Redesign `render_status_bar_static` to show different hints based on focused pane:

```
EventList focused:  "j/k:nav  s:sort  /:filter  z:zoom  ?:help  q:quit"
DecodeTree focused: "j/k:nav  Enter:expand  Space:toggle  z:zoom"
HexDump focused:    "j/k:scroll  g:top  z:zoom"
Timeline focused:   "z:zoom"
```

Each pane should expose a `keybind_hints() -> &'static str` method (add to PaneComponent or standalone function).

The status bar layout:

```
 {event_count} events │ {protocol_breakdown} │ {pane_hints}
```

Use `unicode-width` for accurate padding (already established pattern from Phase 0 fix).

### 1.4 Warning Row Tinting

Events with non-empty `warnings` vec get a subtle red-tinted background when not selected:

```rust
pub fn warning_row() -> Style {
    Style::default().bg(Color::Rgb(50, 20, 20))
}
```

Check `event.warnings.is_empty()` during row rendering.

## Key Files and Context

- `crates/prb-tui/src/theme.rs` — All style definitions (currently static methods)
- `crates/prb-tui/src/app.rs` — `render_status_bar_static()`, `render_all()` pane block creation
- `crates/prb-tui/src/panes/event_list.rs` — Row rendering loop in `render()`
- `crates/prb-tui/src/panes/mod.rs` — `PaneComponent` trait definition

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-tui` — all existing tests pass, no regressions
2. **Visual verification:** Zebra striping renders for odd rows, focused pane has rounded border + `[*]` + cyan, unfocused pane has plain border + gray
3. **Status bar:** Shows pane-specific hints that change when Tab cycles focus
4. **Warning tint:** Events with warnings render with red-tinted background
5. **Regression tests:** `cargo nextest run --workspace` — no regressions
6. **Full build gate:** `cargo build --workspace`
7. **Lint gate:** `cargo clippy --workspace -- -D warnings`
