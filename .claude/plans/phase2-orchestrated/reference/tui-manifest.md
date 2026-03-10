# Phase 2: Interactive TUI with Real-Time Filtering — Deep Plan

**Goal**: Build a Termshark-grade terminal UI that transforms Probe from a batch
CLI tool into an interactive investigation environment. Four panes—event list,
protocol decode tree, hex dump, and timeline—with a real-time query language for
filtering decoded protocol fields.

**Scope**: Two new crates (`prb-query`, `prb-tui`), one modified crate (`prb-cli`).
~4,500 lines of new code.

**State of the art references**:
- **Termshark** (Go/tcell): 3-pane Wireshark-in-terminal, the UX gold standard
- **RustNet** (Rust/ratatui, 1,795 stars): real-time network monitor with DPI,
  vim/fzf filtering, proves developer demand for Rust TUI network tools
- **ntap** (Rust): CLI+TUI network analyzer with process attribution
- **Wireshark**: GUI benchmark—packet list + decode tree + hex dump layout

**Architecture**: Component Architecture (not TEA) per ratatui best practices.
Each pane is a self-contained component with `handle_event()`, `update()`, and
`render()` methods. An `App` struct owns all components and routes keyboard events
to the focused pane.

---

## Dependency Map

```
prb-query ──────────────────────────────┐
  (nom parser, evaluator)               │
                                        ▼
prb-tui ◄── prb-core, prb-query, prb-fixture, prb-storage, prb-pcap
  │         prb-grpc, prb-zmq, prb-dds, prb-decode
  │
  │  Libraries:
  │  ├── ratatui 0.30        (TUI framework)
  │  ├── crossterm            (terminal backend)
  │  ├── tui-tree-widget 0.24 (expandable tree)
  │  ├── tui-input 0.15       (filter input)
  │  ├── tokio (rt, sync)     (async event loop)
  │  └── unicode-width        (column alignment)
  │
  ▼
prb-cli  ←  adds `prb tui <file>` subcommand
```

---

## Subsection Index

| # | Subsection | Segments | New Crate | Est. Lines |
|---|-----------|----------|-----------|------------|
| 1 | Query Language Engine | 3 | `prb-query` | ~800 |
| 2 | TUI Core & App Shell | 3 | `prb-tui` | ~600 |
| 3 | Event List Pane | 3 | — | ~500 |
| 4 | Protocol Decode Tree | 2 | — | ~400 |
| 5 | Hex Dump Pane | 2 | — | ~350 |
| 6 | Timeline Pane | 2 | — | ~250 |
| 7 | Data Layer & CLI Integration | 3 | — | ~500 |
| 8 | Conversation & Session Reconstruction | 6 | — | ~1,500 |

**Execution order**: S1 → S8.1 → (S8.2, S8.3, S8.4 parallel) → S8.5 → S2 → S7 → S3 → S8.6 → S4 → S5 → S6 (S4-S6 parallelize)

S8 (Conversation Reconstruction) is a Phase 2A prerequisite per the competitive
analysis roadmap. S8.1–S8.5 run before TUI work (S2) because conversation data
feeds the TUI's conversation view. S8.6 (TUI integration) runs after S3 (Event
List) since it adds the conversation overlay to the event list pane.

---

## Subsection Details

### S1: Query Language Engine (`prb-query`)

See: `subsection-1-query-language.md`

A `nom`-based parser that compiles filter expressions to predicate closures
evaluated against `DebugEvent`. Supports field access (dot notation into
metadata), comparisons, boolean logic, string operators, and time ranges.

**Segments**:
- S1.1: Lexer + AST types
- S1.2: Parser (nom combinators) + evaluator
- S1.3: prb-core integration + CLI `--where` flag

### S2: TUI Core & App Shell (`prb-tui`)

See: `subsection-2-tui-core.md`

Application shell with crossterm backend, async event loop (tick + render +
keyboard), pane focus management, and theme system.

**Segments**:
- S2.1: Crate scaffold, terminal init/restore, event loop
- S2.2: Layout engine (4-pane split) + focus routing
- S2.3: Theme/color system, status bar, help overlay

### S3: Event List Pane

See: `subsection-3-event-list-pane.md`

Virtual-scrolling table displaying events with columns: #, timestamp, source,
destination, protocol, direction, summary. Integrated filter bar using prb-query.

**Segments**:
- S3.1: Virtual-scroll table widget
- S3.2: Filter bar with live prb-query evaluation
- S3.3: Column sorting + detail summary generation

### S4: Protocol Decode Tree

See: `subsection-4-decode-tree-pane.md`

Hierarchical tree view of the selected event's decoded protocol layers using
tui-tree-widget. Expandable nodes for each protocol layer (IP, TCP, gRPC, etc.).

**Segments**:
- S4.1: Tree node model + DebugEvent-to-tree conversion
- S4.2: Protocol-specific formatters (gRPC, ZMQ, DDS, TCP, UDP)

### S5: Hex Dump Pane

See: `subsection-5-hex-dump-pane.md`

