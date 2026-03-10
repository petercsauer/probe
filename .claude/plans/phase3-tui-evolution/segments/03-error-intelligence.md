---
segment: 3
title: "Error Intelligence"
depends_on: []
risk: 2
complexity: Low
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): inline error intelligence — status code tooltips, TCP/TLS explanations, warning badges"
---

# Segment 3: Error Intelligence

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Surface human-readable explanations for protocol error codes, TCP states, and TLS alerts directly in the decode tree and event list — no LLM needed, purely static lookup tables.

**Depends on:** None

## Current State

- Decode tree shows raw field values like `grpc.status: 4` with no explanation
- Events with `warnings` vec are not visually distinguished in the event list
- TCP RST, TLS alert codes show as raw numbers
- No protocol-specific intelligence in the UI

## Scope

- `crates/prb-tui/src/error_intel.rs` — **New file.** Static lookup tables for gRPC, TCP, TLS codes
- `crates/prb-tui/src/panes/decode_tree.rs` — Inline explanations next to known fields
- `crates/prb-tui/src/panes/event_list.rs` — Warning badge in event list rows

## Implementation

### 3.1 Error Intelligence Module

Create `crates/prb-tui/src/error_intel.rs` with static lookup functions:

```rust
pub fn grpc_status_name(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("OK"),
        1 => Some("CANCELLED"),
        2 => Some("UNKNOWN"),
        3 => Some("INVALID_ARGUMENT"),
        4 => Some("DEADLINE_EXCEEDED"),
        5 => Some("NOT_FOUND"),
        6 => Some("ALREADY_EXISTS"),
        7 => Some("PERMISSION_DENIED"),
        8 => Some("RESOURCE_EXHAUSTED"),
        9 => Some("FAILED_PRECONDITION"),
        10 => Some("ABORTED"),
        11 => Some("OUT_OF_RANGE"),
        12 => Some("UNIMPLEMENTED"),
        13 => Some("INTERNAL"),
        14 => Some("UNAVAILABLE"),
        15 => Some("DATA_LOSS"),
        16 => Some("UNAUTHENTICATED"),
        _ => None,
    }
}

pub fn grpc_status_explanation(code: u32) -> Option<&'static str> {
    match code {
        4 => Some("The deadline expired before the operation completed"),
        7 => Some("Caller lacks permission for this operation"),
        8 => Some("Server resource limit reached (quota, memory, connections)"),
        13 => Some("Internal server error — check server logs"),
        14 => Some("Server unavailable — may be starting up or overloaded"),
        _ => None,
    }
}

pub fn tcp_flag_explanation(flags: &str) -> Option<&'static str> {
    match flags {
        "RST" | "R" => Some("Connection forcefully terminated. Causes: crashed server, firewall, port not listening"),
        "FIN" => Some("Graceful connection close initiated"),
        _ => None,
    }
}

pub fn tls_alert_description(code: u8) -> Option<&'static str> {
    match code {
        0 => Some("close_notify — Connection closing normally"),
        10 => Some("unexpected_message — Inappropriate message received"),
        20 => Some("bad_record_mac — Record failed integrity check"),
        40 => Some("handshake_failure — No common cipher suite or parameters"),
        42 => Some("bad_certificate — Certificate is corrupt"),
        43 => Some("unsupported_certificate — Certificate type not supported"),
        44 => Some("certificate_revoked — Certificate was revoked by CA"),
        45 => Some("certificate_expired — Certificate has expired"),
        46 => Some("certificate_unknown — Unknown certificate issue"),
        47 => Some("illegal_parameter — Handshake field out of range"),
        48 => Some("unknown_ca — CA certificate not recognized"),
        49 => Some("access_denied — Valid cert but access denied by policy"),
        50 => Some("decode_error — Message could not be decoded"),
        70 => Some("protocol_version — Protocol version not supported"),
        71 => Some("insufficient_security — Cipher suite too weak"),
        80 => Some("internal_error — Internal error unrelated to peer"),
        86 => Some("inappropriate_fallback — Downgrade attack detected"),
        90 => Some("user_canceled — Handshake canceled by user"),
        112 => Some("missing_extension — Required extension missing"),
        _ => None,
    }
}
```

### 3.2 Decode Tree Integration

In `decode_tree.rs`, when building tree items from event metadata, check if values match known protocol fields and append explanations:

```rust
// When rendering a metadata field like "grpc.status"
if key == "grpc.status" {
    if let Ok(code) = value.parse::<u32>() {
        if let Some(name) = error_intel::grpc_status_name(code) {
            // Render as: "grpc.status: 4 (DEADLINE_EXCEEDED)"
            label = format!("{}: {} ({})", key, value, name);
        }
    }
}
```

For explanations, add them as child nodes that can be expanded:

```
▶ grpc.status: 4 (DEADLINE_EXCEEDED)
  └─ The deadline expired before the operation completed
```

### 3.3 Warning Badges in Event List

In `event_list.rs`, prefix the ID or Summary column with a `!` indicator for events with non-empty warnings:

```rust
let warning_indicator = if !event.warnings.is_empty() { "! " } else { "  " };
let id_text = format!("{}{}", warning_indicator, event.id);
```

Style the `!` with `Theme::warning()` (already exists).

### 3.4 Register Module

Add `pub mod error_intel;` to `lib.rs`.

## Key Files and Context

- `crates/prb-core/src/event.rs` — `DebugEvent { warnings: Vec<String>, metadata: HashMap<String, String> }`
- `crates/prb-tui/src/panes/decode_tree.rs` — Tree building from event data
- `crates/prb-tui/src/panes/event_list.rs` — Row rendering
- `crates/prb-tui/src/theme.rs` — `warning()` style already exists

## Build and Test Commands

- Build: `cargo check -p prb-tui`
- Test (targeted): `cargo nextest run -p prb-tui`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **New module:** `error_intel.rs` exists with lookup functions for gRPC (17 codes), TLS (15+ alerts), TCP flags
2. **Targeted tests:** Unit tests for all lookup functions pass
3. **Decode tree:** gRPC status codes show human name inline, with expandable explanation
4. **Warning badges:** Events with warnings show `!` prefix in event list
5. **Regression tests:** `cargo nextest run --workspace` — no regressions
6. **Full build gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
