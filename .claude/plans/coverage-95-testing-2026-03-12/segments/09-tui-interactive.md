---
segment: 9
title: "TUI Interactive Testing"
depends_on: []
risk: 4/10
complexity: Medium
cycle_budget: 15
status: merged
commit_message: "test(tui): Add async, mouse, and resize interaction tests"
---

# Segment 9: TUI Interactive Testing

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add async, mouse, and resize testing for TUI components to cover interaction flows.

**Depends on:** None (independent)

## Context: Issues Addressed

**Core Problem:** TUI has 600+ tests but interaction flows are undertested. Async live capture with tokio channels untested. Mouse support (lines 500-600 in app.rs) untested. Resize handling untested. No property tests for navigation invariants (selection always in bounds). Accessibility (keyboard-only navigation) untested.

**Proposed Fix:** Add async tests for live capture event streams and ring buffer overflow. Add mouse tests for pane focus clicks, resize drags, and scroll wheel. Add resize tests for layout preservation. Add property tests for navigation invariants. Add keyboard-only accessibility tests.

**Pre-Mortem Risks:**
- Async tests could be flaky on timing - mitigate by using deterministic event ordering, avoid real-time sleeps
- Mouse tests depend on exact layout calculation - use specific known terminal sizes (80×24, 120×40)
- Property tests might be slow with 100-key sequences - acceptable, can run with `--release` if needed

## Scope

- `crates/prb-tui` async/interaction tests (new test files)
- `crates/prb-tui/tests/async_capture_test.rs` - New async live capture tests
- `crates/prb-tui/tests/mouse_test.rs` - New mouse interaction tests
- `crates/prb-tui/tests/resize_test.rs` - New terminal resize tests
- `crates/prb-tui/tests/navigation_property_test.rs` - New property tests
- `crates/prb-tui/tests/accessibility_test.rs` - New keyboard-only tests

## Key Files and Context

**`crates/prb-tui/src/app.rs`**:
- State machine with 11 input modes
- Mouse support (lines 500-600)
- Resize handling

**`crates/prb-tui/src/live.rs`**:
- Async live capture with tokio channels

**Existing tests:**
- `test_handle_key()` method for synchronous key simulation

**Missing coverage:**
- Async tests for live capture
- Mouse interaction tests
- Resize tests
- Property tests for navigation
- Keyboard-only accessibility tests

## Implementation Approach

1. **Add async live capture tests** in `tests/async_capture_test.rs`:
   ```rust
   #[tokio::test]
   async fn test_live_capture_event_stream() {
       let (tx, rx) = tokio::sync::mpsc::channel(100);
       let mut app = App::new_live(rx);

       tx.send(event1).await.unwrap();
       tokio::time::sleep(Duration::from_millis(10)).await;

       assert_eq!(app.state.store.len(), 1);
       assert_eq!(app.state.store.get(0), Some(&event1));
   }

   #[tokio::test]
   async fn test_ring_buffer_overflow() {
       let (tx, rx) = tokio::sync::mpsc::channel(100);
       let mut app = App::new_live_with_ring_buffer(rx, 10);

       for i in 0..20 {
           tx.send(make_event(i)).await.unwrap();
       }
       tokio::time::sleep(Duration::from_millis(50)).await;

       assert_eq!(app.state.store.len(), 10); // Ring buffer capacity
       // Verify oldest events dropped, newest 10 retained
   }
   ```

2. **Add mouse interaction tests** in `tests/mouse_test.rs`:
   - Pane focus by click: Click on decode tree pane area, verify focus changes
   - Resize drag: Simulate drag on split border, verify split percentages update
   - Scroll with mouse wheel: Send ScrollUp/ScrollDown events, verify scroll offsets
   ```rust
   #[test]
   fn test_mouse_pane_focus() {
       let mut app = setup_app_with_panes();
       assert_eq!(app.focus, PaneId::EventList);

       // Click on decode tree pane (assume rect at x=40, y=10)
       app.handle_mouse_event(MouseEvent {
           kind: MouseEventKind::Down(MouseButton::Left),
           column: 40,
           row: 10,
           modifiers: KeyModifiers::empty(),
       });

       assert_eq!(app.focus, PaneId::DecodeTree);
   }
   ```

