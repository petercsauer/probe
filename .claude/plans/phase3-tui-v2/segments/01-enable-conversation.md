---
segment: 01
title: Enable Conversation View
depends: []
risk: 3
complexity: Low
cycle_budget: 3
estimated_lines: 150
---

# Segment 01: Enable Conversation View

## Context

The conversation view code exists in the codebase but is commented out. The `ConversationListPane` is present, `showing_conversations` flag exists, and the conversation engine is available. We need to uncomment and wire this up properly.

## Current State

```rust
// In app.rs:
pub showing_conversations: bool,
// conversation_list: crate::panes::conversation_list::ConversationListPane,

// In render:
if self.showing_conversations {
    // self.conversation_list.render(...)
    self.event_list.render(...)  // fallback
}
```

## Goal

Enable conversation view with a keybinding, allowing users to see conversations (TCP streams, UDP flows) grouped and analyzed.

## Exit Criteria

1. [ ] Uncomment `ConversationListPane` struct and instantiation
2. [ ] Wire conversation view rendering in `render_all()`
3. [ ] Add keybinding (suggest `v` for "view conversations") to toggle `showing_conversations`
4. [ ] Populate conversations from `state.conversations` (already exists)
5. [ ] Status bar shows "CONVERSATIONS" mode indicator
6. [ ] Can toggle back to event list view
7. [ ] All existing tests pass
8. [ ] Manual test: toggle between event list and conversation views

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/app.rs` (~100 lines)
  - Uncomment `conversation_list` field
  - Uncomment conversation rendering
  - Add toggle keybinding
  - Update status bar for mode indicator

### Conversation Data

The `AppState.conversations` field already exists:
```rust
pub conversations: Option<prb_core::engine::ConversationSet>,
```

This is populated from the core engine. The pane just needs to display it.

### Keybinding Suggestion

```rust
KeyCode::Char('v') if self.input_mode == InputMode::Normal => {
    self.showing_conversations = !self.showing_conversations;
    if self.showing_conversations && self.state.conversations.is_none() {
        // Build conversations on-demand
        self.state.conversations = Some(
            prb_core::engine::ConversationSet::from_events(&self.state.store)
        );
    }
    return false;
}
```

## Test Plan

1. Launch TUI with pcap file
2. Press `v` to toggle to conversation view
3. Verify conversations are displayed
4. Press `v` again to toggle back to event list
5. Verify status bar shows correct mode
6. Run test suite: `cargo nextest run -p prb-tui`

## Blocked By

None - this is Wave 1, first priority.

## Blocks

- S09 (Trace Correlation) - needs conversation infrastructure
- S11 (Timeline Enhancements) - benefits from conversation context
- S12 (Complete Waterfall) - uses conversation data
- S18 (Multi-Tab) - will need per-tab conversation state

## Rollback Plan

If issues arise, comment out the conversation code again and revert keybinding.

## Success Metrics

- Conversation view accessible via keybinding
- Can toggle between event list and conversation views
- Zero regressions in existing tests
- Code is uncommented cleanly with no dead code warnings
