---
id: "S2-2"
title: "Schema Storage and Session Self-Containment"
risk: 3/10
addressed_by_segments: [1, 2]
---

# Issue S2-2: Schema Storage and Session Self-Containment

**Core Problem:**
The plan says "schema storage" but does not specify whether protobuf schemas are stored inside MCAP session files (making them self-contained and shareable) or kept as external files that must accompany the session. For a debug tool, self-contained sessions are critical: users share session files with teammates, and broken external references make sessions useless.

**Root Cause:**
The plan treats schema loading and session storage as orthogonal concerns, but they are tightly coupled: a session without its schemas cannot decode protobuf payloads.

**Proposed Fix:**
Store schemas inside MCAP sessions using MCAP's native schema mechanism:

- When a user loads schemas (via `prb schemas load foo.desc` or `prb schemas load foo.proto`), the schemas are registered in the SchemaRegistry.
- When writing a session, all schemas from the registry that were used during decode are stored as MCAP Schema records with `encoding="protobuf"` and `data=FileDescriptorSet bytes`.
- When reading a session, the reader extracts MCAP Schema records and populates the SchemaRegistry automatically. No external .desc files needed.
- Channels that carry protobuf-encoded application payloads (after Phase 3/4 decoders extract them) reference these schemas by ID.

This means sessions are fully self-contained: open the .mcap file, and all schemas needed to decode its contents are embedded.

Additionally, support a `prb schemas export session.mcap` command that extracts stored schemas for reuse.

**Existing Solutions Evaluated:**
N/A -- internal design decision. MCAP's spec explicitly supports this pattern: schemas are first-class records in the MCAP format (mcap.dev/spec, Section "Schema").

**Alternatives Considered:**

- Store schemas as MCAP Attachments instead of Schema records. Rejected: Schema records are the semantically correct mechanism and integrate with MCAP readers/viewers. Attachments are for arbitrary files.
- Store schemas externally in a sidecar file (e.g., session.mcap.schemas). Rejected: breaks self-containment and creates a coupling between files.

**Pre-Mortem -- What Could Go Wrong:**

- Embedding full FileDescriptorSets (with all imports) can be large (100KB+ for complex service definitions). For sessions with many schemas, this adds overhead. Mitigation: deduplicate schemas by content hash.
- Schema version conflicts: a session might embed schema version A, but the user's current .proto files are version B. The session should always use its embedded schemas for consistency, with an override flag for reinterpretation.

**Risk Factor:** 3/10

**Evidence for Optimality:**

- External evidence: MCAP's format specification defines Schema records specifically for this purpose, with encoding and data fields designed to hold FileDescriptorSets.
- Existing solutions: Foxglove Studio reads schemas from MCAP Schema records to decode protobuf messages, confirming this is the intended usage pattern.

**Blast Radius:**

- Direct changes: SessionWriter (schema embedding), SessionReader (schema extraction), SchemaRegistry (MCAP integration)
- Potential ripple: `prb inspect` (auto-loads schemas from session)
