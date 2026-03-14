# prb-tui

Interactive terminal UI for PRB, built with ratatui and crossterm. Displays decoded network events in a multi-pane layout with keyboard navigation, real-time filtering, and support for multiple input formats (JSON fixtures, PCAP captures, MCAP recordings). Supports both offline file analysis and live packet capture.

## Key types

| Type | Description |
|------|-------------|
| `App` | Top-level application — owns state, event loop, and rendering |
| `EventStore` | Indexed, filterable collection of `DebugEvent`s |
| `RingBuffer` | Fixed-capacity ring buffer for live-capture event storage |
| `LiveDataSource` | Async event source for live packet capture |
| `AppEvent` | Internal event enum (key press, new data, resize, …) |
| `CaptureState` | Live capture lifecycle state |
| `PaneComponent` | Trait implemented by each pane for input handling and rendering |

### Panes

| Pane | Purpose |
|------|---------|
| `event_list` | Scrollable table of events with timestamp, protocol, direction, and summary columns |
| `decode_tree` | Hierarchical tree view of decoded fields for the selected event |
| `hex_dump` | Raw hex + ASCII dump of the event payload with byte-range highlighting |
| `timeline` | Visual timeline showing event distribution over time |

## Usage

```rust
use prb_tui::App;
use camino::Utf8PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = Utf8PathBuf::from("capture.json");
    let app = App::from_file(&path)?;
    app.run().await
}
```

## Relationship to other crates

- **prb-core** — provides `DebugEvent` and protocol types rendered by the UI
- **prb-query** — powers the interactive filter bar
- **prb-storage** — persistence layer for event stores
- **prb-pcap** — reads PCAP/PCAPNG files for offline analysis
- **prb-fixture** — loads JSON fixture files
- **prb-grpc**, **prb-zmq**, **prb-dds** — protocol decoders invoked during file loading
- **prb-capture** — live packet capture data source

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

Terminal UI for the PRB universal message debugger.

This crate provides an interactive terminal interface for analyzing debug events,
with support for filtering, AI-powered explanations, and live capture.

<!-- cargo-rdme end -->
