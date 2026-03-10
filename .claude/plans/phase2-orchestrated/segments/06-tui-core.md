---
segment: 6
title: "TUI Core & App Shell (prb-tui)"
depends_on: []
risk: 5
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-tui): add ratatui app shell with event loop, layout, and theme"
---

# Subsection 2: TUI Core & App Shell (`prb-tui`)

## Purpose

Application skeleton: terminal init/restore, async event loop, pane layout,
focus management, and theme system. All other panes plug into this shell.

## Architecture

```
┌─ App ────────────────────────────────────────────┐
│                                                   │
│  EventLoop (tokio current_thread)                 │
│  ├── crossterm EventStream → AppEvent::Key/Mouse  │
│  ├── tick interval (250ms) → AppEvent::Tick       │
│  └── render interval (16ms) → AppEvent::Render    │
│                                                   │
│  AppState                                         │
│  ├── events: EventStore                           │
│  ├── filter: Option<Filter>                       │
│  ├── filtered_indices: Vec<usize>                 │
│  ├── selected_event: Option<usize>                │
│  └── focus: PaneId                                │
│                                                   │
│  Panes (Component trait)                          │
│  ├── EventListPane                                │
│  ├── DecodeTreePane                               │
│  ├── HexDumpPane                                  │
│  └── TimelinePane                                 │
│                                                   │
│  Overlays                                         │
│  ├── FilterBar                                    │
│  └── HelpOverlay                                  │
└───────────────────────────────────────────────────┘
```

## Component Trait

```rust
pub trait PaneComponent {
    fn handle_key(&mut self, key: KeyEvent, state: &mut AppState) -> Action;
    fn render(&self, area: Rect, buf: &mut Buffer, state: &AppState, focused: bool);
}

pub enum Action {
    None,
    SelectEvent(usize),
    HighlightBytes { offset: usize, len: usize },
    Quit,
}
```

---

## Segment S2.1: Crate Scaffold + Event Loop

**Terminal lifecycle**: init (enable raw mode, alternate screen, mouse capture),
restore on drop (disable raw, leave alternate screen). Use `scopeguard` or
manual Drop impl to guarantee cleanup on panic.

**Event loop** (tokio `current_thread`):
```
loop {
    select! {
        Some(event) = crossterm_stream.next() => handle_input(event),
        _ = tick_interval.tick() => handle_tick(),
        _ = render_interval.tick() => { terminal.draw(|f| app.render(f))?; }
    }
}
```

Tick: 4 Hz (filter recomputation, stats update).
Render: 60 Hz (smooth scrolling).

---

## Segment S2.2: Layout Engine + Focus

**Layout** (Termshark-inspired):
```
┌─────────────────────────────────────────────┐
│ Filter: transport == "gRPC"       [4/127]   │  ← filter bar (1 line)
├─────────────────────────────────────────────┤
│ #  │ Time     │ Src        │ Dst   │ Proto  │  ← event list (50%)
│ 1  │ 14:00:01 │ 10.0.0.1   │ :8080 │ gRPC   │
│ 2  │ 14:00:01 │ :8080      │ 10.0  │ gRPC   │
│ 3  │ 14:00:02 │ 10.0.0.1   │ :8080 │ gRPC   │
├─────────────┬───────────────────────────────┤
│ ▶ Event     │  00 01 02 03 04 05 06 07 ...  │  ← decode tree (25%)
│   ▶ Source  │  08 09 0a 0b 0c 0d 0e 0f ...  │    + hex dump
│   ▶ gRPC    │  ................................│
│     method  │  ................................│
├─────────────┴───────────────────────────────┤
│ ▁▂▃▅▇█▇▅▃▂▁▁▂▃▅▇█▇▅▃▂▁  14:00 ─── 14:05  │  ← timeline (3 lines)
├─────────────────────────────────────────────┤
│ 127 events │ gRPC: 89 │ ZMQ: 38 │ q:quit   │  ← status bar (1 line)
└─────────────────────────────────────────────┘
```

**Focus routing**: `Tab` cycles `EventList → DecodeTree → HexDump → Timeline`.
`/` jumps to filter bar. `Esc` returns to event list.

---

## Segment S2.3: Theme + Status Bar + Help

**Theme**: Dark terminal theme using ratatui `Style`:
- Selected row: bold white on dark blue
- Focused pane border: cyan
- Unfocused pane border: dark gray
- gRPC events: green
- ZMQ events: yellow
- DDS events: magenta
- Filter match count: bright white
- Warnings: red

**Status bar**: Event count, protocol breakdown, current filter, keybind hints.

**Help overlay** (`?`): Full-screen overlay listing all keybinds.
