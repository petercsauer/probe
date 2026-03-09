---
segment: 4
title: "DDS/RTPS Correlation Strategy"
depends_on: [1]
risk: 7/10
complexity: High
cycle_budget: 20
status: pending
commit_message: "feat(correlation): add DDS/RTPS two-phase correlation with discovery cache"
---

# Segment 4: DDS/RTPS Correlation Strategy

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement DDS correlation with two-phase discovery cache (SEDP extraction followed by GUID-to-topic mapping) and data message correlation.

**Depends on:** Segment 1

## Context: Issues Addressed

**S5-4: DDS Correlation -- Two-Phase Discovery Cache**

- **Core Problem:** Topic name and type name appear ONLY in SEDP DATA submessages, not in regular RTPS user data messages. Regular DATA submessages contain only writer/reader GUID. Correlation requires first building a discovery cache, then looking up topic names by GUID.
- **Proposed Fix:** Two-phase correlation: (A) Scan events for SPDP/SEDP submessages, extract writer/reader GUID, topic_name, type_name, domain_id; build `HashMap<GUID, DiscoveryInfo>`. (B) For each RTPS DATA event, look up writer GUID in cache; correlation key = `(domain_id, topic_name)` if found, else GUID-only fallback. Graceful degradation: if no discovery events, emit warning and fall back to GUID-only. Domain ID inference: from PID_DOMAIN_ID or from UDP port formula `port = 7400 + 250 * domain_id`.
- **Pre-Mortem risks:** Discovery traffic may be in separate pcap; RTPS vendor-specific extensions may cause parsing failures (use lenient parsing); two-pass doubles memory traversal; sequence number matching assumes reliable mode (best-effort lacks ACKNACKs).

## Scope

- `crates/prb-correlation/src/dds.rs` -- `DdsCorrelationStrategy` with discovery cache

## Key Files and Context

- `crates/prb-correlation/src/dds.rs` -- new file
- `crates/prb-correlation/src/engine.rs` -- register DDS strategy
- `crates/prb-core/src/event.rs` -- DebugEvent must carry from Subsection 4's DDS/RTPS decoder:
  - `guid_prefix: Option<[u8; 12]>` -- RTPS GUID prefix (host_id + app_id + instance_id)
  - `entity_id: Option<[u8; 4]>` -- RTPS entity ID (3 bytes object ID + 1 byte entity kind)
  - `rtps_submessage_kind: Option<RtpsSubmessageKind>` -- enum: DATA, HEARTBEAT, ACKNACK, INFO_TS, GAP, etc.
  - `sequence_number: Option<u64>` -- writer sequence number in DATA submessages
  - `domain_id: Option<u32>` -- extracted from PID_DOMAIN_ID in SPDP discovery or inferred from port
  - `topic_name: Option<String>` -- ONLY populated for SEDP discovery events (not regular data)
  - `type_name: Option<String>` -- ONLY populated for SEDP discovery events

RTPS discovery architecture (OMG DDS-RTPS v2.5):
- **SPDP:** Announces DomainParticipants. Contains participant GUID prefix, domain_id, locators.
- **SEDP:** Announces DataWriters and DataReaders. Contains: writer/reader GUID, topic_name, type_name, QoS. Well-known entity IDs: `0x000100c2` (publications writer), `0x000100c7` (subscriptions writer).
- Regular DATA submessages contain only the writer GUID and sequence number. Topic name must be resolved from the discovery cache.

Domain ID inference when not in discovery data: `domain_id = (port - 7400) / 250` from the UDP destination port per RTPS spec formula.

## Implementation Approach

Two-pass correlation:

**Pass 1 -- Build Discovery Cache:**
1. Iterate all events once. For events with `rtps_submessage_kind == SEDP_DATA`:
   - Extract writer/reader GUID (prefix + entity_id)
   - Extract topic_name, type_name from event metadata
   - Store in `HashMap<RtpsGuid, DiscoveryInfo>` where `DiscoveryInfo = { topic_name, type_name, domain_id }`
2. For SPDP events, extract domain_id and participant GUID prefix. Store in participant map.

**Pass 2 -- Correlate Data Messages:**
1. For each RTPS DATA event with a writer GUID:
   - Look up writer GUID in discovery cache to get topic_name
   - If found: correlation key = `CorrelationKey::Dds { domain_id, topic_name }`
   - If not found: correlation key = `CorrelationKey::DdsUnresolved { guid_prefix, entity_id }` (GUID-only fallback)
2. Within a topic flow, events are ordered by timestamp. Optionally match DataWriter sequence numbers to DataReader ACKNACK ranges for reliable connections.

**Graceful degradation:** If no discovery events are found in the session, emit a `tracing::warn!` and fall back to GUID-only grouping. All data events from the same writer GUID become one flow. Display GUID as hex in flow metadata.

## Alternatives Ruled Out

- Requiring separate discovery dump file from user (poor UX)
- GUID-only grouping without discovery resolution (opaque hex, unusable)
- Full DDS stack (rustdds) for correlation (too heavyweight for passive analysis)

## Pre-Mortem Risks

- Discovery traffic may be in a separate pcap (different multicast group). Without it, all flows show GUID-only. Document this limitation clearly.
- RTPS vendor-specific extensions (RTI, eProsima) in discovery data may have non-standard PIDs that cause parsing errors. Use lenient parsing: skip unknown PIDs.
- Two-pass correlation doubles memory traversal. For 1M events this is still fast (2 sequential scans of memory-mapped data). If performance is a concern, can be optimized to single-pass with deferred resolution in Phase 2.
- Sequence number matching for ACKNACK correlation assumes reliable mode. Best-effort topics lack ACKNACKs entirely. Handle gracefully.

## Build and Test Commands

- Build: `cargo build -p prb-correlation`
- Test (targeted): `cargo nextest run -p prb-correlation -- dds`
- Test (regression): `cargo nextest run -p prb-correlation -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_dds_discovery_cache_from_sedp`: SEDP events build a cache mapping writer GUID to topic_name/type_name.
   - `test_dds_data_resolved_via_cache`: DATA events with writer GUID found in cache produce flows keyed by (domain_id, topic_name).
   - `test_dds_data_unresolved_fallback`: DATA events with GUID not in cache produce GUID-only flows with warning logged.
   - `test_dds_domain_id_from_port`: When domain_id is absent from discovery, infer from UDP port using RTPS formula.
   - `test_dds_no_discovery_warns`: Session with zero discovery events emits tracing warning and falls back to GUID-only.
   - `test_dds_multiple_topics`: Events from different topics produce separate flows, even from same participant.
   - `test_dds_multiple_domains`: Events from different domain IDs produce separate flows, even for same topic name.
2. **Regression tests:** All Segment 1 tests and existing workspace tests pass.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changes only in `crates/prb-correlation/src/dds.rs` and strategy registration in `engine.rs`.
