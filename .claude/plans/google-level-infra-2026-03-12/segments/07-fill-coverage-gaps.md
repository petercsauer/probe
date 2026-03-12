---
segment: 07
title: Fill Coverage Gaps
depends_on: [6]
risk: 5
complexity: High
cycle_budget: 15
estimated_lines: ~1000 new test lines
---

# Segment 07: Fill Coverage Gaps

## Context

Based on the analysis from Segment 06, systematically add tests to bring workspace coverage from current level to 80%+ target. Focus on high-value targets: error paths, edge cases, and critical functionality.

## Current State

- Coverage analysis complete (S06)
- Specific gaps identified per crate/module
- Priority list established

## Goal

Add comprehensive tests to reach 80%+ coverage workspace-wide, with critical crates at 90%+.

## Exit Criteria

1. [ ] prb-core coverage ≥90%
2. [ ] prb-pcap coverage ≥90%
3. [ ] prb-grpc, prb-zmq, prb-dds coverage ≥85%
4. [ ] prb-storage coverage ≥85%
5. [ ] All other crates coverage ≥80%
6. [ ] Total workspace coverage ≥80%
7. [ ] All new tests pass
8. [ ] CI coverage job passes threshold check
9. [ ] Manual review: tests cover error paths and edge cases

## Implementation Plan

This segment will be guided by the handoff from S06. General approach:

### Priority 1: prb-core (Foundation)

Target: 90%+ coverage

**Add tests for:**
- Error handling paths in `src/error.rs`
- Edge cases in `src/conversation.rs` (empty conversations, single events)
- Event correlation edge cases in `src/engine.rs`
- Timestamp/ID ordering edge cases
- Serialization/deserialization edge cases

**Example test additions:**
```rust
// crates/prb-core/src/conversation_tests.rs

#[test]
fn test_empty_conversation_creation() {
    let conv = Conversation::new(ConversationId::new());
    assert_eq!(conv.events().len(), 0);
}

#[test]
fn test_conversation_with_single_event() {
    let mut conv = Conversation::new(ConversationId::new());
    let event = create_test_event(1);
    conv.add_event(event);
    assert_eq!(conv.events().len(), 1);
}

#[test]
fn test_conversation_ordering_with_out_of_order_events() {
    let mut conv = Conversation::new(ConversationId::new());
    let event2 = create_test_event_with_ts(2, 2000);
    let event1 = create_test_event_with_ts(1, 1000);
    conv.add_event(event2);
    conv.add_event(event1);
    // Should maintain temporal order
    assert_eq!(conv.events()[0].id, EventId::from_raw(1));
}
```

### Priority 2: prb-pcap (Critical Path)

Target: 90%+ coverage

**Add tests for:**
- TCP reassembly error paths in `src/tcp.rs`
- TLS decryption failures in `src/tls/decrypt.rs`
- Malformed packet handling in `src/normalize.rs`
- PCAP format edge cases in `src/reader.rs`
- Parallel pipeline error handling in `src/parallel/orchestrator.rs`

**Example test additions:**
```rust
// crates/prb-pcap/src/tcp_tests.rs

#[test]
fn test_tcp_reassembly_with_missing_segments() {
    let mut reassembler = TcpReassembler::new();
    // Add segment 1 and 3, missing segment 2
    reassembler.add_segment(create_segment(1, 1000, b"part1"));
    reassembler.add_segment(create_segment(3, 3000, b"part3"));
    // Should buffer until segment 2 arrives
    assert!(reassembler.pending_segments() > 0);
}

#[test]
fn test_tls_decrypt_with_unknown_cipher() {
    let keylog = TlsKeyLog::new();
    let encrypted = create_encrypted_packet(CipherSuite::Unknown);
    let result = decrypt_tls_record(&encrypted, &keylog);
    assert!(result.is_err());
}
```

### Priority 3: Protocol Decoders

Target: 85%+ each

**prb-grpc:**
- HTTP/2 frame parsing edge cases
- gRPC status code handling
- Protobuf field decoding errors
- Stream multiplexing edge cases

**prb-zmq:**
- Socket type variations
- Multi-part message handling
- Identity frame edge cases
- Invalid frame handling

**prb-dds:**
- RTPS header parsing errors
- Domain ID edge cases
- QoS parameter variations
- Participant discovery edge cases

### Priority 4: Remaining Crates

Target: 80%+ each

Add tests for uncovered functions and error paths in:
- prb-storage - MCAP read/write errors
- prb-query - Query parser edge cases
- prb-export - Export format errors
- prb-ai - API error handling
- prb-capture - Permission errors
- prb-cli - Command validation
- prb-plugin-* - Plugin loading errors

## Files to Modify

Based on S06 analysis, expect to modify:
- `crates/prb-core/src/*_tests.rs` (~200 lines)
- `crates/prb-pcap/tests/*.rs` (~300 lines)
- `crates/prb-grpc/tests/*.rs` (~150 lines)
- `crates/prb-zmq/tests/*.rs` (~100 lines)
- `crates/prb-dds/tests/*.rs` (~100 lines)
- Other crate test files (~150 lines)

Total: ~1000 new test lines

## Test Plan

1. Read handoff from S06 with specific gaps
2. Start with prb-core:
   ```bash
   cargo llvm-cov -p prb-core --html
   ```
3. Add tests for identified gaps
4. Verify coverage increases:
   ```bash
   cargo llvm-cov -p prb-core --summary-only
   ```
5. Repeat for each crate in priority order
6. Run full workspace coverage:
   ```bash
   cargo llvm-cov --workspace --summary-only
   ```
7. Verify ≥80% total coverage
8. Run all tests to ensure they pass:
   ```bash
   cargo test --workspace
   ```
9. Commit: "test: Increase coverage to 80%+ across workspace"

## Blocked By

- Segment 06 (Coverage Analysis) - must know what gaps to fill

## Blocks

None - but enables CI coverage threshold to pass.

## Success Metrics

- Workspace coverage ≥80%
- Critical crates (core, pcap) ≥90%
- All new tests pass
- CI coverage job passes
- No test flakiness introduced

## Notes

- This is the largest segment by effort (~15 cycles)
- Break work into sub-tasks by crate
- Focus on meaningful tests, not just coverage numbers
- Prioritize error paths and edge cases over happy path duplication
- Use property testing (proptest) where appropriate
- Consider splitting into multiple smaller segments if needed
- Some uncovered code may be legitimately untestable (mark with #[cfg(not(coverage))])
