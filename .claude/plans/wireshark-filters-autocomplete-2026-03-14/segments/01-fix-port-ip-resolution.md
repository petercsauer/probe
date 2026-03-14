---
segment: 1
title: "Fix Port/IP Field Resolution"
depends_on: []
risk: 5/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "fix(query): Add protocol-aware port/IP field resolution"
---

# Segment 1: Fix Port/IP Field Resolution

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Fix filter bug where `udp.port==5353` shows TCP traffic by implementing structured port/IP extraction and protocol validation.

**Depends on:** None

## Context: Issue 1 - Broken Port/IP Field Resolution

**Core Problem:**
- Filter `udp.port==5353` shows TCP traffic because field resolution doesn't extract ports or validate protocol
- `prb-query/src/eval.rs:26-40` uses string matching only, no structured parsing
- `prb-core/src/event.rs` stores addresses as strings: `src: "IP:port"` format
- `prb-detect/src/registry.rs:249` has IPv6 bug: splits `[::1]:8080` incorrectly on colons

**Current implementation:**
```rust
// eval.rs:26-40
match field {
    "transport" => event.transport.as_ref().map(|t| t.kind.to_string()),
    "src" | "src_addr" => event.network.as_ref().map(|n| n.src.clone()),
    "dst" | "dst_addr" => event.network.as_ref().map(|n| n.dst.clone()),
    // Missing: tcp.port, udp.port, tcp.srcport, udp.srcport, ip.src, ip.dst
}
```

**Root Cause:**
1. No field patterns for protocol-specific ports (`tcp.port`, `udp.port`, `tcp.srcport`, `tcp.dstport`)
2. No IP extraction without port (`ip.src`, `ip.dst`)
3. No protocol validation (filter can reference udp.port on TCP events)
4. String-based address storage prevents structured access

**Proposed Fix:**
Add protocol-aware field resolution with helpers:
```rust
// New helper functions in eval.rs
fn extract_port(addr: &str) -> Option<u16> {
    addr.parse::<SocketAddr>().ok().map(|sa| sa.port())
}

fn extract_ip(addr: &str) -> Option<IpAddr> {
    addr.parse::<SocketAddr>().ok().map(|sa| sa.ip())
}

fn matches_protocol(event: &DebugEvent, expected: &str) -> bool {
    event.transport.as_ref()
        .map(|t| t.kind.to_string().to_lowercase() == expected.to_lowercase())
        .unwrap_or(false)
}

// Extended field matching in resolve_field()
match field {
    // TCP-specific fields
    "tcp.port" if matches_protocol(event, "tcp") => {
        event.network.as_ref()
            .and_then(|n| extract_port(&n.src).or_else(|| extract_port(&n.dst)))
            .map(|p| p.to_string())
    }
    "tcp.srcport" if matches_protocol(event, "tcp") => {
        event.network.as_ref().and_then(|n| extract_port(&n.src)).map(|p| p.to_string())
    }
    "tcp.dstport" if matches_protocol(event, "tcp") => {
        event.network.as_ref().and_then(|n| extract_port(&n.dst)).map(|p| p.to_string())
    }

    // UDP-specific fields
    "udp.port" if matches_protocol(event, "udp") => {
        event.network.as_ref()
            .and_then(|n| extract_port(&n.src).or_else(|| extract_port(&n.dst)))
            .map(|p| p.to_string())
    }
    "udp.srcport" if matches_protocol(event, "udp") => {
        event.network.as_ref().and_then(|n| extract_port(&n.src)).map(|p| p.to_string())
    }
    "udp.dstport" if matches_protocol(event, "udp") => {
        event.network.as_ref().and_then(|n| extract_port(&n.dst)).map(|p| p.to_string())
    }

    // IP-only fields
    "ip.src" => event.network.as_ref().and_then(|n| extract_ip(&n.src)).map(|ip| ip.to_string()),
    "ip.dst" => event.network.as_ref().and_then(|n| extract_ip(&n.dst)).map(|ip| ip.to_string()),
    "ip.addr" => event.network.as_ref()
        .and_then(|n| extract_ip(&n.src).or_else(|| extract_ip(&n.dst)))
        .map(|ip| ip.to_string()),

    // Frame-level fields
    "frame.len" => Some(event.frame_number.to_string()),

    // Protocol field returns None for mismatched protocol
    field if field.starts_with("tcp.") && !matches_protocol(event, "tcp") => None,
    field if field.starts_with("udp.") && !matches_protocol(event, "udp") => None,

    // Fallback to existing logic
    _ => existing_field_resolution(event, field)
}
```

