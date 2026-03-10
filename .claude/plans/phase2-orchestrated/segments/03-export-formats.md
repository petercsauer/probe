---
segment: 3
title: "Export to Developer Ecosystem Formats (prb-export)"
depends_on: []
risk: 4
complexity: Medium
cycle_budget: 4
status: pending
commit_message: "feat(prb-export): add CSV, HAR, OTLP, Parquet, and HTML export"
---

# Subsection 8: Export to Developer Ecosystem Formats (`prb-export`)

## Purpose

A new `prb-export` crate providing a trait-based export framework that converts
`DebugEvent`s to formats developers already use: CSV, HAR (HTTP Archive), OTLP
JSON (OpenTelemetry), Parquet (columnar analytics), and self-contained HTML
reports. Addresses competitive analysis recommendation #9.

**Why**: Teams use 8+ observability tools (Grafana survey 2025). Probe must
integrate into existing workflows, not create another data silo. Shareable HTML
reports let developers attach decoded protocol views to GitHub issues without
requiring teammates to install Probe.

## State-of-the-Art References

- **Wireshark**: Export to CSV, PSML, PDML, JSON, C arrays — no OTLP or Parquet
- **Termshark**: No export beyond pcap
- **Malcolm**: Exports to Elasticsearch, Kibana dashboards
- **Fluxzy**: HAR export for HTTP traffic
- **CloudShark**: Shareable pcap analysis via HTML — the UX model for our HTML
  report
- **DuckDB + Parquet**: Industry-standard analytics pipeline; data teams query
  Parquet with SQL

## Architecture

```
prb-export
├── lib.rs           # Exporter trait, ExportConfig, format registry
├── error.rs         # ExportError
├── csv_export.rs    # CSV exporter (csv 1.4)
├── har_export.rs    # HAR exporter (har 0.8)
├── otlp_export.rs   # OTLP JSON exporter (manual serde structs)
├── parquet_export.rs # Parquet exporter (arrow + parquet, feature-gated)
└── html_export.rs   # Self-contained HTML report
```

### Exporter Trait

```rust
pub trait Exporter {
    fn format_name(&self) -> &'static str;
    fn file_extension(&self) -> &'static str;
    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError>;
}
```

### CLI Command

```
prb export <input> --format <csv|har|otlp|parquet|html> [-o output] [--where "filter"]
```

Input loading reuses the same NDJSON/MCAP/PCAP detection logic from
`prb inspect`. Output defaults to stdout for text formats (CSV, HAR, OTLP JSON),
requires `-o` for binary formats (Parquet, HTML).

---

## Dependencies

| Crate | Version | Purpose | Feature-gated? |
|-------|---------|---------|----------------|
| `csv` | 1.4 | CSV writing | No |
| `har` | 0.8 | HAR spec types + serde | No |
| `chrono` | 0.4 | ISO 8601 timestamp formatting | No (already in workspace) |
| `serde` + `serde_json` | workspace | JSON serialization | No |
| `arrow` | 54 | Arrow RecordBatch for Parquet | Yes (`parquet` feature) |
| `parquet` | 54 | Parquet file writing | Yes (`parquet` feature) |

---

## Segment S8.1: Export Framework & CLI Integration

**Files**: `crates/prb-export/src/lib.rs`, `crates/prb-export/src/error.rs`,
`crates/prb-cli/src/commands/export.rs`, `crates/prb-cli/src/cli.rs`

### Exporter Trait

```rust
pub trait Exporter {
    fn format_name(&self) -> &'static str;
    fn file_extension(&self) -> &'static str;
    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError>;
}
```

### Format Registry

```rust
pub fn create_exporter(format: &str) -> Result<Box<dyn Exporter>, ExportError> {
    match format {
        "csv" => Ok(Box::new(CsvExporter)),
        "har" => Ok(Box::new(HarExporter)),
        "otlp" => Ok(Box::new(OtlpExporter)),
        "html" => Ok(Box::new(HtmlExporter)),
        #[cfg(feature = "parquet")]
        "parquet" => Ok(Box::new(ParquetExporter)),
        _ => Err(ExportError::UnsupportedFormat(format.to_string())),
    }
}
```

### CLI

- New `Commands::Export(ExportArgs)` variant in `cli.rs`
- `ExportArgs`: `input: Utf8PathBuf`, `--format: ExportFormatArg`, `--output: Option<Utf8PathBuf>`, `--where: Option<String>`
- `export.rs` command handler loads events (reuse inspect logic), creates exporter, writes output

---

## Segment S8.2: CSV Exporter

**File**: `crates/prb-export/src/csv_export.rs`

Flattens `DebugEvent` into a tabular row. Uses the `csv` crate (1.4, 241 MB/sec
raw throughput).

### CSV Schema