3. **Add terminal resize tests** in `tests/resize_test.rs`:
   - Render at 80×24, resize to 120×40, verify:
     - Selection index preserved
     - Layout recalculated (pane_rects updated)
     - No panic or visual glitches
   - Test resize during filter input mode (edge case - ensure input not lost)
   - Test resize with zoomed pane (should maintain zoom state)

4. **Add property tests** for navigation in `tests/navigation_property_test.rs`:
   ```rust
   use proptest::prelude::*;

   proptest! {
       #[test]
       fn navigation_selection_always_valid(
           keys in prop::collection::vec(
               prop::sample::select(vec![KeyCode::Up, KeyCode::Down, KeyCode::PageUp, KeyCode::PageDown]),
               0..100
           ),
           event_count in 0usize..1000
       ) {
           let events = (0..event_count).map(|i| make_event(i)).collect();
           let mut app = App::new(EventStore::new(events), None, None);

           for key in keys {
               app.test_handle_key(KeyEvent::new(key, KeyModifiers::NONE));

               // Invariants:
               if event_count > 0 {
                   assert!(app.state.selected_event.is_some());
                   let idx = app.state.selected_event.unwrap();
                   assert!(idx < event_count); // Never out of bounds
               }
           }
       }
   }
   ```

5. **Add keyboard-only navigation test** (accessibility) in `tests/accessibility_test.rs`:
   - Navigate through all 8 panes with Tab key
   - Open and close all overlays with keyboard shortcuts (?, c, e, etc.)
   - Verify no mouse-only features (all actions keyboard-accessible)
   - Test with screen reader simulation (optional: verify ARIA-like hints in status bar)

## Alternatives Ruled Out

- **Mocking at TTY level:** Rejected - too complex, TestBackend + event simulation sufficient
- **Only synchronous tests:** Rejected - async is production reality with live capture, must validate tokio integration

## Pre-Mortem Risks

- Async tests could be flaky on timing: Mitigate by using deterministic event ordering, avoid real-time sleeps where possible
- Mouse tests depend on exact layout calculation: Use specific known terminal sizes (80×24, 120×40) and test with those
- Property tests might be slow with 100-key sequences: Acceptable - property tests should be thorough, can run with `--release` if needed

## Build and Test Commands

- Build: `cargo build -p prb-tui`
- Test (targeted): `cargo test -p prb-tui async_capture mouse_interaction resize navigation_property accessibility`
- Test (regression): `cargo test -p prb-tui`
- Test (full gate): `cargo nextest run -p prb-tui`

## Exit Criteria

1. **Targeted tests:**
   - `async_capture` - 5 tests pass (event stream, ring buffer overflow, state transitions, channel close, backpressure)
   - `mouse_interaction` - 3 tests pass (pane focus click, resize drag, scroll wheel)
   - `resize` - 3 tests pass (layout preservation, filter mode resize, zoomed pane resize)
   - `navigation_property` - proptest passes (100+ key sequences, selection always valid)
   - `accessibility` - keyboard-only navigation test passes (all 8 panes, all overlays)
   - Total: 12+ new interaction tests

2. **Regression tests:** All existing TUI tests pass (600+ tests)

3. **Full build gate:** `cargo build -p prb-tui` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-tui` passes (612+ total tests: 600+ existing + 12+ new)

5. **Self-review gate:**
   - Async tests use deterministic timing with tokio::time::pause where possible
   - Property tests bounded to prevent CI timeout
   - Mouse tests use known terminal dimensions

6. **Scope verification gate:** Only modified:
   - Test files added in `tests/` directory
   - No src/ changes
