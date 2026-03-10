# Protocol Support

PRB decodes three protocol families from raw network traffic. This document covers what gets decoded, the metadata produced, and protocol-specific options.

## gRPC / HTTP/2

PRB fully decodes gRPC-over-HTTP/2, including:

- HTTP/2 frame parsing (HEADERS, DATA, RST_STREAM, GOAWAY, SETTINGS, WINDOW_UPDATE, PING)
- HPACK header decompression
- gRPC length-prefixed message extraction
- Protobuf payload decoding (wire-format or schema-backed)
- gRPC status and trailing metadata
- gRPC-Web support

### Metadata Keys

| Key | Description | Example |
|-----|-------------|---------|
| `grpc.method` | Full gRPC method path | `/api.v1.Users/GetUser` |
| `h2.stream_id` | HTTP/2 stream identifier | `1` |
| `grpc.status` | gRPC status code | `0` (OK) |

### Correlation

gRPC events are correlated by HTTP/2 stream ID within a TCP connection, grouping request and response messages into conversations.

### Usage

```bash
# Auto-detected on port 50051 and via HTTP/2 connection preface
prb ingest grpc-traffic.pcap

# Force gRPC detection on non-standard ports
prb ingest traffic.pcap --protocol grpc

# With protobuf schemas for rich decoding
prb schemas load service.proto -I ./protos
prb ingest grpc-traffic.pcap
```

### Wire Format Decoding

Even without `.proto` schemas, PRB performs best-effort wire-format decoding of protobuf payloads, showing field numbers and inferred types. With schemas, field names and enum values are resolved.

## ZeroMQ / ZMTP

PRB decodes ZMTP (ZeroMQ Message Transport Protocol), including:

- ZMTP greeting (version negotiation)
- ZMTP handshake (mechanism: NULL, PLAIN, CURVE)
- Socket type identification (REQ, REP, PUB, SUB, PUSH, PULL, DEALER, ROUTER, PAIR)
- Message frames (single and multi-part)
- Command frames (READY, SUBSCRIBE, CANCEL)
- Topic extraction for PUB/SUB patterns

### Metadata Keys

| Key | Description | Example |
|-----|-------------|---------|
| `zmq.topic` | PUB/SUB topic name | `market.prices` |
| `zmq.socket_type` | Socket type | `PUB` |
| `zmq.identity` | Socket identity | `worker-01` |

### Correlation

ZMQ events are correlated by topic name (for PUB/SUB) or by connection ID (for REQ/REP and other patterns).

### Usage

```bash
# Auto-detected via ZMTP greeting magic bytes (0xFF...0x7F)
prb ingest zmq-traffic.pcap

# Force ZMTP detection
prb ingest traffic.pcap --protocol zmtp
```

## DDS / RTPS

PRB decodes the Real-Time Publish-Subscribe (RTPS) wire protocol used by DDS implementations (RTI Connext, Eclipse Cyclone DDS, OpenDDS, FastDDS):

- RTPS header parsing (protocol version, vendor ID, GUID prefix)
- Submessage decoding (DATA, HEARTBEAT, ACKNACK, GAP, INFO_TS, INFO_DST, INFO_SRC)
- Serialized data payload extraction
- Participant and endpoint discovery
- QoS parameter parsing

### Metadata Keys

| Key | Description | Example |
|-----|-------------|---------|
| `dds.domain_id` | DDS domain identifier | `0` |
| `dds.topic_name` | DDS topic name | `VehiclePosition` |
| `dds.participant_guid` | Participant GUID prefix | `01.0f.44.23` |

### Correlation

DDS events are correlated by topic name within a domain, grouping publishers and subscribers on the same topic.

### Usage

```bash
# Auto-detected via RTPS magic bytes ("RTPS") on UDP
prb ingest dds-traffic.pcap

# Force RTPS detection
prb ingest traffic.pcap --protocol rtps
```

## Protocol Detection

PRB auto-detects protocols using a layered strategy:

1. **Port mapping** -- well-known ports (e.g., 50051 for gRPC) provide a low-confidence initial hint
2. **Magic-byte inspection** -- protocol-specific byte patterns in the first bytes of a stream:
   - HTTP/2: connection preface `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`
   - ZMTP: greeting byte `0xFF` followed by padding and `0x7F`
   - RTPS: magic bytes `RTPS` at offset 0
3. **Heuristic analysis** -- deeper inspection of frame structure, header patterns, and byte distributions

The highest-confidence match wins. Detection can be overridden per-invocation with `--protocol`.

### Adding Protocol Support

Custom protocols can be added via the plugin system. See [Plugin Development](plugin-development.md) for writing detection and decoding plugins.