| Column | Source |
|--------|--------|
| id | `event.id.as_u64()` |
| timestamp_nanos | `event.timestamp.as_nanos()` |
| timestamp_iso | Formatted via chrono |
| adapter | `event.source.adapter` |
| origin | `event.source.origin` |
| src_addr | `event.source.network.src` or "" |
| dst_addr | `event.source.network.dst` or "" |
| transport | `event.transport.to_string()` |
| direction | `event.direction.to_string()` |
| payload_type | "raw" or "decoded" |
| payload_size | byte length |
| schema_name | from decoded payload or "" |
| decoded_fields | JSON string of decoded fields or "" |
| metadata | JSON object of all metadata KV pairs |
| grpc_method | Well-known metadata shortcut |
| grpc_status | Well-known metadata shortcut |
| zmq_topic | Well-known metadata shortcut |
| dds_topic_name | Well-known metadata shortcut |
| sequence | Optional sequence number |
| warnings | JSON array or "" |

### Implementation

```rust
pub struct CsvExporter;

impl Exporter for CsvExporter {
    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError> {
        let mut wtr = csv::Writer::from_writer(writer);
        for event in events {
            wtr.serialize(EventRow::from(event))?;
        }
        wtr.flush()?;
        Ok(())
    }
}
```

---

## Segment S8.3: HAR Exporter

**File**: `crates/prb-export/src/har_export.rs`

Maps gRPC-over-HTTP/2 events to HAR 1.2 format. Uses the `har` crate (0.8,
936K+ downloads) for spec-compliant types.

### Mapping: DebugEvent → HAR Entry

| HAR Field | Source |
|-----------|--------|
| `log.entries[].startedDateTime` | `event.timestamp` → ISO 8601 |
| `log.entries[].time` | 0 (single event, no round-trip) |
| `log.entries[].request.method` | "POST" (gRPC is always POST) |
| `log.entries[].request.url` | `grpc.method` metadata |
| `log.entries[].request.httpVersion` | "HTTP/2.0" |
| `log.entries[].request.headers` | From event metadata |
| `log.entries[].request.bodySize` | Payload byte count |
| `log.entries[].response.*` | From paired response event if available |

### Strategy

- Filter for `TransportKind::Grpc` events (HAR is HTTP-specific)
- Group by H2 stream ID to pair requests and responses
- Non-gRPC events get a comment noting they were skipped
- Sets `log.creator` to `{ name: "probe", version: env!("CARGO_PKG_VERSION") }`

---

## Segment S8.4: OTLP JSON Exporter

**File**: `crates/prb-export/src/otlp_export.rs`

Maps `DebugEvent`s to OpenTelemetry Trace spans in OTLP JSON format. The output
can be sent directly to any OTLP-compatible backend (Jaeger, Grafana Tempo,
Datadog) via `curl` or imported programmatically.

### OTLP JSON Structure (manual serde structs)

We define the minimal OTLP JSON types ourselves to avoid pulling in the full
`opentelemetry-proto` crate (which depends on `tonic`, `prost`, etc.):

```rust
struct ExportTraceServiceRequest {
    resource_spans: Vec<ResourceSpans>,
}

struct ResourceSpans {
    resource: Resource,
    scope_spans: Vec<ScopeSpans>,
}

struct ScopeSpans {
    scope: InstrumentationScope,
    spans: Vec<Span>,
}

struct Span {
    trace_id: String,      // hex-encoded 16 bytes
    span_id: String,       // hex-encoded 8 bytes
    name: String,          // grpc.method or transport + direction
    kind: i32,             // SPAN_KIND_CLIENT(3) or SPAN_KIND_SERVER(2)
    start_time_unix_nano: String,
    end_time_unix_nano: String,
    attributes: Vec<KeyValue>,
    status: SpanStatus,
}
```

### Mapping: DebugEvent → OTLP Span

| OTLP Field | Source |
|------------|--------|
| `trace_id` | Deterministic from event correlation keys or generated |
| `span_id` | Deterministic from event ID |
| `name` | `grpc.method` metadata, or `"{transport} {direction}"` |
| `kind` | Outbound → CLIENT(3), Inbound → SERVER(2) |
| `start_time_unix_nano` | `event.timestamp.as_nanos()` as string |
| `attributes` | All metadata KV pairs + transport, adapter, origin |
| `status` | From `grpc.status` (0=OK, else ERROR) |

### Usage

```bash
prb export capture.mcap --format otlp -o spans.json
curl -X POST http://localhost:4318/v1/traces \
  -H "Content-Type: application/json" \
  -d @spans.json
```

---

## Segment S8.5: Parquet Exporter (Feature-Gated)

**File**: `crates/prb-export/src/parquet_export.rs`

Writes events as a columnar Parquet file for analytics with DuckDB, Pandas,
Polars, or any Arrow-compatible tool. Feature-gated behind `parquet` to avoid
heavy compile-time cost for users who don't need it.

### Arrow Schema

