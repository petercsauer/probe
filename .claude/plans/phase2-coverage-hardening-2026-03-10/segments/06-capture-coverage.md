---
segment: 6
title: "prb-capture Mock Tests"
depends_on: []
risk: 5
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(prb-capture): add unit tests with mock pcap for capture engine and adapter"
---

# Segment 6: prb-capture Mock Tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Bring prb-capture from ~35% to ≥90% line coverage by testing capture logic with mock pcap interfaces.

**Depends on:** None

## Coverage Gaps

| File | Lines | Current | Target | Deficit |
|------|-------|---------|--------|---------|
| prb-capture/src/capture.rs | 161 | 9% | 90% | ~130 |
| prb-capture/src/adapter.rs | 130 | 40% | 90% | ~65 |
| prb-capture/src/stats.rs | 53 | 42% | 90% | ~25 |
| prb-capture/src/interfaces.rs | 95 | 85% | 90% | ~4 |
| prb-capture/src/privileges.rs | 7 | 0% | 90% | ~6 |

## Scope

- `crates/prb-capture/src/capture.rs` — CaptureEngine, packet loop, channel send
- `crates/prb-capture/src/adapter.rs` — LiveCaptureAdapter
- `crates/prb-capture/src/stats.rs` — CaptureStats tracking
- `crates/prb-capture/src/interfaces.rs` — Network interface enumeration
- `crates/prb-capture/src/privileges.rs` — Privilege check (small)

## Implementation Approach

### capture.rs (9% → 90%)
The capture engine uses `pcap` crate for live capture. Testing requires:
- Test `OwnedPacket::from_pcap` with synthetic packet data
- Test `CaptureConfig` validation (interface name, BPF filter syntax)
- Test channel behavior: construct OwnedPackets, send through bounded channel, verify received
- Test stats accumulation: packet count, byte count, dropped count
- Test stop signaling via AtomicBool
- For the actual pcap loop: use `pcap::Capture::from_file` with a test .pcap fixture as a stand-in

### adapter.rs (40% → 90%)
- Test `LiveCaptureAdapter::new` construction
- Test `ingest` iteration over captured packets
- Test pipeline integration: adapter → normalization → decode
- Test error propagation

### stats.rs (42% → 90%)
- Test `CaptureStats::new`, `record_packet`, `record_drop`
- Test `Display` impl formatting
- Test atomic counter increments are correct

### interfaces.rs (85% → 90%)
- Test `list_interfaces` returns at least loopback
- Test interface name validation

### privileges.rs (0% → 90%)
- Small file (7 lines). Test the privilege check function returns a reasonable result on the test platform.

## Pre-Mortem Risks

- Live pcap capture cannot be tested without root/capabilities — use file-based capture or mock the pcap handle
- The `pcap` crate's `Capture` type may not be easily mockable — test around it by exercising the data flow functions

## Build and Test Commands

- Build: `cargo check -p prb-capture`
- Test (targeted): `cargo nextest run -p prb-capture`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** `cargo nextest run -p prb-capture` — all new tests pass
2. **Coverage gate:** Every file ≥ 85% line coverage (privileges.rs excepted if <10 lines)
3. **Regression tests:** `cargo nextest run --workspace` — no regressions
4. **Full build gate:** `cargo build --workspace`
5. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
6. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
7. **Scope verification gate:** Only prb-capture test and source files modified
