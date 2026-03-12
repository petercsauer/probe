---
segment: 11
title: "Live Capture Mode"
depends_on: [1]
risk: 7
complexity: High
cycle_budget: 12
status: pending
commit_message: "feat(prb-tui): live capture mode — async event loop, capture control bar, auto-scroll, ring buffer"
---

# Segment 11: Live Capture Mode

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Wire the existing `LiveDataSource`, `RingBuffer`, and `AppEvent` infrastructure into the TUI event loop, creating a fully functional live capture mode with control bar, auto-scroll, and memory management.

**Depends on:** S01 (Visual Polish — status bar redesign)

## Current State

- `LiveDataSource::start(adapter, interface)` spawns capture thread, returns `mpsc::Receiver<AppEvent>`
- `AppEvent` enum: `Key`, `Tick`, `Resize`, `CapturedEvent`, `CaptureStats`, `CaptureStopped`
- `CaptureState` enum: `Capturing`, `Paused`, `Stopped`
- `RingBuffer<T>` with `push`, `len`, `evicted`, capacity management — fully tested
- `EventStore::push()` exists for appending events
- `prb-capture` is already a dependency in `crates/prb-tui/Cargo.toml` — no dep change needed
- **None of this is wired into the App event loop** — the TUI only works in file-load mode

## Scope

- `crates/prb-tui/src/app.rs` — Dual-mode event loop (file vs live), capture state management
- `crates/prb-tui/src/live.rs` — May need adjustments for integration
- `crates/prb-cli/src/commands/tui.rs` — Wire `--live` or `--interface` flag
- `crates/prb-cli/src/commands/capture.rs` — Wire `--tui` flag

## Implementation

### 11.1 Dual-Mode Event Loop

Add a second event loop method for live mode:

```rust
impl App {
    pub fn new_live(store: EventStore, live_source: LiveDataSource) -> Self { ... }

    pub fn run_live(&mut self) -> Result<()> {
        let mut rx = self.live_source.take_receiver().unwrap();
        let mut capture_state = CaptureState::Capturing;

        loop {
            self.draw(&mut terminal)?;

            // Drain capture events (batched for performance)
            let mut batch_count = 0;
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::CapturedEvent(debug_event) => {
                        self.state.store.push(*debug_event);
                        batch_count += 1;
                        if batch_count >= 100 { break; } // cap per frame
                    }
                    AppEvent::CaptureStats(stats) => {
                        self.capture_stats = Some(stats);
                    }
                    AppEvent::CaptureStopped => {
                        capture_state = CaptureState::Stopped;
                    }
                    _ => {}
                }
            }

            // Recompute filtered indices if new events arrived
            if batch_count > 0 {
                self.recompute_filter();
                if self.auto_follow { self.scroll_to_bottom(); }
            }

            // Poll keyboard
            if event::poll(Duration::from_millis(33))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key) { break; }
                }
            }
        }
    }
}
```

### 11.2 Capture Control Bar

Replace status bar with capture control bar when in live mode:

```
 ● CAPTURING  en0 | 1,234 pkts | 56.3 KB | 423 pps | 0 drops | [S]top [P]ause [F]ollow
```

State colors: green=capturing, yellow=paused, red=stopped.

Key bindings in live mode:
- `S` — stop capture (sends stop to LiveDataSource)
- `P` — pause/resume
- `f` — toggle auto-follow (already used for quick-filter in normal mode, only active when not filtering)

### 11.3 Auto-Scroll with Follow Mode

When auto-follow is enabled and user is at the bottom of the event list:
- New events auto-scroll the list to show latest
- Scrolling up disengages auto-follow
- Show `[FOLLOW]` indicator in control bar when active
- If disengaged, show "N new events below — press F to follow"

```rust
struct App {
    auto_follow: bool,
    new_events_since_scroll: usize,
    // ...
}
```

### 11.4 Ring Buffer Integration

Use `RingBuffer` to cap events at 100K (configurable):

```rust
fn push_event(&mut self, event: DebugEvent) {
    self.ring_buffer.push(event);
    // Sync EventStore from ring buffer
    // Or replace EventStore with RingBuffer-backed store
}
```

Show eviction count in control bar: `100K/234K events (134K evicted)`.

### 11.5 Rate Limiting and Batching

At high throughput (>1K pps):
- Batch events per render frame (cap at 30fps)
- Skip decode tree/hex dump updates for non-focused panes
- Show pps counter in control bar

### 11.6 CLI Wiring

In `capture.rs`:
```rust
if args.tui {
    let adapter = LiveCaptureAdapter::new(config)?;
    let live_source = LiveDataSource::start(adapter, interface)?;
    let store = EventStore::empty();
    let mut app = App::new_live(store, live_source);
    return app.run_live();
}
```

## Key Files and Context

- `crates/prb-tui/src/live.rs` — `LiveDataSource`, `AppEvent`, `CaptureState`
- `crates/prb-tui/src/ring_buffer.rs` — `RingBuffer<T>` (fully implemented with tests)
- `crates/prb-tui/src/app.rs` — Current file-mode event loop
- `crates/prb-tui/src/event_store.rs` — `EventStore::push()`
- `crates/prb-capture/src/adapter.rs` — `LiveCaptureAdapter`
- `crates/prb-cli/src/commands/capture.rs` — CLI capture command

## Pre-Mortem Risks

- Live capture requires OS privileges (pcap) — ensure graceful error on permission denied
- High-throughput capture may overwhelm the render loop — batching is critical
- Ring buffer eviction invalidates filtered_indices — need to recompute
- Thread safety: `LiveDataSource` runs capture on a separate thread — ensure mpsc is the only communication

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Dual event loop:** `App::run_live()` processes both keyboard and capture events
2. **Control bar:** Shows capture state, interface, pkt count, pps, drops
3. **Auto-follow:** New events scroll list when at bottom, disengage on scroll-up
4. **Ring buffer:** Events capped at 100K, eviction count shown
5. **Rate limiting:** UI stays responsive at >1K pps
6. **CLI flags:** `prb capture --tui` launches live capture TUI
7. **Graceful errors:** Permission denied and missing interface show useful error messages
8. **Tests:** Live event loop unit tests (mock receiver), ring buffer integration tests pass
9. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
