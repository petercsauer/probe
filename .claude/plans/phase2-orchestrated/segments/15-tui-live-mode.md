---
segment: 15
title: "TUI Live Capture Mode"
depends_on: [6, 8, 14]
risk: 8
complexity: High
cycle_budget: 4
status: pending
commit_message: "feat(prb-tui): add live capture mode with auto-scroll, stats, and capture controls"
---

# S4: TUI Live Mode

**Goal**: Extend the Phase 2 TUI (ratatui-based, 4-pane layout) to accept live
capture as a data source. The event list auto-scrolls as new packets arrive,
with real-time stats, capture control (start/stop/pause), and rate-limited
rendering to handle high-throughput traffic.

**References**: Termshark (live capture TUI, 3K★), RustNet (Rust/ratatui real-time
network monitor, 1.8K★), Wireshark live capture mode.

---

## S4.1: `LiveDataSource` — tokio Channel → EventStore Bridge

### Problem

The Phase 2 TUI's `EventStore` (S7 of the TUI plan) is designed for static data:
load all events at init, then navigate. Live capture needs to append events
continuously while the user browses.

### Solution: `LiveDataSource`

A bridge that receives `DebugEvent`s from the `LiveCaptureAdapter`'s async channel
and appends them to the `EventStore` in a ring-buffer fashion (capped at a
configurable maximum, e.g., 100K events, evicting oldest).

```rust
// In prb-tui or prb-capture

pub struct LiveDataSource {
    adapter: LiveCaptureAdapter,
    event_ring: RingBuffer<DebugEvent>,
    max_events: usize,
    paused: bool,
}

pub struct RingBuffer<T> {
    data: VecDeque<T>,
    capacity: usize,
    total_pushed: u64,
    evicted: u64,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
            total_pushed: 0,
            evicted: 0,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
            self.evicted += 1;
        }
        self.data.push_back(item);
        self.total_pushed += 1;
    }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn iter(&self) -> impl Iterator<Item = &T> { self.data.iter() }
    pub fn get(&self, index: usize) -> Option<&T> { self.data.get(index) }
}
```

### Integration with TUI Event Loop

The TUI's async event loop (from Phase 2 S2: TUI Core) processes three event
types: keyboard input, tick (UI refresh), and data events. Live capture adds a
fourth:

```rust
enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
    // New for live capture:
    CapturedEvent(DebugEvent),
    CaptureStats(CaptureStats),
    CaptureStopped,
}
```

### Event Receiver Task

A dedicated tokio task drains events from the `LiveCaptureAdapter` and sends them
to the TUI event channel:

```rust
async fn capture_event_forwarder(
    mut adapter: LiveCaptureAdapter,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
) {
    loop {
        match adapter.next_event_async().await {
            Some(Ok(event)) => {
                if tx.send(AppEvent::CapturedEvent(event)).await.is_err() {
                    break;
                }
            }
            Some(Err(e)) => {
                tracing::warn!("Capture event error: {}", e);
            }
            None => {
                let _ = tx.send(AppEvent::CaptureStopped).await;
                break;
            }
        }
    }
}
```

### Stats Ticker

A second task periodically polls capture stats:

```rust
async fn stats_ticker(
    adapter: Arc<LiveCaptureAdapter>,
    tx: tokio::sync::mpsc::Sender<AppEvent>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let stats = adapter.stats();
        if tx.send(AppEvent::CaptureStats(stats)).await.is_err() {
            break;
        }
    }
}
```

---

## S4.2: Capture Control Bar + Stats Overlay

### Capture Control Bar

A status bar at the bottom of the TUI showing capture state and controls:

```
┌─ Event List ───────────────────────────────────────────────────────────────────┐
│ #  Timestamp           Source              Dest                Proto  Summary  │
│ 1  14:32:01.123456     10.0.0.1:50051      10.0.0.2:8080       gRPC   POST /…│
│ 2  14:32:01.234567     10.0.0.2:8080       10.0.0.1:50051      gRPC   200 OK │
│ ...                                                                           │
├─ Decode Tree ──────────┬─ Hex Dump ────────────────────────────────────────────┤
│ ▶ Ethernet             │ 0000  45 00 00 3c 1c 46 40 00  40 06 ...  E..<.F@.@.│
│   ▶ IPv4               │ 0010  c0 a8 01 64 c0 a8 01 65  50 00 ...  ...d...eP.│
│     ▶ TCP              │                                                      │
│       ▶ gRPC           │                                                      │
├────────────────────────┴──────────────────────────────────────────────────────┤
│ ● CAPTURING  eth0 │ 1,234 pkts │ 56.3 KB │ 423 pps │ 0 drops │ [S]top [P]ause│
└───────────────────────────────────────────────────────────────────────────────┘
```

### Control Bar Widget

