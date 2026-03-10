---
issue: 1
title: "ZMTP and RTPS not covered by `guess` crate"
severity: Medium
segments_affected: [1]
status: mitigated
---

# Issue 1: ZMTP and RTPS Not Covered by `guess` Crate

## Problem

The `guess` crate (v0.2) supports 20+ protocols (HTTP/2, TLS, SSH, DNS, etc.)
but does **not** include ZMTP (ZeroMQ) or RTPS (DDS). These are niche protocols
specific to messaging middleware, not general internet protocols.

## Impact

If we relied solely on `guess` for detection, ZMQ and DDS streams would never
be identified and would always fall back to `RawTcp`/`RawUdp`. This defeats
the purpose of auto-detection for two of Probe's three core protocols.

## Mitigation (already incorporated in Segment 1)

Custom detectors for both protocols are planned in Segment 1:

1. **`ZmtpDetector`**: Detects the ZMTP 3.x greeting signature (`0xFF` at byte 0,
   `0x7F` at byte 9). This is a unique and reliable magic-byte pattern.

2. **`RtpsDetector`**: Detects the RTPS header magic bytes (`"RTPS"` at bytes 0-3).
   This is a definitive identifier — no other protocol starts with "RTPS".

Both custom detectors run alongside the `guess`-backed detector in the
`DetectionEngine`. The layered detection model means `guess` handles common
protocols while custom detectors handle domain-specific ones.

## Residual Risk

- **Low**: ZMTP and RTPS have very distinctive signatures. False positive rate
  is essentially zero for normal traffic.
- **Edge case**: If a ZMTP connection starts mid-stream (no greeting captured),
  detection will fail. See Issue 2 (mid-stream detection).

## Future: Contributing to `guess`

Consider contributing ZMTP and RTPS detection to the `guess` crate upstream.
This benefits the broader Rust ecosystem and reduces maintenance burden.
