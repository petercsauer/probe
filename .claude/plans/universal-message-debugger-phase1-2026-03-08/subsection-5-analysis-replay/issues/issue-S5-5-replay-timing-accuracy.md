---
id: "S5-5"
title: "Replay Timing Accuracy Below 1ms"
risk: 3/10
addressed_by_segments: [5]
---
# Issue S5-5: Replay Timing Accuracy Below 1ms

**Core Problem:**
Tokio's timer driver has millisecond resolution. Events spaced less than 1ms apart (common in burst traffic -- gRPC can produce hundreds of messages in microseconds) will replay with incorrect timing. The parent plan identifies this as a risk but proposes no mitigation.

**Root Cause:**
Tokio optimizes for async I/O workloads where 1ms resolution is sufficient. Sub-millisecond timing is not its design target.

**Proposed Fix:**
Implement a hybrid timing strategy:
1. For inter-event gaps >= 1ms: use `tokio::time::sleep` (efficient, yields CPU)
2. For inter-event gaps < 1ms: use `Instant::now()` spin-wait with `std::hint::spin_loop()`
3. For `--speed max` mode: skip all timing, output as fast as possible

```rust
let mut last_ts = events[0].timestamp;
for event in &events {
    let delta = event.timestamp - last_ts;
    let adjusted = delta / speed_multiplier;
    if adjusted >= Duration::from_millis(1) {
        tokio::time::sleep(adjusted).await;
    } else if adjusted > Duration::ZERO {
        let target = Instant::now() + adjusted;
        while Instant::now() < target {
            std::hint::spin_loop();
        }
    }
    output_event(event, &mut writer)?;
    last_ts = event.timestamp;
}
```

**Existing Solutions Evaluated:**
- `sturgeon` (crates.io) -- async stream replay with speed multipliers. Designed for recording/replaying live async streams, not MCAP file playback. Not directly adoptable.
- `crossbeam_utils::Backoff` -- exponential backoff spin-wait. Too coarse for precise sub-ms timing; designed for contention, not time targets.
- `tokio-timerfd` -- Linux-only high-resolution timer using timerfd. macOS unsupported. Rejected for Phase 1 (cross-platform required).

**Alternatives Considered:**
- Always use spin-wait. Rejected: consumes 100% CPU even for long gaps. Unacceptable for multi-minute replays.
- Ignore sub-millisecond timing (round up to 1ms). Rejected: distorts burst patterns which are often the most interesting traffic for debugging.

**Pre-Mortem -- What Could Go Wrong:**
- Spin-wait on a loaded system may overshoot due to OS scheduling delays. This is inherent and should be documented.
- Speed multiplier < 1.0 (slow-motion) amplifies all gaps, making even sub-ms gaps sleepable. Less of a concern.
- Windows timer resolution is ~16ms, making even the 1ms threshold unreliable. Document as a known Windows limitation.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: tcpreplay uses the exact same hybrid approach -- busy-loop polling (rdtsc/gettimeofday) for precise timing, OS sleep as fallback. Documented in tcpreplay wiki and man page.
- External evidence: Linux kernel documentation recommends busy-wait for sub-millisecond delays (`asm volatile("pause")` pattern).

**Blast Radius:**
- Direct: replay engine timing module
- Ripple: none (isolated to replay)