**Also fix registry.rs IPv6 bug:**
```rust
// registry.rs:249 - BEFORE (broken)
let port = addr.split(':').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);

// AFTER (correct)
let port = addr.parse::<SocketAddr>().ok().map(|sa| sa.port()).unwrap_or(0);
```

**Pre-Mortem Risks:**
1. **IPv6 edge cases**: `[::1]:8080` vs `[fe80::1%eth0]:8080` (zone IDs)
2. **Performance**: `parse::<SocketAddr>()` on every field access (mitigated by query planner in S3)
3. **Protocol assumption**: Events without `transport` metadata return None for protocol-specific fields
4. **Dual-stack**: `ip.addr` matches either src or dst (check this matches Wireshark semantics)

**Alternatives Ruled Out:**
- **Manual IP:port parsing**: Error-prone for IPv6, std::net::SocketAddr handles all cases correctly
- **Storing parsed SocketAddr in DebugEvent**: Breaking change to core type, would require migration
- **Caching parsed addresses**: Premature optimization, query planner (S3) will handle performance

## Scope

**Files to modify:**
- `crates/prb-query/src/eval.rs` - Add helper functions and extended field patterns
- `crates/prb-detect/src/registry.rs:249` - Fix IPv6 port extraction bug

**Files to create:**
- `crates/prb-query/tests/field_resolution_test.rs` - New test module for protocol-aware fields

**Unchanged files:**
- `crates/prb-core/src/event.rs` - NetworkAddr remains string-based
- `crates/prb-tui/src/filter_state.rs` - No changes to filter state management

## Implementation Approach

1. **Add helper functions to eval.rs** (before `resolve_field()`)
   - `extract_port(addr: &str) -> Option<u16>`
   - `extract_ip(addr: &str) -> Option<IpAddr>`
   - `matches_protocol(event: &DebugEvent, protocol: &str) -> bool`

2. **Extend match statement in resolve_field()**
   - Add TCP field patterns with protocol check
   - Add UDP field patterns with protocol check
   - Add IP-only field patterns
   - Add frame.len field
   - Add protocol mismatch guards (return None for tcp.* on UDP events)

3. **Fix registry.rs IPv6 bug**
   - Replace string splitting with `parse::<SocketAddr>()`
   - Add test case for IPv6 addresses

4. **Write comprehensive tests**
   - Test TCP port extraction (IPv4 and IPv6)
   - Test UDP port extraction
   - Test IP-only extraction
   - Test protocol mismatch (udp.port on TCP event returns None)
   - Test IPv6 zone IDs if SocketAddr supports them
   - Test frame.len field

5. **Integration test with existing filters**
   - Verify `udp.port==5353` only matches UDP events
   - Verify `tcp.port==443` only matches TCP events
   - Verify `ip.src==192.168.1.1` matches regardless of protocol

## Build and Test Commands

**Build:** `cargo build --package prb-query`

**Test (targeted):** `cargo test --package prb-query field_resolution`

**Test (regression):** `cargo test --package prb-query && cargo test --package prb-detect`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:**
   - `test_tcp_port_extraction_ipv4` - tcp.port field on TCP event
   - `test_tcp_port_extraction_ipv6` - tcp.port with IPv6 addresses
   - `test_udp_port_extraction` - udp.port field on UDP event
   - `test_protocol_mismatch` - udp.port on TCP event returns None
   - `test_ip_only_extraction` - ip.src, ip.dst without ports
   - `test_frame_len_field` - frame.len field
   - `test_registry_ipv6_bug_fix` - registry.rs port extraction with `[::1]:8080`

2. **Regression tests:** All existing prb-query and prb-detect tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - No string splitting logic for address parsing
   - All IPv6 cases handled by SocketAddr
   - Protocol validation guards in place
   - No dead code or commented-out blocks

6. **Scope verification gate:**
   - Only eval.rs, registry.rs, and new test files modified
   - No changes to DebugEvent structure
   - No changes to filter_state.rs

**Risk Factor:** 5/10 - Touches field resolution (shared by all filters), but change is additive and well-tested

**Estimated Complexity:** Medium - Straightforward implementation, but needs comprehensive testing for IPv4/IPv6/protocol edge cases

**Evidence for Optimality:**
1. **Codebase evidence**: std::net::SocketAddr already used in registry.rs (just used incorrectly), proven pattern
2. **Wireshark semantics**: Protocol-specific fields (tcp.port) only match events of that protocol (verified in Wireshark docs)
3. **Rust std library**: SocketAddr::parse() handles all IPv4/IPv6 cases correctly, including zone IDs
4. **Performance**: Query planner (S3) will optimize away repeated parsing by using indices
