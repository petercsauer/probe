//! Merge command - combine OTLP traces with packet-level events.

use crate::cli::MergeArgs;
use anyhow::{Context, Result};
use prb_core::DebugEvent;
use prb_export::{merge_traces_with_packets, otlp_to_events, parse_otlp_json};
use prb_storage::SessionReader;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

pub fn run_merge(args: MergeArgs) -> Result<()> {
    tracing::info!("Merging OTLP traces with packet events");

    // Load packet events
    let packet_events = load_packet_events(&args.packets)?;
    tracing::info!("Loaded {} packet events", packet_events.len());

    // Load OTLP trace events
    let trace_events = load_otlp_trace_events(&args.traces)?;
    tracing::info!("Loaded {} trace events", trace_events.len());

    // Merge
    let merged = merge_traces_with_packets(&packet_events, &trace_events);
    tracing::info!("Merged into {} timeline events", merged.len());

    // Write output
    let mut writer: Box<dyn Write> = if let Some(ref output_path) = args.output {
        tracing::info!("Writing merged output to: {}", output_path);
        let file = File::create(output_path)
            .with_context(|| format!("Failed to create output file {}", output_path))?;
        Box::new(file)
    } else {
        Box::new(io::stdout())
    };

    // Output as NDJSON
    for merged_event in merged {
        // For now, just output the event as-is. In a richer implementation,
        // we could include the span summary in the JSON output.
        serde_json::to_writer(&mut writer, &merged_event.event)?;
        writeln!(&mut writer)?;
    }

    Ok(())
}

fn load_packet_events(path: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    let ext = path.extension().unwrap_or("");

    match ext {
        "mcap" => load_packet_events_from_mcap(path),
        "ndjson" | "json" => load_packet_events_from_ndjson(path),
        _ => {
            // Try NDJSON first, then MCAP
            load_packet_events_from_ndjson(path).or_else(|_| load_packet_events_from_mcap(path))
        }
    }
}

fn load_packet_events_from_mcap(path: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    let reader = SessionReader::open(path.as_std_path())?;
    let mut events = Vec::new();

    for event_result in reader.events() {
        let event = event_result?;
        events.push(event);
    }

    Ok(events)
}

fn load_packet_events_from_ndjson(path: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: DebugEvent = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse JSON on line {}", line_num + 1))?;
        events.push(event);
    }

    Ok(events)
}

fn load_otlp_trace_events(path: &camino::Utf8Path) -> Result<Vec<DebugEvent>> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read OTLP trace file: {}", path))?;

    let request = parse_otlp_json(&data)
        .with_context(|| format!("Failed to parse OTLP JSON from {}", path))?;

    let events = otlp_to_events(&request);
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_ndjson() {
        let event = DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"test"),
            })
            .build();

        let line1 = serde_json::to_string(&event).unwrap();
        let line2 = serde_json::to_string(&event).unwrap();
        let ndjson = format!("{}\n{}\n", line1, line2);

        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), ndjson).unwrap();

        let path = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let events = load_packet_events_from_ndjson(&path).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_load_otlp_json() {
        let json = r#"{
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "test-service"}}
                    ]
                },
                "scopeSpans": [{
                    "scope": {"name": "test", "version": "1.0"},
                    "spans": [{
                        "traceId": "4bf92f3577b34da6a3ce929d0e0e4736",
                        "spanId": "00f067aa0ba902b7",
                        "name": "/api.v1.Users/Get",
                        "kind": 3,
                        "startTimeUnixNano": "1710000000000000000",
                        "endTimeUnixNano": "1710000000100000000",
                        "attributes": []
                    }]
                }]
            }]
        }"#;

        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), json).unwrap();

        let path = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let events = load_otlp_trace_events(&path).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source.adapter, "otlp-import");
    }
}
