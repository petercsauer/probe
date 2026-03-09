---
id: "S1-4"
title: "JSON Fixture Format Not Defined"
risk: 2/10
addressed_by_segments: [2]
---
# Issue S1-4: JSON Fixture Format Not Defined

**Core Problem:**
The parent plan says "JSON fixture adapter" and "a working end-to-end pipeline: JSON fixture -> DebugEvent -> CLI output" but does not specify what a fixture file looks like. The format must be stable because users will author fixtures for testing and development, and the format is the first thing users interact with.

**Root Cause:**
The parent plan focused on the adapter as a code artifact, not on its input format.

**Proposed Fix:**
Define a versioned JSON fixture format:

```json
{
  "version": 1,
  "description": "Optional human-readable description of the fixture",
  "events": [
    {
      "timestamp_ns": 1709913600000000000,
      "transport": "grpc",
      "direction": "inbound",
      "payload_base64": "AAAAABIKCAESB...",
      "metadata": {
        "grpc.method": "/myservice.v1.MyService/GetItem",
        "h2.stream_id": "1"
      }
    },
    {
      "timestamp_ns": 1709913600100000000,
      "transport": "raw_tcp",
      "direction": "outbound",
      "payload_utf8": "HTTP/1.1 200 OK\r\n...",
      "source": {
        "src_addr": "10.0.0.1",
        "src_port": 8080,
        "dst_addr": "10.0.0.2",
        "dst_port": 45321
      }
    }
  ]
}
```

Design decisions:
- `version` field for forward compatibility
- Timestamps are nanoseconds (matches MCAP log_time precision)
- Payload has two representations: `payload_base64` for binary, `payload_utf8` for text (exactly one required)
- `metadata` uses dotted-prefix keys matching well-known constants from prb-core (`grpc.method`, `h2.stream_id`, `zmq.topic`, `dds.domain_id`)
- `source` is optional (fixtures may not have network addresses)
- `transport` uses snake_case enum variant names

**Existing Solutions Evaluated:**
- CloudEvents JSON format (CNCF): uses `data_base64` for binary payloads and `data` for structured payloads. Our format adopts this same dual-representation pattern.
- Wireshark's `-T json` export format: outputs packet dissection as nested JSON. Too complex for hand-authored fixtures.
- MCAP's JSON-based schema support: MCAP supports schemaless JSON messages (schema_id=0). Our fixture format is simpler because it's a flat event list, not an MCAP container.

**Alternatives Considered:**
- Use MCAP files as fixtures. Rejected: MCAP is a binary format; hand-authoring fixtures requires a tool that doesn't exist yet (circular dependency).
- Use YAML instead of JSON. Rejected: adds a dependency (serde_yaml); JSON is sufficient and universally supported. YAML can be added later if requested.
- Use newline-delimited JSON (NDJSON). Rejected: harder to hand-edit (no outer structure for version/description). Standard JSON is better for small fixture files.

**Pre-Mortem -- What Could Go Wrong:**
- Base64 encoding is error-prone for hand-authored fixtures. Mitigation: the `payload_utf8` alternative avoids base64 for text-based protocols; provide example fixtures with both.
- The format may need fields not yet anticipated (e.g., schema references for Subsection 2). Mitigation: the `version` field enables non-breaking format evolution; unknown fields are ignored by serde's `#[serde(deny_unknown_fields)]` at the event level (strict) or skipped (lenient).
- Users may submit invalid JSON (trailing commas, comments). Mitigation: use `serde_json` strict parsing; provide clear error messages citing line/column.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: CloudEvents JSON format (CNCF v1.0.2) uses the same dual-payload pattern (`data` for structured, `data_base64` for binary), validating this approach for event serialization.
- External evidence: JSON Schema specification (json-schema.org) recommends versioned schemas for forward compatibility; our `version` field follows this practice.

**Blast Radius:**
- Direct changes: `crates/prb-fixture/src/` (adapter implementation), `fixtures/` (sample files)
- Potential ripple: documentation, README examples, integration tests
