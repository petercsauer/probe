//! Inspect command implementation.

use crate::cli::{InspectArgs, OutputFormat};
use crate::output;
use anyhow::{Context, Result};
use prb_core::{
    DebugEvent, METADATA_KEY_OTEL_SPAN_ID, METADATA_KEY_OTEL_TRACE_ID, Payload, TransportKind,
};
use prb_decode::decode_wire_format;
use prb_query::Filter;
use prb_storage::SessionReader;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::str::FromStr;

pub fn run_inspect(args: InspectArgs) -> Result<()> {
    tracing::info!("Inspecting debug events");

    // Parse transport filter if provided
    let filter = args
        .filter
        .as_ref()
        .map(|s| {
            TransportKind::from_str(s)
                .map_err(|e| anyhow::anyhow!("Invalid transport filter: {}", e))
        })
        .transpose()?;

    // Parse query filter (--where) if provided
    let query_filter = args
        .where_clause
        .as_ref()
        .map(|s| Filter::parse(s).map_err(|e| anyhow::anyhow!("Invalid filter expression: {}", e)))
        .transpose()?;

    // Determine input format and read events
    let mut events = if let Some(ref input_path) = args.input {
        if input_path.extension() == Some("mcap") {
            read_events_from_mcap(input_path.as_ref(), filter)?
        } else {
            read_events_from_ndjson_file(input_path.as_ref(), filter)?
        }
    } else {
        read_events_from_ndjson_stdin(filter)?
    };

    if let Some(ref qf) = query_filter {
        events.retain(|e| qf.matches(e));
    }

    // Apply trace ID filter if provided
    if let Some(ref trace_id) = args.trace_id {
        events.retain(|e| e.metadata.get(METADATA_KEY_OTEL_TRACE_ID) == Some(trace_id));
    }

    // Apply span ID filter if provided
    if let Some(ref span_id) = args.span_id {
        events.retain(|e| e.metadata.get(METADATA_KEY_OTEL_SPAN_ID) == Some(span_id));
    }

    tracing::info!("Loaded {} events", events.len());

    // If wire-format decoding is requested, decode payloads and print
    if args.wire_format {
        for event in &events {
            println!("=== Event {} at {} ===", event.id, event.timestamp);
            println!(
                "Transport: {} | Direction: {}",
                event.transport, event.direction
            );

            if let Payload::Raw { raw } = &event.payload {
                match decode_wire_format(raw) {
                    Ok(msg) => {
                        println!("{}", msg);
                    }
                    Err(e) => {
                        println!("Wire-format decode error: {}", e);
                    }
                }
            } else {
                println!("(Payload already decoded)");
            }
            println!();
        }
    } else if args.group_by_trace {
        // Group events by trace ID and display as conversation trees
        format_grouped_by_trace(&events);
    } else {
        // Format and output normally
        match args.format {
            OutputFormat::Table => {
                output::format_table(&events);
            }
            OutputFormat::Json => {
                output::format_json(&events)?;
            }
        }
    }

    Ok(())
}

/// Format events grouped by trace ID as conversation trees.
fn format_grouped_by_trace(events: &[DebugEvent]) {
    // Group events by trace ID
    let mut traces: BTreeMap<String, Vec<&DebugEvent>> = BTreeMap::new();
    let mut no_trace_events: Vec<&DebugEvent> = Vec::new();

    for event in events {
        if let Some(trace_id) = event.metadata.get(METADATA_KEY_OTEL_TRACE_ID) {
            traces.entry(trace_id.clone()).or_default().push(event);
        } else {
            no_trace_events.push(event);
        }
    }

    // Print traces
    for (trace_id, trace_events) in traces {
        // Calculate trace duration
        let mut min_ts = u64::MAX;
        let mut max_ts = 0u64;
        for event in &trace_events {
            let ts = event.timestamp.as_nanos();
            min_ts = min_ts.min(ts);
            max_ts = max_ts.max(ts);
        }
        let duration_ms = (max_ts - min_ts) / 1_000_000;

        println!(
            "Trace: {} ({} events, {}ms)",
            trace_id,
            trace_events.len(),
            duration_ms
        );

        for (idx, event) in trace_events.iter().enumerate() {
            let span_id = event
                .metadata
                .get(METADATA_KEY_OTEL_SPAN_ID)
                .map(|s| s.as_str())
                .unwrap_or("unknown");

            let direction_symbol = match event.direction.to_string().as_str() {
                "Outbound" => "→",
                "Inbound" => "←",
                _ => "·",
            };

            let method = event
                .metadata
                .get("grpc.method")
                .or_else(|| event.metadata.get("dds.topic_name"))
                .or_else(|| event.metadata.get("zmq.topic"))
                .map(|s| s.as_str())
                .unwrap_or("(no method)");

            let status = event
                .metadata
                .get("grpc.status")
                .map(|s| format!(" status={}", s))
                .unwrap_or_default();

            let payload_size = match &event.payload {
                Payload::Raw { raw } => format!(" ({}B)", raw.len()),
                Payload::Decoded { .. } => " (decoded)".to_string(),
            };

            println!(
                "  [{:2}] {} {} {} span={}{}{}",
                idx + 1,
                event.timestamp,
                direction_symbol,
                method,
                &span_id[..span_id.len().min(16)],
                payload_size,
                status
            );
        }
        println!();
    }

    // Print events without trace context
    if !no_trace_events.is_empty() {
        println!("Events without trace context: {}", no_trace_events.len());
        for event in no_trace_events {
            println!(
                "  {} {} {}",
                event.timestamp, event.transport, event.direction
            );
        }
    }
}

fn read_events_from_mcap(
    path: &std::path::Path,
    filter: Option<TransportKind>,
) -> Result<Vec<DebugEvent>> {
    tracing::info!("Reading MCAP from: {}", path.display());

    let reader = SessionReader::open(path)
        .with_context(|| format!("Failed to open MCAP file {}", path.display()))?;

    let mut events = Vec::new();
    for event_result in reader.events() {
        let event = event_result.context("Failed to read event from MCAP")?;

        // Apply filter if specified
        if let Some(ref transport) = filter
            && &event.transport != transport
        {
            continue;
        }

        events.push(event);
    }

    Ok(events)
}

fn read_events_from_ndjson_file(
    path: &std::path::Path,
    filter: Option<TransportKind>,
) -> Result<Vec<DebugEvent>> {
    tracing::info!("Reading NDJSON from: {}", path.display());

    let file = File::open(path)
        .with_context(|| format!("Failed to open input file {}", path.display()))?;
    let reader = BufReader::new(file);

    read_events_from_ndjson(reader, filter)
}

fn read_events_from_ndjson_stdin(filter: Option<TransportKind>) -> Result<Vec<DebugEvent>> {
    tracing::info!("Reading NDJSON from stdin");
    let reader = BufReader::new(io::stdin());
    read_events_from_ndjson(reader, filter)
}

fn read_events_from_ndjson(
    reader: impl BufRead,
    filter: Option<TransportKind>,
) -> Result<Vec<DebugEvent>> {
    let mut events = Vec::new();
    for (line_num, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("Failed to read line {}", line_num + 1))?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        let event: DebugEvent = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse JSON on line {}", line_num + 1))?;

        // Apply filter if specified
        if let Some(ref transport) = filter
            && &event.transport != transport
        {
            continue;
        }

        events.push(event);
    }

    Ok(events)
}
