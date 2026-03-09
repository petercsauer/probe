---
id: "S5-4"
title: "DDS Correlation -- Two-Phase Discovery Cache"
risk: 7/10
addressed_by_segments: [4]
---
# Issue S5-4: DDS Correlation -- Two-Phase Discovery Cache

**Core Problem:**
The parent plan says "correlate by (domain ID, topic name, GUID prefix)" implying topic name is available in every RTPS message. It is not. Topic name and type name appear ONLY in SEDP (Simple Endpoint Discovery Protocol) DATA submessages -- specifically in `PublicationBuiltinTopicData` and `SubscriptionBuiltinTopicData`. Regular user data messages contain only the writer/reader GUID (prefix + entity ID). Correlation requires first building a discovery cache, then looking up topic names by GUID.

**Root Cause:**
The parent plan does not distinguish between RTPS discovery traffic and RTPS user data traffic. These carry fundamentally different information.

**Proposed Fix:**
Implement two-phase DDS correlation:

**Phase A -- Discovery Cache:**
1. Scan events for RTPS SPDP/SEDP submessages (well-known entity IDs: `0x000100c2` for SEDP publications, `0x000100c7` for subscriptions)
2. Extract: writer/reader GUID, topic name, type name, domain ID, QoS parameters
3. Build `HashMap<GUID, DiscoveryInfo>` mapping each writer/reader to its topic

**Phase B -- Data Correlation:**
1. For each RTPS DATA submessage, extract the writer GUID
2. Look up writer GUID in discovery cache to get topic name
3. Correlation key: `(domain_id, topic_name)` groups all writers and readers for the same topic
4. Within a topic, match DataWriter events to DataReader ACKNACKs by sequence number

**Graceful degradation:** If discovery traffic is missing, emit a warning and fall back to GUID-only correlation without topic names. Display GUID prefix as hex string.

**Domain ID inference:** Extract from `PID_DOMAIN_ID` in discovery messages, or infer from UDP destination port using RTPS spec formula: `port = 7400 + 250 * domain_id + offset`.

**Existing Solutions Evaluated:**
- `rtps-parser` (crates.io, v0.1.1) -- passive RTPS message parser extracted from Dust DDS. Suitable for Subsection 4's RTPS parsing.
- `rustdds` (v0.11.8) -- full DDS implementation with RTPS. Too heavyweight for passive correlation.
- `ddshark` (GitHub: NEWSLabNTU/ddshark) -- RTPS monitoring tool. Reference for discovery information extraction.
- RTI Wireshark RTPS dissector docs -- reference for GUID filtering and topic correlation approach.

**Alternatives Considered:**
- Require users to provide a DDS discovery dump file separately. Rejected: poor UX; if discovery traffic is in the capture, extract it automatically.
- Skip topic-level correlation, only show GUID-based grouping. Rejected: GUIDs are opaque hex strings; topic names are essential for usability.

**Pre-Mortem -- What Could Go Wrong:**
- Discovery traffic may be in a separate pcap (common when data and discovery use different multicast groups). Without discovery, topic names unknown.
- RTPS vendorId-specific extensions in discovery data may cause parsing failures for non-standard DDS implementations.
- Sequence number matching assumes RTPS reliable mode. Best-effort connections lack ACKNACKs.
- RTPS fragment reassembly (large messages) should be handled by Subsections 3-4. If not, large discovery messages may be incomplete.

**Risk Factor:** 7/10

**Evidence for Optimality:**
- External evidence: RTI Wireshark documentation describes enabling "Topic Information" to map DataWriter GUIDs to topics via discovery traffic -- the exact pattern proposed here.
- External evidence: OMG DDS-RTPS v2.5 specification defines SEDP entity IDs and the discovery protocol for writer/reader matching.

**Blast Radius:**
- Direct: DDS correlation strategy in `prb-correlation`
- Ripple: requires DebugEvent to carry RTPS-specific metadata (GUID prefix, entity ID, submessage type, sequence number) from Subsection 4
