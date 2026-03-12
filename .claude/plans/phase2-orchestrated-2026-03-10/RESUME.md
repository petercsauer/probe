# Phase 2 Orchestration - Resume Point

**Last Updated:** 2026-03-10 04:10 AM
**Status:** Suspended - Needs Manual Investigation

---

## Quick Status

**21 of 29 segments complete (72%)**

| Status | Count | Details |
|--------|-------|---------|
| ✅ Complete | 21 | All TUI, Core, Export, OTel + most Capture/Parallel/Detect |
| ❌ Blocked | 5 | S05 (AI API), S22/S23 (TCP issue), S27/S28 (debugging) |
| ⏳ Waiting | 1 | S29 (waiting on S27/S28) |
| 🔍 Debugging | 2 | S15, S27, S28 (debuggers launched earlier) |

---

## Wave 7 Progress Report

### What Was Accomplished

**Sequential Path Implementation:**
✅ Implemented `ParallelPipeline::run_sequential()` (was placeholder)
✅ Made `ShardProcessor::process_single_shard()` public
✅ Fixed benchmark clippy error (unused import)
✅ Build succeeds: `cargo build --workspace`
✅ Benchmarks compile: `cargo build --benches -p prb-pcap`

**Files Modified (Uncommitted):**
- `crates/prb-pcap/src/parallel/orchestrator.rs` - Sequential implementation
- `crates/prb-pcap/src/parallel/shard.rs` - Public method
- `crates/prb-pcap/benches/fixtures/pcap_gen.rs` - Import fix

### Critical Issue Found

**PCAP ingest produces 0 events** despite:
- Packets being read (1 packet read)
- Packets being normalized (1 packet normalized)
- Sequential processing running (`process_single_shard` called)
- Flush being called at end of shard

**Diagnosis:**
TCP reassembler is not emitting events. Investigation shows bytes_buffered likely remains 0, preventing flush from generating output.

**Test Failures:** 5/35 CLI integration tests fail
- All PCAP-related tests expect `"transport":"raw-tcp"` but get empty output

---

## Recommended Next Steps

### Option A: Quick Fix (30 min)
Use existing `PcapCaptureAdapter` for sequential path:

```rust
fn run_sequential(&self, packets: Vec<OwnedNormalizedPacket>) -> Result<Vec<DebugEvent>, CoreError> {
    // Fall back to proven Phase 1 code
    let mut adapter = PcapCaptureAdapter::new(self.capture_path.clone(), None);
    let events: Result<Vec<_>, _> = adapter.ingest().collect();
    events.map_err(|e| CoreError::Adapter(format!("{}", e)))
}
```

This bypasses the parallel pipeline for small files and uses the working Phase 1 implementation.

### Option B: Debug TCP Processing (2-3 hours)
1. Add tracing to TCP reassembler to see where payload goes
2. Check if `bytes_buffered` is being incremented
3. Verify `flush_all()` behavior with single-packet streams
4. Create unit test: `test_tcp_single_packet_with_fin()`

### Option C: Accept Partial Completion
- Mark S22/S23 as "needs-design-revision"
- Document that parallel pipeline needs TCP processing fixes
- Focus on unblocking S15, S27, S28 (debugger sessions)
- Achieve 73% completion (21/29) and move to Phase 3

---

## Debugging Resources

**Logs:**
- `.claude/plans/phase2-orchestrated/execution-log.md` - Full session log
- `.claude/plans/phase2-orchestrated/wave7-debug-log.md` - TCP investigation details

**Key Files:**
- `crates/prb-pcap/src/tcp.rs:240-484` - TCP reassembly logic
- `crates/prb-pcap/src/parallel/shard.rs` - Shard processing
- `crates/prb-cli/tests/integration.rs:360-426` - Test PCAP helper

**Test Command:**
```bash
cargo nextest run -p prb-cli --no-fail-fast
# Should see 13/35 passing currently
# After fix: expect 18/35 passing
```

---

## Other Blocked Segments

### S05: AI-Powered Explanation
**Issue:** async-openai v0.33 API breaking changes
**Solution:** Downgrade to v0.20 or migrate to new API

### S15: TUI Live Mode  
**Issue:** DecoderRegistry has `Rc<dyn Decoder>` which is !Send
**Status:** Debugger launched 2026-03-10 12:50:00Z (check status)

### S27/S28: Plugin Systems
**Issue:** Core API mismatches (CorrelationKey, DebugEventBuilder, Payload, Direction)
**Status:** Debuggers launched 2026-03-10 12:50:00Z (check status)

### S29: Plugin CLI
**Blocked on:** S27 and S28 completion

---

## Build Health

- ✅ Workspace builds cleanly
- ✅ 125/125 prb-pcap tests pass
- ❌ 13/35 prb-cli tests pass (5 PCAP tests fail, 17 skipped)
- ⚠️ WIP commit exists with partial work

---

## Context for Next Session

**The architectural implementation is correct.** The sequential path now properly delegates to ShardProcessor with a single shard. The issue is in the TCP reassembly layer, which either:

1. Isn't buffering payload bytes correctly
2. Isn't emitting events on flush for mid-stream packets
3. Has a condition we haven't identified yet

The fastest path forward is Option A (use PcapCaptureAdapter), which takes 30 minutes and unblocks S22/S23 immediately. Option B (debug TCP) is more thorough but time-consuming.

**Decision needed:** Quick fix vs. thorough debugging?
