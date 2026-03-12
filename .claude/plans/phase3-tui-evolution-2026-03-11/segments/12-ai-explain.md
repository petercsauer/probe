---
segment: 12
title: "AI Explain Panel"
depends_on: [4, 6]
risk: 5
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): AI explain panel with streaming responses, conversation narration, provider config"
---

# Segment 12: AI Explain Panel

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Wire the existing `prb-ai` crate into the TUI with a streaming explain panel, conversation narration, and provider configuration display.

**Depends on:** S04 (Schema Decode — richer event context for AI), S06 (Filter — filter bar patterns)

## Current State

- `prb-ai` has `explain_event(event, config) -> Result<String>` and `explain_event_stream(event, config) -> Result<impl Stream<Item=String>>`
- `AiConfig` supports Ollama (local) and OpenAI providers
- `ExplainContext` enriches event data for the prompt
- The CLI `explain` command exists but is disabled due to an async-openai API change
- No AI integration in the TUI

## Scope

- `crates/prb-tui/Cargo.toml` — Add `prb-ai` dependency
- `crates/prb-tui/src/panes/ai_panel.rs` — **New file.** AI explain panel
- `crates/prb-tui/src/app.rs` — Wire `a` key, AI state management

## Implementation

### 12.1 AI Panel

Press `a` on any event to open a streaming AI explanation panel at the bottom (replaces timeline temporarily):

```
AI Explain ──────────────────────────────────────────────────
This is an outbound gRPC request to /api.v1.Users/Get. The
payload contains a protobuf-encoded "Test" string.

Analysis: No anomalies detected. The gRPC frame header
indicates a unary call with no compression.
▌ (streaming...)
```

The panel occupies the bottom 30% of the screen. Press `a` again or `Esc` to dismiss.

### 12.2 Streaming Integration

Use `explain_event_stream()` for token-by-token streaming:

```rust
struct AiPanel {
    content: String,
    streaming: bool,
    scroll_offset: usize,
    cached: HashMap<EventId, String>, // cache per event
}
```

Run the stream on a tokio task, send tokens via channel:

```rust
fn start_explain(&mut self, event: &DebugEvent, config: &AiConfig) {
    let (tx, rx) = mpsc::channel(100);
    let event_clone = event.clone();
    let config_clone = config.clone();

    tokio::spawn(async move {
        match explain_event_stream(&event_clone, &config_clone).await {
            Ok(mut stream) => {
                while let Some(chunk) = stream.next().await {
                    if tx.send(chunk).await.is_err() { break; }
                }
            }
            Err(e) => { let _ = tx.send(format!("Error: {}", e)).await; }
        }
    });

    self.stream_rx = Some(rx);
    self.streaming = true;
}
```

Drain the channel each frame and append to `content`.

### 12.3 Explanation Cache

Cache explanations per `EventId` to avoid re-querying:

```rust
if let Some(cached) = self.cached.get(&event.id) {
    self.content = cached.clone();
    self.streaming = false;
    return;
}
```

### 12.4 AI Provider Status

Show provider status in the status bar:
- `[AI: ollama/llama3.2]` — connected
- `[AI: openai/gpt-4]` — connected
- `[AI: off]` — no provider configured

### 12.5 Configuration

Read AI config from `~/.config/prb/config.toml` or environment variables:

```toml
[ai]
provider = "ollama"
model = "llama3.2"
endpoint = "http://localhost:11434"
```

Or via `PRB_AI_PROVIDER`, `PRB_AI_MODEL`, `PRB_AI_ENDPOINT` env vars.

### 12.6 Fix async-openai Compatibility

Check the async-openai API change that disabled the CLI explain command. May need a version bump or minor API adjustment in `prb-ai`.

## Key Files and Context

- `crates/prb-ai/src/explain.rs` — `explain_event()`, `explain_event_stream()`
- `crates/prb-ai/src/config.rs` — `AiConfig`, `AiProvider`
- `crates/prb-ai/src/context.rs` — `ExplainContext` enrichment
- `crates/prb-tui/src/app.rs` — Event loop, pane rendering

## Pre-Mortem Risks

- async-openai version may need updating — check Cargo.toml compatibility
- Ollama may not be running locally — show graceful "AI unavailable" message
- Streaming requires async runtime — ensure tokio is properly configured in TUI
- Large responses may overwhelm the panel — add scrolling and max-length cap

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui -p prb-ai`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **AI panel:** `a` opens streaming explain panel at bottom of screen
2. **Streaming:** Tokens appear incrementally as the LLM generates them
3. **Cache:** Re-pressing `a` on same event shows cached result instantly
4. **Provider status:** Status bar shows AI provider info or "off"
5. **Graceful errors:** Missing provider shows "AI unavailable — configure in ~/.config/prb/config.toml"
6. **Tests:** AI panel state management tests pass (mock stream)
7. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
