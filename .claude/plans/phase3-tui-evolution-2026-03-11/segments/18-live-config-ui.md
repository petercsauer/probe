---
segment: 18
title: "Live Capture Config UI"
depends_on: [11]
risk: 4
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "feat(prb-tui): live capture config UI — interface picker, BPF filter input, privilege check"
---

# Segment 18: Live Capture Config UI

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add a TUI interface for configuring live capture: network interface selection, BPF filter input, capture settings, and privilege warnings.

**Depends on:** S11 (Live Capture Mode — the capture infrastructure to configure)

## Current State

- `prb-capture` has `InterfaceEnumerator::list()` → `Vec<InterfaceInfo>` with name, status, addresses, loopback flag
- `CaptureConfig` has interface, filter, snaplen, promiscuous, buffer_size
- `PrivilegeCheck` can verify capture permissions
- All configured via CLI flags only — no TUI interface

## Scope

- `crates/prb-tui/src/overlays/capture_config.rs` — **New file.** Interface picker and config dialog
- `crates/prb-tui/src/app.rs` — Wire the config overlay

## Implementation

### 18.1 Interface Picker

When entering live capture mode (or pressing `I`), show interface selection dialog:

```
Select Interface ──────────────────────────
 > en0     Up    192.168.1.100    Wi-Fi
   lo0     Up    127.0.0.1        [loopback]
   en1     Down                   Ethernet
 ──────────────────────────────────────────
 Enter: select  b: BPF filter  q: cancel
```

Use `InterfaceEnumerator::list()` to populate. j/k to navigate, Enter to select. Gray out Down interfaces.

### 18.2 BPF Filter Input

Press `b` in interface picker to enter BPF filter:

```
BPF Filter: port 8080 and tcp
```

Basic text input. Show validation hint (BPF syntax is validated by libpcap on capture start, but show format examples).

### 18.3 Capture Settings

Tab through settings:
- Snaplen: default 65535 (editable number input)
- Promiscuous mode: toggle with Space
- Buffer size: default 1MB

### 18.4 Privilege Check

Before starting capture, run `PrivilegeCheck`. If insufficient:

```
⚠ Insufficient permissions for packet capture.
  Run with sudo or add user to 'pcap' group.
  See: https://probe.dev/docs/capture-permissions
```

## Key Files and Context

- `crates/prb-capture/src/interfaces.rs` — `InterfaceEnumerator`, `InterfaceInfo`
- `crates/prb-capture/src/config.rs` — `CaptureConfig`
- `crates/prb-capture/src/privileges.rs` — `PrivilegeCheck`

## Exit Criteria

1. **Interface picker:** Shows available interfaces with status and addresses
2. **BPF filter:** Text input for BPF filter expression
3. **Settings:** Snaplen and promiscuous mode configurable
4. **Privilege check:** Warning shown if insufficient permissions
5. **Tests pass:** `cargo nextest run -p prb-tui`
6. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
