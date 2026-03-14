# Query Language

PRB includes a built-in query language for filtering events. Filters can be used with the `--where` flag on `inspect`, `tui`, and `export` commands, and in the TUI's interactive filter input.

## Syntax Overview

A filter expression is a boolean predicate evaluated against each `DebugEvent`. Events matching the expression are included; non-matching events are excluded.

```
prb inspect events.ndjson --where 'transport == "gRPC"'
prb tui capture.pcap --where 'grpc.method contains "Users"'
prb export capture.pcap --format csv --where 'direction == "inbound" && id > 100'
```

## Field Names

### Built-in Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | number | Event ID |
| `timestamp` | number | Timestamp in nanoseconds since epoch |
| `transport` | string | Transport kind: `gRPC`, `ZMQ`, `DDS-RTPS`, `TCP`, `UDP` |
| `direction` | string | Message direction: `inbound`, `outbound`, `unknown` |
| `source.adapter` / `adapter` | string | Adapter name (e.g., `pcap`, `json-fixture`) |
| `source.origin` / `origin` | string | Origin identifier (e.g., file path) |
| `source.src` / `src` | string | Source IP:port |
| `source.dst` / `dst` | string | Destination IP:port |
| `sequence` | number | Sequence number within a stream |

### Network Protocol Fields

Wireshark-style protocol fields (protocol-validated):

| Field | Type | Description |
|-------|------|-------------|
| `tcp.port` | number | TCP port (bidirectional: matches src OR dst) |
| `tcp.srcport` | number | TCP source port |
| `tcp.dstport` | number | TCP destination port |
| `udp.port` | number | UDP port (bidirectional: matches src OR dst) |
| `udp.srcport` | number | UDP source port |
| `udp.dstport` | number | UDP destination port |
| `ip.addr` | string | IP address (bidirectional: matches src OR dst) |
| `ip.src` | string | IP source address |
| `ip.dst` | string | IP destination address |
| `frame.len` | number | Frame length (packet size) |

**Note:** Protocol fields are validated - `tcp.port` only matches TCP traffic, `udp.port` only matches UDP traffic.

### Metadata Fields

Any metadata key can be used directly as a dotted field name. Common metadata fields:

| Field | Protocol | Description |
|-------|----------|-------------|
| `grpc.method` | gRPC | Full method path (e.g., `/api.v1.Users/Get`) |
| `h2.stream_id` | gRPC | HTTP/2 stream ID |
| `grpc.status` | gRPC | gRPC status code |
| `zmq.topic` | ZMQ | PUB/SUB topic |
| `zmq.socket_type` | ZMQ | Socket type (PUB, SUB, REQ, etc.) |
| `dds.domain_id` | DDS | DDS domain ID |
| `dds.topic_name` | DDS | DDS topic name |
| `otel.trace_id` | Any | OpenTelemetry trace ID |
| `otel.span_id` | Any | OpenTelemetry span ID |

## Operators

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equal | `transport == "gRPC"` |
| `!=` | Not equal | `transport != "ZMQ"` |
| `>` | Greater than | `id > 100` |
| `>=` | Greater or equal | `id >= 50` |
| `<` | Less than | `id < 200` |
| `<=` | Less or equal | `sequence <= 10` |

### String Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `contains` | Case-insensitive substring match | `grpc.method contains "Users"` |
| `matches` | Regular expression match | `tcp.payload matches "^GET"` |

### Set Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `in` | Value is in set | `tcp.port in {80, 443, 8080}` |

The `in` operator supports strings, numbers, and booleans:

```
transport in {"gRPC", "ZMQ"}
grpc.status in {0, 1, 2}
```

### Existence Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `exists` | Field is present and non-empty | `otel.trace_id exists` |

For `warnings`, `exists` checks that the warnings list is non-empty. For `source.network`, it checks that network address info is present.

### Slice Operator

| Operator | Description | Example |
|----------|-------------|---------|
| `[start:end]` | Byte range slice | `tcp.payload[0:4]` |

Extract a byte range from a field value. Useful for inspecting specific payload regions:

```
tcp.payload[0:4] == "POST"
udp.payload[8:12] contains "DNS"
```

## Functions

Functions can be applied to field values for transformation before comparison:

