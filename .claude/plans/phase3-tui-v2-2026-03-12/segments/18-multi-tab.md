---
segment: 18
title: Multi-Tab Support
depends_on: [01]
risk: 7
complexity: High
cycle_budget: 10
estimated_lines: 650
---

# Segment 18: Multi-Tab Support

## Context

Users need to compare multiple captures or views simultaneously. Multi-tab support allows opening multiple files/sessions in tabs, with independent state per tab.

## Current State

- Single file/capture view only
- No tab management
- App state is global, not per-tab

## Goal

Add tab bar UI and per-tab state management, allowing users to open multiple files in tabs and switch between them quickly.

## Exit Criteria

1. [ ] Tab bar renders at top of TUI
2. [ ] Can open new tab with `:open <file>` command
3. [ ] Can switch tabs with `gt` (next) and `gT` (previous)
4. [ ] Can close tab with `:close` command
5. [ ] Each tab has independent state:
   - Event store
   - Filter
   - Scroll position
   - Selected event
   - Pane focus
6. [ ] Tab shows filename/label
7. [ ] Active tab visually distinct
8. [ ] Can rename tab with `:tab-name <name>`
9. [ ] Tab count limit (e.g., 10 max)
10. [ ] Manual test: open 3 tabs, switch between them

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/app.rs` (~400 lines)
  - Add TabManager struct
  - Refactor App to support per-tab state
  - Tab switching logic
- `crates/prb-tui/src/tabs.rs` (~250 lines NEW)
  - Tab struct
  - TabManager implementation
  - Tab bar rendering

### Architecture

```rust
struct Tab {
    id: TabId,
    label: String,
    state: TabState,  // Contains EventStore, filter, etc.
}

struct TabState {
    store: EventStore,
    filter: Option<Filter>,
    selected_event: Option<usize>,
    scroll_offset: usize,
    focus: PaneId,
    // ... other per-tab state
}

struct TabManager {
    tabs: Vec<Tab>,
    active_tab: TabId,
}

impl TabManager {
    fn switch_to(&mut self, tab_id: TabId) { ... }
    fn close_tab(&mut self, tab_id: TabId) { ... }
    fn new_tab(&mut self, state: TabState) -> TabId { ... }
}
```

### Tab Bar Rendering

```
┌─ [1: grpc.pcap] [2: http.pcap*] [3: mqtt.pcap] ────────┐
```

The active tab shows with `*` or highlighted styling.

### State Management

Current global state needs to move into `TabState`:
- `EventStore`
- `filtered_indices`
- `filter`
- `selected_event`
- `scroll_offset`
- pane states

App-level state remains global:
- `theme`
- `config`
- `plugin_manager`
- global overlays (help, command palette)

## Test Plan

1. Launch TUI with one file
2. Open new tab: `:open test2.pcap`
3. Verify tab bar shows both tabs
4. Switch tabs with `gt`/`gT`
5. Verify each tab has independent scroll/selection
6. Close tab with `:close`
7. Test with live capture tab + file tabs
8. Run test suite: `cargo nextest run -p prb-tui`

## Blocked By

- S01 (Enable Conversation View) - conversations need per-tab state

## Blocks

None - multi-tab is late-stage polish.

## Rollback Plan

If multi-tab proves too complex, feature-gate behind `--tabs` flag or disable temporarily.

## Success Metrics

- Clean tab switching with no state leakage
- Tab bar renders correctly
- Independent state per tab verified
- No crashes when closing tabs
- Good UX for tab management
- Zero regressions in existing tests

## Notes

- Tab state serialization for session save/restore
- Consider max tab limit to prevent resource exhaustion
- Keyboard shortcuts follow Vim conventions (gt, gT)
- Could add mouse click on tab bar
- May want drag-and-drop tab reordering (future enhancement)
