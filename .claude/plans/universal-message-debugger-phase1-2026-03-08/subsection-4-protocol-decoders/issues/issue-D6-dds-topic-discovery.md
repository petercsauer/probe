---
id: "D6"
title: "DDS Topic Name Extraction Requires Discovery"
risk: 5/10
addressed_by_segments: [3]
---
# Issue D6: DDS Topic Name Extraction Requires Discovery

**Core Problem:**
RTPS entity IDs are numeric identifiers. Topic names are exchanged via the Simple Endpoint Discovery Protocol (SEDP) at connection establishment using well-known built-in entity IDs. If a capture does not include the initial SEDP exchange, the decoder cannot map entity IDs to topic names, which is the primary correlation metadata for DDS.

**Root Cause:**
RTPS separates naming (discovery) from data transfer (user traffic). This is architecturally different from gRPC where the method name is in every request's HTTP/2 HEADERS.

**Proposed Fix:**
1. Implement an `RtpsDiscoveryTracker` that processes SEDP DATA submessages (sent to well-known entity IDs like `ENTITYID_SEDP_BUILTIN_PUBLICATIONS_WRITER = {0x00,0x00,0x03,0xC2}`) and builds a lookup table: `(GuidPrefix, EntityId) -> TopicName`.
2. When processing user DATA submessages, look up the writer entity in the discovery table.
3. If lookup fails (discovery not observed), display entity ID in hex as fallback.
4. Include `topic_name: Option<String>` in the DebugEvent. Document that topic name resolution requires the capture to include the initial discovery phase.

**Existing Solutions Evaluated:**
- N/A -- internal design. Discovery tracking is domain-specific to the event model.

**Alternatives Considered:**
- Always display entity IDs without attempting name resolution. Rejected: topic names are far more useful for debugging than raw entity IDs.
- Parse ALL RTPS submessages to reconstruct full DDS state. Rejected for Phase 1: massive scope expansion. Focused SEDP parsing is sufficient.

**Pre-Mortem -- What Could Go Wrong:**
- SEDP serialized data uses CDR (Common Data Representation) encoding, not protobuf. Parsing CDR parameter lists adds complexity.
- Multiple DDS domains in the same capture may have overlapping entity IDs. Must scope lookup by GUID prefix.
- SEDP data may be fragmented across multiple DATA_FRAG submessages.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- External evidence: OMG DDSI-RTPS spec (v2.5) defines SEDP as the standard discovery mechanism. Topic names are in `DiscoveredWriterData.topic_name`.
- External evidence: Wireshark's RTPS dissector uses the same approach (tracks discovery to annotate data submessages with topic names).

**Blast Radius:**
- Direct: DDS/RTPS decoder (new discovery tracker module)
- Ripple: DebugEvent (topic_name field), correlation engine in Subsection 5 (can use topic names when available)