Classic hex dump (16 bytes/line) with ASCII sidebar. Cross-highlights bytes
corresponding to the selected tree node. Selection support for copy.

**Segments**:
- S5.1: Hex dump renderer with scrolling
- S5.2: Cross-highlighting from decode tree selection

### S6: Timeline Pane

See: `subsection-6-timeline-pane.md`

Sparkline showing event density over time. Serves as a minimap for navigating
large captures. Shows protocol distribution via color.

**Segments**:
- S6.1: Sparkline widget with time axis
- S6.2: Protocol distribution overlay + time range display

### S7: Data Layer & CLI Integration

See: `subsection-7-data-integration.md`

Event store that loads DebugEvents from JSON fixtures, MCAP sessions, or PCAP
files. Index for fast filtering. CLI `prb tui <file>` subcommand.

**Segments**:
- S7.1: EventStore struct with indexing
- S7.2: File loaders (JSON, MCAP, PCAP pipeline)
- S7.3: `prb tui` subcommand in prb-cli

### S8: Conversation & Session Reconstruction

See: `subsection-8-conversation-reconstruction.md`

Implements recommendation #8 from the competitive analysis. Groups related
DebugEvents into logical conversations (gRPC request/response pairs, ZMQ
REQ/REP exchanges, DDS topic flows) with latency metrics, error classification,
and state tracking. Implements the existing `CorrelationStrategy` trait from
prb-core with protocol-specific strategies in prb-grpc, prb-zmq, and prb-dds.

**Segments**:
- S8.1: Conversation model + ConversationEngine in prb-core
- S8.2: GrpcCorrelationStrategy (connection + H2 stream ID grouping)
- S8.3: ZmqCorrelationStrategy (PUB/SUB, REQ/REP, PUSH/PULL patterns)
- S8.4: DdsCorrelationStrategy (domain + topic + writer GUID grouping)
- S8.5: Latency analysis + error classification
- S8.6: CLI `prb conversations` command + TUI conversation view overlay

---

## Key Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| TUI framework | ratatui 0.30 + crossterm | De facto Rust TUI standard, 19M+ downloads |
| Architecture | Component pattern | Better than TEA for multi-pane apps with independent state |
| Query parser | nom | Battle-tested, zero-copy, composable combinators |
| Tree widget | tui-tree-widget 0.24 | Purpose-built for ratatui, actively maintained |
| Input widget | tui-input 0.15 | 1.1M+ downloads, works with ratatui 0.30 |
| Async runtime | tokio (current_thread) | Needed for event stream + tick timer |
| Virtual scroll | Manual implementation | ratatui Table is O(n) — custom windowed rendering |
| Hex dump | Custom widget | No suitable crate exists; ~150 lines |

---

## Keyboard Map

```
Global:
  Tab / Shift+Tab    Cycle focus between panes
  q / Ctrl+C         Quit
  ?                  Toggle help overlay
  /                  Focus filter bar
  Esc                Clear filter / close overlay

Event List:
  j / ↓              Next event
  k / ↑              Previous event
  g / Home           First event
  G / End            Last event
  Enter              Select event (populate decode tree + hex dump)
  s                  Cycle sort column
  S                  Reverse sort

Decode Tree:
  j / ↓              Next node
  k / ↑              Previous node
  Enter / →          Expand node
  Backspace / ←      Collapse node
  Space              Toggle expand/collapse

Hex Dump:
  j / ↓              Scroll down
  k / ↑              Scroll up

Filter Bar:
  Enter              Apply filter
  Esc                Cancel / clear
  Ctrl+U             Clear input
```

---

## Acceptance Criteria

- [ ] `cargo build --workspace` — zero errors, zero warnings
- [ ] `cargo clippy --workspace --all-targets` — zero warnings
- [ ] `cargo test --workspace` — all tests pass
- [ ] `prb tui fixtures/grpc_sample.json` opens TUI with events displayed
- [ ] Events displayed in scrollable table with columns
- [ ] Selecting an event shows decode tree and hex dump
- [ ] Filter bar accepts and applies prb-query expressions
- [ ] Timeline sparkline reflects event distribution
- [ ] Tab cycles focus between all 4 panes
- [ ] vim-style navigation (j/k/g/G) works in all panes
- [ ] `/` opens filter, Esc clears it, Enter applies
- [ ] `q` quits cleanly with terminal restored
- [ ] Handles 100k+ events without lag (virtual scroll)
- [ ] `prb inspect --where "grpc.method contains Users"` works from CLI
- [ ] `prb conversations <file>` lists all reconstructed conversations
- [ ] gRPC events grouped by connection + H2 stream ID into conversations
- [ ] ZMQ events grouped by socket pattern (PUB/SUB, REQ/REP, PUSH/PULL)
- [ ] DDS events grouped by domain + topic + writer GUID
- [ ] Conversation metrics: latency, TTFR, byte count, error status
- [ ] `C` key in TUI opens conversation view for selected event
- [ ] `L` key in TUI switches to conversation list view
