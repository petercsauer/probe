---
id: "1"
title: "Scattered pure-logic unit test gaps across crates"
risk: 2/10
addressed_by_segments: [1]
---

# Issue 1: Scattered pure-logic unit test gaps across crates

## Core Problem

Multiple crates have untested pure-logic code paths that require zero external dependencies: `CaptureStatsInner::new/snapshot` (prb-capture/stats.rs), `CaptureConfig::with_snaplen` (config.rs), `PrivilegeCheck::check/status` on macOS (privileges.rs), `CaptureError::Pcap/Other` Display impls (error.rs), `detect_format` magic-byte branches (prb-cli/ingest.rs, prb-tui/loader.rs), and minor edge cases in prb-dds/prb-decode/prb-pcap. These are low-hanging fruit — all testable with existing infrastructure.

## Root Cause

These paths were simply never added to the test suite. No structural barrier prevents testing.

## Proposed Fix

Add targeted unit tests for each gap. No refactoring, no new dependencies.

## Existing Solutions Evaluated

N/A — internal test additions. No external tools address this.

## Pre-Mortem

- Risk is near-zero: pure assertions on existing public/crate-visible APIs.
- Only concern: `CaptureStatsInner` is `pub(crate)` — tests must be inside the crate module or use `#[cfg(test)]` inline tests.

## Risk Factor: 2/10

Isolated additions, well-tested areas, clear implementation.

## Blast Radius

- Direct: test files in prb-capture, prb-cli, prb-tui, prb-dds, prb-decode, prb-pcap
- Ripple: None
