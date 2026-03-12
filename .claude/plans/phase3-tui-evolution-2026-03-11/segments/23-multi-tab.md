---
segment: 23
title: "Multi-Tab Support"
depends_on: [9]
risk: 7
complexity: High
cycle_budget: 10
status: pending
commit_message: "feat(prb-tui): multi-tab support — tab bar, per-tab state, tab switching"
---

# Segment 23: Multi-Tab Support

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Support opening multiple capture files in tabs, each with independent state (event store, filter, selection, conversations).

**Depends on:** S09 (Conversation View — per-tab conversation state)

## Scope

- `crates/prb-tui/src/tab.rs` — **New file.** Tab state, TabManager
- `crates/prb-tui/src/app.rs` — Major refactor: extract per-tab state, add tab bar rendering
- `crates/prb-cli/src/commands/tui.rs` — Accept multiple file arguments

## Implementation

### 23.1 Tab State

Extract per-tab state from `App`:

```rust
pub struct TabState {
    pub state: AppState,  // store, filter, selected_event, etc.
    pub event_list: EventListPane,
    pub decode_tree: DecodeTreePane,
    pub hex_dump: HexDumpPane,
    pub timeline: TimelinePane,
    pub conversations: Option<ConversationSet>,
    pub schema_registry: Option<SchemaRegistry>,
    pub label: String, // filename or "live:en0"
}

pub struct TabManager {
    tabs: Vec<TabState>,
    active: usize,
}
```

### 23.2 Tab Bar

Render tab bar at the top of the screen:

```
[1: capture.pcap] [2: replay.json] [3: live:en0]
```

Active tab highlighted with theme accent color. Width-limited: truncate filenames, scroll if many tabs.

### 23.3 Tab Operations

- `1`-`9` — switch to tab by number
- `Ctrl+T` — open new tab (shows file picker or welcome screen)
- `Ctrl+W` — close current tab (confirm if modified)
- `Ctrl+Tab` — next tab
- `Ctrl+Shift+Tab` — previous tab

### 23.4 CLI Multi-File

```bash
prb tui file1.pcap file2.json file3.mcap
```

Opens each file in its own tab.

### 23.5 Refactor App

The `App` struct becomes a shell around `TabManager`:

```rust
pub struct App {
    tab_manager: TabManager,
    focus: PaneId,
    input_mode: InputMode,
    // global state (theme, AI config, etc.)
}
```

All rendering and key handling delegates to the active tab's state.

## Pre-Mortem Risks

- Major refactor of App — many tests may need updating
- Memory consumption multiplied by tab count — may need lazy loading for inactive tabs
- Focus and input mode are global, but filter state is per-tab — careful separation needed

## Exit Criteria

1. **Tab bar:** Rendered at top showing all open tabs
2. **Tab switching:** Number keys and Ctrl+Tab switch tabs
3. **Independent state:** Each tab has its own events, filter, selection
4. **Open/close:** Ctrl+T opens, Ctrl+W closes tabs
5. **Multi-file CLI:** Multiple file args open in separate tabs
6. **Tests pass:** `cargo nextest run -p prb-tui`
7. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
