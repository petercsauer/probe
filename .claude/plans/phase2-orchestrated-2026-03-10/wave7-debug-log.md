# Wave 7 Debug Log

## Issue: Sequential Path Implementation

**Date:** 2026-03-10
**Status:** Partially Fixed - Deeper Issue Found

### Initial Problem

S22 and S23 were blocked because `ParallelPipeline::run_sequential()` was a placeholder returning empty vector.

### Fix Applied

**Files Modified:**
1. `crates/prb-pcap/src/parallel/orchestrator.rs:116-126` - Implemented sequential path using ShardProcessor
2. `crates/prb-pcap/src/parallel/shard.rs:59` - Made `process_single_shard()` public
3. `crates/prb-pcap/benches/fixtures/pcap_gen.rs:120` - Removed unused import

**Implementation:**
```rust
fn run_sequential(&self, packets: Vec<OwnedNormalizedPacket>) -> Result<Vec<DebugEvent>, CoreError> {
    let shard_processor = ShardProcessor::new(Arc::clone(&self.tls_keylog), self.capture_path.clone());
    let events = shard_processor.process_single_shard(packets);
    Ok(events)
}
```

### Build Status

✅ `cargo build --bin prb` succeeds
✅ Benchmarks compile without errors

### Remaining Issue

After implementing the sequential path, PCAP ingest still produces 0 events:

```
RUST_LOG=info ./target/debug/prb ingest /tmp/test.pcap
[INFO] Read 1 packets from capture
[INFO] Normalized 1 packets  
[INFO] Sequential processing complete: 0 events  ← Still empty!
```

### Root Cause Analysis

The sequential path is now correctly calling `ShardProcessor::process_single_shard()`, but the TCP reassembler is not emitting events. Investigation shows:

1. **TCP Packet Structure:** Test packets have FIN+ACK+PSH flags but no SYN
2. **Reassembly Logic:** Reassembler accepts packets without SYN (uses or_insert_with)
3. **Flush Behavior:** `flush_all()` only emits events if `bytes_buffered > 0`
4. **Suspected Issue:** Payload is being processed but `bytes_buffered` remains 0

**Possible Causes:**
- TCP assembler may be rejecting packets for some reason
- Payload offset calculation issue
- Sequence number handling for mid-stream packets
- Missing initialization step

### Test Failures

CLI integration tests still fail (5/35):
- `test_cli_ingest_pcap`
- `test_cli_ingest_pcap_tls`
- `test_cli_format_autodetect`
- `test_cli_ingest_magic_bytes_detection`
- `test_cli_ingest_pcap_to_mcap`

All fail with: `Unexpected stdout, failed var.contains("transport":"raw-tcp")`

### Commits

```bash
git log --oneline -3
30a3213 WIP: CLI Integration + Adaptive Parallelism - cycle 1, 2/2 tests passing
88571c7 feat(prb-pcap): add channel-based streaming pipeline...
fd81cca refactor(prb-pcap): make TLS decrypt thread-safe...
```

Changes are uncommitted (WIP state).

### Next Steps

1. **Debug TCP Reassembly:**
   - Add tracing to see if payload is being stored
   - Check `bytes_buffered` value after processing
   - Verify `flush_all()` is being called and what it returns

2. **Alternative Approach:**
   - Use existing `PcapCaptureAdapter` for sequential path
   - This is the proven working code from Phase 1

3. **Testing:**
   - Create unit test for single-packet TCP processing
   - Verify with real PCAP from tests/fixtures

### Files to Review

- `crates/prb-pcap/src/tcp.rs:240-414` - TCP segment processing
- `crates/prb-pcap/src/tcp.rs:460-484` - flush_all() implementation  
- `crates/prb-pcap/src/parallel/shard.rs:59-103` - Shard processing

### Context for Resume

The sequential path implementation is correct architecturally, but there's a deeper issue with TCP event generation that needs investigation. The parallel path (`run_parallel()`) may have the same issue but hasn't been tested with real data yet.

**Recommendation:** Use `PcapCaptureAdapter` for sequential fallback instead of ShardProcessor until TCP processing is debugged.