| Function | Description | Example |
|----------|-------------|---------|
| `len(field)` | Field length | `len(payload) > 1000` |
| `lower(field)` | Convert to lowercase | `lower(grpc.method) contains "user"` |
| `upper(field)` | Convert to uppercase | `upper(http.host) == "API.EXAMPLE.COM"` |

Functions can be nested:

```
lower(upper(http.host)) == "api.example.com"
len(tcp.payload[0:100]) > 50
```

## Value Types

| Type | Syntax | Example |
|------|--------|---------|
| String | Double-quoted | `"gRPC"`, `"/api/Users"` |
| Number | Integer or decimal, optionally negative | `42`, `3.14`, `-1` |
| Boolean | `true` or `false` | `true` |

### String Escaping

| Escape | Character |
|--------|-----------|
| `\"` | Double quote |
| `\\` | Backslash |
| `\n` | Newline |
| `\t` | Tab |

## Boolean Logic

### AND

```
transport == "gRPC" && direction == "inbound"
```

### OR

```
transport == "gRPC" || transport == "ZMQ"
```

### NOT

```
!transport == "ZMQ"
```

### Parentheses

```
(transport == "gRPC" || transport == "ZMQ") && direction == "inbound"
```

### Operator Precedence

From highest to lowest:

1. `!` (NOT)
2. `&&` (AND)
3. `||` (OR)

Use parentheses to override precedence:

```
# Without parens: a OR (b AND c)
a == 1 || b == 2 && c == 3

# With parens: (a OR b) AND c
(a == 1 || b == 2) && c == 3
```

## Examples

### Filter by protocol

```bash
prb inspect events.ndjson --where 'transport == "gRPC"'
prb inspect events.ndjson --where 'transport == "DDS-RTPS"'
```

### Filter by gRPC method

```bash
prb inspect events.ndjson --where 'grpc.method == "/api.v1.Users/GetUser"'
prb inspect events.ndjson --where 'grpc.method contains "Users"'
```

### Filter by direction

```bash
prb inspect events.ndjson --where 'direction == "outbound"'
```

### Filter by network address

```bash
prb inspect events.ndjson --where 'src contains ":50051"'
prb inspect events.ndjson --where 'dst == "10.0.0.2:8080"'
```

### Combine filters

```bash
# gRPC requests to a specific service
prb inspect events.ndjson --where 'transport == "gRPC" && grpc.method contains "Users" && direction == "outbound"'

# Events with trace context
prb inspect events.ndjson --where 'otel.trace_id exists'

# Events with warnings
prb inspect events.ndjson --where 'warnings exists'

# Multiple protocols
prb inspect events.ndjson --where 'transport == "gRPC" || transport == "ZMQ"'
```

### Filter by event ID range

```bash
prb inspect events.ndjson --where 'id >= 100 && id <= 200'
```

### Filter in TUI

```bash
prb tui capture.pcap --where 'transport == "gRPC" && grpc.method contains "Health"'
```

In the TUI, press `/` to open the filter input and type an expression interactively.

### Advanced Operators

**Regular expressions:**
```bash
# Match HTTP GET requests
prb inspect events.ndjson --where 'tcp.payload matches "^GET /"'

# Match UUIDs in gRPC methods
prb inspect events.ndjson --where 'grpc.method matches "[0-9a-f]{8}-[0-9a-f]{4}"'
```

**Set membership:**
```bash
# Multiple ports
prb inspect events.ndjson --where 'tcp.port in {80, 443, 8080, 8443}'

# Multiple protocols
prb inspect events.ndjson --where 'transport in {"gRPC", "ZMQ", "DDS-RTPS"}'
```

**Byte slices:**
```bash
# Check magic bytes
prb inspect events.ndjson --where 'tcp.payload[0:4] == "RTPS"'

# Inspect specific offset
prb inspect events.ndjson --where 'udp.payload[8:12] contains "DNS"'
```

**Functions:**
```bash
# Large payloads
prb inspect events.ndjson --where 'len(payload) > 10000'

# Case-insensitive method matching
prb inspect events.ndjson --where 'lower(grpc.method) contains "getuser"'

# Nested functions
prb inspect events.ndjson --where 'len(tcp.payload[0:100]) > 50'
```
