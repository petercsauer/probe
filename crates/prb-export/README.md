# prb-export

Export and import subsystem for PRB. Converts `DebugEvent` collections into CSV, HAR, OTLP JSON, HTML, and (feature-gated) Apache Parquet formats. Also provides OTLP trace import and a trace-packet merging pipeline that correlates OpenTelemetry spans with captured network events.

## Key types and traits

| Type / Trait | Description |
|------|-------------|
| `Exporter` | Trait — `format_name()`, `file_extension()`, `export(&[DebugEvent], &mut dyn Write)` |
| `CsvExporter` | Exports events as a flat CSV table |
| `HarExporter` | Exports HTTP-shaped events as HAR 1.2 archives |
| `OtlpExporter` | Exports events as OTLP JSON (`ExportTraceServiceRequest`) |
| `HtmlExporter` | Generates a self-contained HTML report |
| `ParquetExporter` | Columnar Parquet output (requires `parquet` feature) |
| `ExportTraceServiceRequest` | Deserialized OTLP trace import structure |
| `MergedEvent` | Combined span + packet view from `merge_traces_with_packets` |
| `SpanSummary` | Lightweight span summary used during trace-packet merging |
| `ExportError` | Error type for export/import operations |

### Helper functions

| Function | Description |
|----------|-------------|
| `create_exporter(format)` | Factory — returns a boxed `Exporter` by format name |
| `supported_formats()` | Lists all available format names |
| `parse_otlp_json(json)` | Parses OTLP JSON into `ExportTraceServiceRequest` |
| `otlp_to_events(request)` | Converts OTLP spans to `DebugEvent`s |
| `merge_traces_with_packets(spans, packets)` | Correlates trace spans with network captures |

## Usage

```rust
use prb_export::{create_exporter, Exporter};
use prb_core::DebugEvent;

let events: Vec<DebugEvent> = load_events();
let exporter = create_exporter("csv")?;

let mut output = Vec::new();
exporter.export(&events, &mut output)?;

println!("Exported {} bytes of {}", output.len(), exporter.format_name());
```

## Relationship to other crates

- **prb-core** — provides `DebugEvent`, the input to all exporters
- **prb-tui** — offers export actions from the UI
- CLI binary uses `create_exporter` for `prb export` subcommand

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

Export `DebugEvents` to various formats (CSV, HAR, OTLP, HTML, Parquet).

This crate provides exporters for converting `DebugEvents` into various
industry-standard and analysis-friendly formats.

<!-- cargo-rdme end -->