```
id: UInt64
timestamp_nanos: UInt64
timestamp_iso: Utf8
adapter: Utf8
origin: Utf8
src_addr: Utf8 (nullable)
dst_addr: Utf8 (nullable)
transport: Utf8
direction: Utf8
payload_type: Utf8
payload_size: UInt64
schema_name: Utf8 (nullable)
decoded_fields_json: Utf8 (nullable)
metadata_json: Utf8
grpc_method: Utf8 (nullable)
grpc_status: Utf8 (nullable)
zmq_topic: Utf8 (nullable)
dds_topic_name: Utf8 (nullable)
sequence: UInt64 (nullable)
warnings_json: Utf8 (nullable)
```

### Usage

```bash
prb export capture.mcap --format parquet -o events.parquet
duckdb -c "SELECT transport, count(*) FROM 'events.parquet' GROUP BY transport"
```

---

## Segment S8.6: Self-Contained HTML Report

**File**: `crates/prb-export/src/html_export.rs`

Generates a single-file HTML report with embedded CSS and JavaScript. Opens in
any browser, no server needed. Inspired by CloudShark's shareable capture view.

### Report Layout

```
┌─────────────────────────────────────────────────┐
│  Probe Report — capture.pcap                     │
│  Generated: 2026-03-10 14:32:01                  │
├─────────────────────────────────────────────────┤
│  Summary                                         │
│  Events: 127 │ Time: 14:00–14:05 │ Protocols:   │
│  gRPC: 89  ZMQ: 38  Warnings: 3                 │
├─────────────────────────────────────────────────┤
│  Event Table (sortable, filterable)              │
│  # │ Time │ Src │ Dst │ Proto │ Dir │ Summary    │
│  Click row to expand details ▼                   │
│    ├── Source: pcap / capture.pcap               │
│    ├── Metadata: grpc.method=/api/Users/Get      │
│    ├── Payload: 142 bytes (decoded)              │
│    └── Fields: { "user_id": "abc-123" }          │
├─────────────────────────────────────────────────┤
│  Footer: Generated by Probe v0.1.0              │
└─────────────────────────────────────────────────┘
```

### Implementation Strategy

- Serialize events as a JSON blob inside `<script>` tag
- Client-side JavaScript renders the interactive table
- CSS: minimal, dark-theme, responsive (no framework)
- Features: click-to-expand rows, text search, column sort
- All inline — zero external requests (works offline)
- Total template: ~200 lines of HTML/CSS/JS

---

## Dependency Map

```
prb-export ───── prb-core (DebugEvent types)
  │
  ├── csv 1.4         (CSV writer)
  ├── har 0.8         (HAR types + serde)
  ├── chrono 0.4      (timestamp formatting)
  ├── serde + serde_json (workspace)
  │
  └── [feature: parquet]
      ├── arrow 54     (RecordBatch, Schema)
      └── parquet 54   (ArrowWriter)

prb-cli ──────── prb-export
```

---

## Execution Order

S8.1 (framework + CLI) → S8.2 (CSV) → S8.3 (HAR) → S8.4 (OTLP) → S8.5 (Parquet) → S8.6 (HTML)

S8.2–S8.6 are independent once S8.1 is done. The ordering above follows
increasing complexity.

---

## Tests

| Test | What it verifies |
|------|-----------------|
| `csv_round_trip` | CSV output parses back; all columns present |
| `csv_empty_events` | Empty input produces header-only CSV |
| `har_grpc_events` | gRPC events map to valid HAR entries |
| `har_skips_non_http` | ZMQ/DDS events excluded from HAR |
| `otlp_valid_json` | Output parses as valid OTLP trace JSON |
| `otlp_span_attributes` | Metadata maps to span attributes |
| `parquet_schema` | Parquet file has expected schema |
| `parquet_round_trip` | Written data reads back correctly |
| `html_contains_events` | HTML contains all event data |
| `html_self_contained` | HTML has no external resource references |
| `export_format_registry` | All formats create correct exporters |

---

## Acceptance Criteria

- [ ] `cargo build --workspace` — zero errors, zero warnings
- [ ] `cargo clippy --workspace --all-targets` — zero warnings
- [ ] `cargo test --workspace` — all tests pass
- [ ] `prb export fixtures/grpc_sample.json --format csv` outputs valid CSV
- [ ] `prb export fixtures/grpc_sample.json --format har` outputs valid HAR JSON
- [ ] `prb export fixtures/grpc_sample.json --format otlp` outputs valid OTLP JSON
- [ ] `prb export fixtures/grpc_sample.json --format html -o report.html` produces
      a self-contained HTML file that opens in a browser
- [ ] `prb export fixtures/grpc_sample.json --format parquet -o events.parquet`
      produces a valid Parquet file (with `--features parquet`)
- [ ] `prb export ... --where "transport == \"gRPC\""` applies filter before export
- [ ] All exporters handle empty event lists gracefully
- [ ] HTML report has zero external resource references (fully offline)
