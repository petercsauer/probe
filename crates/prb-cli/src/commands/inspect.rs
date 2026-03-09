//! Inspect command implementation.

use crate::cli::{InspectArgs, OutputFormat};
use crate::output;
use anyhow::{Context, Result};
use prb_core::{DebugEvent, TransportKind};
use prb_storage::SessionReader;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::str::FromStr;

pub fn run_inspect(args: InspectArgs) -> Result<()> {
    tracing::info!("Inspecting debug events");

    // Parse filter if provided
    let filter = args
        .filter
        .as_ref()
        .map(|s| {
            TransportKind::from_str(s)
                .map_err(|e| anyhow::anyhow!("Invalid transport filter: {}", e))
        })
        .transpose()?;

    // Determine input format and read events
    let events = if let Some(ref input_path) = args.input {
        if input_path.extension() == Some("mcap") {
            // Read from MCAP format
            read_events_from_mcap(input_path.as_ref(), filter)?
        } else {
            // Read from NDJSON format
            read_events_from_ndjson_file(input_path.as_ref(), filter)?
        }
    } else {
        // Read from stdin (NDJSON)
        read_events_from_ndjson_stdin(filter)?
    };

    tracing::info!("Loaded {} events", events.len());

    // Format and output
    match args.format {
        OutputFormat::Table => {
            output::format_table(&events);
        }
        OutputFormat::Json => {
            output::format_json(&events)?;
        }
    }

    Ok(())
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
            && &event.transport != transport {
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
            && &event.transport != transport {
                continue;
            }

        events.push(event);
    }

    Ok(events)
}
