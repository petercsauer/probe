---
id: "S5-7"
title: "MCAP Message Filtering for Replay"
risk: 3/10
addressed_by_segments: [5]
---
# Issue S5-7: MCAP Message Filtering for Replay

**Core Problem:**
The Rust MCAP crate's `MessageStream` iterates all messages in file order. To implement `prb replay --filter 'transport=grpc'`, the engine must read every message and filter in application code. For a 1M-event session where only 1% match, this means scanning 99% of events unnecessarily.

**Root Cause:**
The MCAP Rust API does not expose topic/channel-level filtering. The MCAP format itself supports a summary section with channel listings, but the Rust API does not use it for selective reading.

**Proposed Fix:**
1. **Channel pre-filtering:** Before iterating messages, read the MCAP summary section to get the channel list. Map channels to transport types using channel metadata (topic names contain transport prefix, e.g., `/grpc/...`, `/zmq/...`). Build a `HashSet<u16>` allowlist of channel IDs.
2. **Per-message filtering:** During iteration, skip messages whose `channel_id` is not in the allowlist. O(1) per message.
3. **Time-range filtering:** Accept `--start` and `--end` timestamp flags. Skip messages outside the range. MCAP stores messages in timestamp order within chunks, enabling efficient skip-ahead.
4. **Filter syntax:** Simple key-value pairs: `transport=grpc`, `topic=/my/topic`, `flow=<flow_id>`. No regex for Phase 1.

**Existing Solutions Evaluated:**
- MCAP CLI has a `filter` subcommand (PR #445 in foxglove/mcap) that filters by topic regex and time range. Validates the architectural approach.
- MCAP Go library's `readMessages()` accepts topic filter options. The Rust API lacks this, confirming application-level implementation is required.

**Alternatives Considered:**
- Build a secondary index file alongside MCAP for fast lookups. Rejected for Phase 1: adds file management complexity. Suitable for Phase 2 with a `.prb-index` sidecar.
- Use chunk-level statistics to skip entire chunks lacking matching channels. Rejected for Phase 1: requires chunk index parsing and seek. Good Phase 2 optimization.

**Pre-Mortem -- What Could Go Wrong:**
- Channel metadata schema varies by how events were written in Subsection 2. If naming convention does not encode transport type, channel pre-filtering fails. Must coordinate with Subsection 2's storage schema.
- MCAP files without a summary section (truncated captures) require full scan with no optimization.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- Existing solutions: MCAP's own CLI filter command uses the same approach (channel-level filtering with time ranges), validated by MCAP maintainers.
- External evidence: MCAP specification documents summary section and channel/statistics records enabling pre-filtering.

**Blast Radius:**
- Direct: MCAP reader wrapper in replay engine
- Ripple: `prb flows` command can reuse the same filtering infrastructure
