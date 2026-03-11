---
segment: 03
title: Wire AI Panel
depends: []
risk: 4
complexity: Medium
cycle_budget: 5
estimated_lines: 300
---

# Segment 03: Wire AI Panel

## Context

The `AiPanel` struct exists but may not be fully wired for streaming AI explanations. The panel should explain selected events/packets using AI, with streaming output for good UX.

## Current State

```rust
// In app.rs:
ai_panel: AiPanel,
ai_panel_visible: bool,
```

The prb-ai crate exists with AI functionality, but integration with TUI may be incomplete.

## Goal

Complete AI panel integration so users can get streaming AI explanations of selected packets, conversations, or anomalies.

## Exit Criteria

1. [ ] AI panel toggles with keybinding (suggest `a` for "AI explain")
2. [ ] Panel renders correctly in layout
3. [ ] Streaming AI response displays character-by-character
4. [ ] Panel shows loading indicator while waiting for AI
5. [ ] Can close panel with same keybinding or Esc
6. [ ] Explain selected event/packet
7. [ ] Handle errors gracefully (API key missing, network error)
8. [ ] Status bar shows "AI EXPLAIN" mode
9. [ ] Manual test: explain a complex protocol message

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/panes/ai_panel.rs` (~200 lines)
  - Complete streaming display
  - Add loading indicator
  - Error handling
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Wire AI panel toggle keybinding
  - Integrate with prb-ai crate
  - Handle async streaming

### AI Explanation Flow

1. User selects event and presses `a`
2. AI panel becomes visible
3. Loading indicator shows while API call is made
4. AI response streams in character-by-character
5. User can close panel or ask for more details

### Integration with prb-ai

The prb-ai crate should have an `explain_event` function:

```rust
pub async fn explain_event(
    event: &DebugEvent,
    context: Option<&ConversationContext>,
) -> Result<impl Stream<Item = String>, AiError> {
    // Stream AI response
}
```

### Async Handling in TUI

Since the TUI event loop is synchronous, we'll need to:
1. Spawn async task to call AI
2. Send chunks back via channel
3. TUI polls channel in event loop and updates display

## Test Plan

1. Export `ANTHROPIC_API_KEY` or similar
2. Launch TUI with test pcap
3. Select an event
4. Press `a` to open AI panel
5. Verify streaming response displays correctly
6. Test with missing API key (should show error)
7. Test with network error (should handle gracefully)
8. Run test suite: `cargo nextest run -p prb-tui`

## Blocked By

None - AI panel is independent feature.

## Blocks

- S10 (AI Smart Features) - needs working AI panel foundation

## Rollback Plan

If AI integration is problematic, feature-gate it behind `--features ai` or disable the keybinding.

## Success Metrics

- AI explanations stream smoothly
- Good error handling
- No blocking or freezing of TUI
- Clear loading indicators
- Zero regressions in existing tests

## Notes

- AI explanations require API key configuration
- May want to add config option for AI model selection
- Consider rate limiting to avoid API costs
- Could cache explanations for repeated events
