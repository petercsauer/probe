# ADR 0002: Error Handling Strategy

## Status

Accepted

## Context

PRB parses untrusted network data which can be malformed. Need consistent
error handling across all decoders and layers.

## Decision

1. Use `thiserror` for library error types
2. Use `anyhow` for application errors in CLI
3. All decoders return `Result<T, CoreError>` for cross-crate boundaries
4. Non-fatal warnings stored in `DebugEvent::warnings` field
5. Never panic on malformed input (use Result instead)

## Consequences

**Positive:**
- Consistent error handling across crates
- Rich error context for debugging
- Warnings visible in output without failing
- Safe against malformed packets

**Negative:**
- More verbose than unwrap/expect
- Some error paths need testing

## Implementation

Library crates define error enums with `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum GrpcError {
    #[error("invalid HTTP/2 frame: {0}")]
    InvalidH2Frame(String),

    #[error("decompression failed: {0}")]
    DecompressionFailed(String),
}
```

The CLI uses `anyhow::Result` for main functions:

```rust
fn main() -> anyhow::Result<()> {
    let events = load_events()?;
    process_events(&events)?;
    Ok(())
}
```

Non-fatal issues are captured as warnings:

```rust
if incomplete_message {
    event.warnings.push("Message truncated mid-stream".to_string());
}
```