```rust
struct CaptureControlBar {
    state: CaptureState,
    interface: String,
    stats: CaptureStats,
    decoded_count: u64,
}

#[derive(Clone, Copy)]
enum CaptureState {
    Capturing,
    Paused,
    Stopped,
}

impl CaptureControlBar {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let state_indicator = match self.state {
            CaptureState::Capturing => Span::styled("● CAPTURING", Style::default().fg(Color::Green)),
            CaptureState::Paused => Span::styled("❚❚ PAUSED", Style::default().fg(Color::Yellow)),
            CaptureState::Stopped => Span::styled("■ STOPPED", Style::default().fg(Color::Red)),
        };

        let stats_text = format!(
            " {} │ {} pkts │ {} │ {:.0} pps │ {} drops",
            self.interface,
            self.stats.packets_received,
            format_bytes(self.stats.bytes_received),
            self.stats.packets_per_second,
            self.stats.total_drops(),
        );

        let controls = match self.state {
            CaptureState::Capturing => " │ [S]top [P]ause",
            CaptureState::Paused => " │ [S]top [R]esume",
            CaptureState::Stopped => " │ [Q]uit",
        };

        // Render as a single line with colored segments
        let line = Line::from(vec![
            state_indicator,
            Span::raw(stats_text),
            Span::styled(controls, Style::default().fg(Color::DarkGray)),
        ]);

        Paragraph::new(line)
            .block(Block::default().borders(Borders::TOP))
            .render(area, buf);
    }
}
```

### Keyboard Bindings (Live Capture Mode)

| Key | Action |
|-----|--------|
| `s` / `S` | Stop capture |
| `p` / `P` | Pause/resume capture (stop appending, keep displaying) |
| `f` | Toggle auto-follow (auto-scroll to newest) |
| `Space` | Scroll-lock toggle: freeze at current position |
| `q` | Stop capture + quit |

These bindings overlay the standard TUI bindings. When the event list pane is
focused and capture is active, `s` stops capture rather than cycling sort.

---

## S4.3: Auto-Scroll + Rate Limiting

### Auto-Scroll Behavior

When the user is at the bottom of the event list (hasn't scrolled up), new events
auto-scroll into view. When the user scrolls up to inspect older events, auto-scroll
disables. A status indicator shows when new events are arriving below the viewport:

```
│ ▼ 47 new events below — press 'f' to follow │
```

```rust
struct EventListPane {
    // ... existing fields ...
    auto_follow: bool,
    unseen_below: u64,
}

impl EventListPane {
    fn handle_new_event(&mut self) {
        if self.auto_follow {
            self.scroll_to_bottom();
            self.unseen_below = 0;
        } else {
            self.unseen_below += 1;
        }
    }

    fn handle_scroll_up(&mut self) {
        self.auto_follow = false;
    }

    fn handle_follow_toggle(&mut self) {
        self.auto_follow = !self.auto_follow;
        if self.auto_follow {
            self.scroll_to_bottom();
            self.unseen_below = 0;
        }
    }
}
```

### Render Rate Limiting

At high packet rates (>1000 pps), rendering every packet individually would
overwhelm the terminal. Solution: batch updates and limit render rate.

```rust
const MAX_RENDER_FPS: u32 = 30;
const RENDER_INTERVAL: Duration = Duration::from_millis(33); // ~30fps

struct App {
    // ...
    pending_events: Vec<DebugEvent>,
    last_render: Instant,
}

impl App {
    fn handle_captured_event(&mut self, event: DebugEvent) {
        self.pending_events.push(event);
    }

    fn tick(&mut self) {
        if self.last_render.elapsed() >= RENDER_INTERVAL {
            // Flush all pending events to the store
            let events = std::mem::take(&mut self.pending_events);
            for event in events {
                self.event_store.push(event);
                self.event_list.handle_new_event();
            }
            self.last_render = Instant::now();
            // Trigger render
        }
    }
}
```

This means at 10k pps, events are batched into groups of ~333 per render frame,
keeping the terminal responsive.

### High-Throughput Optimization

For sustained >10k pps, additional optimizations:

1. **Summary-only mode**: Skip decode tree and hex dump updates when not focused.
   Only update the event list table.
2. **Decimation**: At >50k pps, only display every Nth event in the list, with
   a counter showing actual count. All events still stored in the ring buffer.
3. **Virtual rendering**: Only render visible rows in the event list. The existing
   Phase 2 virtual scroll (S3.1) handles this.

---

## Layout: Live Capture TUI

The TUI layout is the same 4-pane layout from Phase 2, with the capture control
bar replacing the status bar when in live mode:

```
┌─────────────────────────────────────────────────────┐
│ Event List (scrolling table)                        │  60%
│                                                     │
│                                                     │
│                                                     │
├─────────────────────────┬───────────────────────────┤
│ Decode Tree (selected)  │ Hex Dump (selected)       │  35%
│                         │                           │
│                         │                           │
├─────────────────────────┴───────────────────────────┤
│ ● CAPTURING eth0 │ 1,234 pkts │ 423 pps │ 0 drops  │   5%
└─────────────────────────────────────────────────────┘
```

When capture is stopped, the control bar shows final stats and the TUI enters
"browse mode" (same as loading a static file).

---

## Implementation Checklist

- [ ] Define `AppEvent::CapturedEvent`, `CaptureStats`, `CaptureStopped` variants
- [ ] Implement `RingBuffer<T>` with push/evict/iterate
- [ ] Implement `LiveDataSource` connecting adapter to EventStore
- [ ] Implement `capture_event_forwarder` tokio task
- [ ] Implement `stats_ticker` tokio task
- [ ] Implement `CaptureControlBar` widget
- [ ] Implement `CaptureState` enum + keyboard handler
- [ ] Add auto-follow logic to `EventListPane`
- [ ] Add "N new events below" indicator
- [ ] Implement render rate limiting (30fps cap)
- [ ] Implement event batching for high-throughput display
- [ ] Wire `--tui` flag in `run_capture()` to launch TUI with live source
- [ ] Test: TUI renders with mock event source
- [ ] Test: auto-follow engages at bottom, disengages on scroll up
- [ ] Test: ring buffer evicts oldest when full
