# Export Formats

PRB exports decoded events to several standard formats for integration with existing tools and workflows.

## CSV

Tabular format for spreadsheets, data analysis, and database import.

```bash
prb export capture.pcap --format csv --output events.csv
prb export capture.pcap --format csv  # stdout
```

### Columns

| Column | Description |
|--------|-------------|
| `id` | Event ID |
| `timestamp` | ISO 8601 timestamp |
| `transport` | Protocol (gRPC, ZMQ, DDS-RTPS, etc.) |
| `direction` | inbound / outbound / unknown |
| `src` | Source address (IP:port) |
| `dst` | Destination address (IP:port) |
| `metadata` | JSON-encoded metadata map |
| `payload_size` | Payload size in bytes |

### Use cases

- Import into pandas, R, or Excel for analysis
- Feed into SQL databases
- Generate charts and statistics

## HAR (HTTP Archive)

Standard HTTP Archive format, compatible with browser developer tools, Charles Proxy, and Fiddler.

```bash
prb export capture.pcap --format har --output events.har
```

HAR export maps gRPC calls to HTTP/2 entries. Each gRPC request/response pair becomes a HAR entry with method, URL, status, headers, and timing.

### Use cases

- Open in Chrome DevTools (Network tab > Import HAR)
- Analyze with HAR viewers (e.g., [toolbox.googleapps.com/apps/har_analyzer](https://toolbox.googleapps.com/apps/har_analyzer))
- Compare request/response patterns across captures

## OTLP JSON

OpenTelemetry Protocol format for trace backends (Jaeger, Grafana Tempo, Zipkin, Honeycomb).

```bash
prb export capture.pcap --format otlp --output traces.json
```

Events with OpenTelemetry trace context (`otel.trace_id`, `otel.span_id`) are exported as OTLP trace spans. Events without trace context are exported as standalone spans with generated trace IDs.

### Use cases

- Import into Jaeger or Tempo for trace visualization
- Merge with existing application traces via `prb merge`
- Correlate network-level events with application-level spans

### Merging with application traces

```bash
# 1. Export packet events as OTLP
prb export capture.pcap --format otlp --output packet-traces.json

# 2. Merge with application traces
prb merge packet-events.ndjson app-traces.json --output merged.ndjson
```

## HTML

Self-contained HTML report for sharing and archival.

```bash
prb export capture.pcap --format html --output report.html
```

The HTML report includes:

- Event summary table with filtering
- Protocol breakdown statistics
- Conversation grouping
- Inline CSS (no external dependencies)

### Use cases

- Share debugging results with teammates via email or Slack
- Archive investigation results
- Generate documentation for incident reports

## Parquet

Columnar binary format for large-scale data analysis.

```bash
prb export capture.pcap --format parquet --output events.parquet
```

> Note: Parquet export requires the `parquet` feature flag: `cargo build --features parquet`

### Schema

Events are stored as a flat Parquet schema with columns for all `DebugEvent` fields, metadata serialized as JSON strings.

### Use cases

- Query with DuckDB, Apache Spark, or Polars
- Store in data lakes (S3, GCS, HDFS)
- Perform large-scale analytics across many captures

**Example with DuckDB:**

```sql
SELECT transport, count(*) as event_count
FROM read_parquet('events.parquet')
GROUP BY transport
ORDER BY event_count DESC;
```

## Filtering Before Export

All export formats support pre-filtering with `--where`:

```bash
prb export capture.pcap --format csv --where 'transport == "gRPC"' --output grpc-only.csv
prb export capture.pcap --format otlp --where 'otel.trace_id exists' --output traced.json
```

## Format Comparison

| Format | Binary | Streaming | Schema | Best for |
|--------|--------|-----------|--------|----------|
| CSV | No | Yes | Flat | Spreadsheets, SQL import |
| HAR | No | No | HTTP Archive | Browser tools, HTTP analysis |
| OTLP | No | No | OpenTelemetry | Trace backends, correlation |
| HTML | No | No | Self-contained | Sharing, archival |
| Parquet | Yes | No | Columnar | Large-scale analytics |
