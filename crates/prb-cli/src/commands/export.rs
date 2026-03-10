//! Export command - convert DebugEvents to developer ecosystem formats.

use crate::cli::{ExportArgs, ExportFormat};
use anyhow::{Context, Result};
use prb_core::DebugEvent;
use prb_export::create_exporter;
use std::fs::File;
use std::io::{self, BufWriter, Write};

pub fn run(args: ExportArgs) -> Result<()> {
    // Load events from input file
    let events = load_events(&args.input)?;

    // Apply filter if provided
    let events = if let Some(ref filter) = args.where_clause {
        apply_filter(events, filter)?
    } else {
        events
    };

    // Create exporter
    let format_str = match args.format {
        ExportFormat::Csv => "csv",
        ExportFormat::Har => "har",
        ExportFormat::Otlp => "otlp",
        ExportFormat::Html => "html",
        #[cfg(feature = "parquet")]
        ExportFormat::Parquet => "parquet",
    };

    let exporter = create_exporter(format_str)
        .with_context(|| format!("Failed to create {} exporter", format_str))?;

    // Determine output destination
    let output: Box<dyn Write> = match args.output {
        Some(path) => {
            let file = File::create(&path)
                .with_context(|| format!("Failed to create output file: {}", path))?;
            Box::new(BufWriter::new(file))
        }
        None => {
            // Binary formats require explicit output file
            match args.format {
                ExportFormat::Html => {
                    anyhow::bail!("HTML export requires --output <file.html>")
                }
                #[cfg(feature = "parquet")]
                ExportFormat::Parquet => {
                    anyhow::bail!("Parquet export requires --output <file.parquet>")
                }
                _ => Box::new(io::stdout()),
            }
        }
    };

    // Export
    exporter
        .export(&events, &mut Box::new(output))
        .context("Export failed")?;

    Ok(())
}

fn load_events(input: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    // Try to detect format and load accordingly
    let ext = input.extension().unwrap_or("");

    match ext {
        "json" => {
            // Try JSON array/object first, then fall back to NDJSON
            load_json_events(input).or_else(|_| load_ndjson_events(input))
        }
        "ndjson" => load_ndjson_events(input),
        "mcap" => load_mcap_events(input),
        "pcap" | "pcapng" => {
            anyhow::bail!(
                "PCAP files must be ingested first: prb ingest {} --output events.mcap",
                input
            )
        }
        _ => {
            // Try all formats
            load_ndjson_events(input)
                .or_else(|_| load_json_events(input))
                .or_else(|_| load_mcap_events(input))
                .with_context(|| format!("Failed to load events from {}", input))
        }
    }
}

fn load_json_events(path: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    let content = std::fs::read_to_string(path)?;

    // Try as array first
    if let Ok(events) = serde_json::from_str::<Vec<DebugEvent>>(&content) {
        return Ok(events);
    }

    // Try as single event
    let event = serde_json::from_str::<DebugEvent>(&content)?;
    Ok(vec![event])
}

fn load_ndjson_events(path: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    let content = std::fs::read_to_string(path)?;
    let mut events = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event: DebugEvent = serde_json::from_str(line)?;
        events.push(event);
    }

    Ok(events)
}

fn load_mcap_events(path: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    use prb_storage::SessionReader;

    let reader = SessionReader::open(path.as_std_path())?;
    let mut events = Vec::new();

    for event_result in reader.events() {
        let event = event_result?;
        events.push(event);
    }

    Ok(events)
}

fn apply_filter(events: Vec<DebugEvent>, filter: &str) -> Result<Vec<DebugEvent>> {
    // Simple filter implementation supporting basic equality checks
    // Format: 'field == "value"' or 'field != "value"'

    let parts: Vec<&str> = filter.split_whitespace().collect();
    if parts.len() != 3 {
        anyhow::bail!("Filter must be in format: field == \"value\" or field != \"value\"");
    }

    let field = parts[0];
    let op = parts[1];
    let value = parts[2].trim_matches('"');

    if op != "==" && op != "!=" {
        anyhow::bail!("Only == and != operators are supported");
    }

    let filtered: Vec<DebugEvent> = events
        .into_iter()
        .filter(|event| {
            let field_value = match field {
                "transport" => Some(event.transport.to_string()),
                "direction" => Some(event.direction.to_string()),
                "adapter" => Some(event.source.adapter.clone()),
                "origin" => Some(event.source.origin.clone()),
                _ => event.metadata.get(field).cloned(),
            };

            match (field_value, op) {
                (Some(fv), "==") => fv == value,
                (Some(fv), "!=") => fv != value,
                (None, "!=") => true,
                _ => false,
            }
        })
        .collect();

    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::*;
    use tempfile::NamedTempFile;

    fn sample_event() -> DebugEvent {
        DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:50051".into(),
                    dst: "10.0.0.2:8080".into(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"hello"),
            })
            .metadata("grpc.method", "/api.v1.Users/Get")
            .build()
    }

    #[test]
    fn filter_by_transport() {
        let grpc_event = sample_event();
        let mut zmq_event = sample_event();
        zmq_event.transport = TransportKind::Zmq;

        let events = vec![grpc_event.clone(), zmq_event];
        let filtered = apply_filter(events, "transport == \"gRPC\"").unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].transport, TransportKind::Grpc);
    }

    #[test]
    fn load_json_array() {
        let event = sample_event();
        let json = serde_json::to_string(&vec![event]).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), json).unwrap();

        let path = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let events = load_json_events(&path).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn load_ndjson() {
        let event = sample_event();
        let line1 = serde_json::to_string(&event).unwrap();
        let line2 = serde_json::to_string(&event).unwrap();
        let ndjson = format!("{}\n{}\n", line1, line2);

        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), ndjson).unwrap();

        let path = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let events = load_ndjson_events(&path).unwrap();
        assert_eq!(events.len(), 2);
    }
}
