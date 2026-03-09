//! Inspect command implementation.

use crate::cli::{InspectArgs, OutputFormat};
use crate::output;
use anyhow::{Context, Result};
use prb_core::{DebugEvent, TransportKind};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::str::FromStr;

pub fn run_inspect(args: InspectArgs) -> Result<()> {
    tracing::info!("Inspecting debug events");

    // Choose input source
    let reader: Box<dyn BufRead> = if let Some(input_path) = args.input {
        tracing::info!("Reading from: {}", input_path);
        let file = File::open(&input_path)
            .with_context(|| format!("Failed to open input file {}", input_path))?;
        Box::new(BufReader::new(file))
    } else {
        Box::new(BufReader::new(io::stdin()))
    };

    // Parse filter if provided
    let filter = args
        .filter
        .as_ref()
        .map(|s| {
            TransportKind::from_str(s)
                .map_err(|e| anyhow::anyhow!("Invalid transport filter: {}", e))
        })
        .transpose()?;

    // Read and parse events
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
        if let Some(ref transport) = filter {
            if &event.transport != transport {
                continue;
            }
        }

        events.push(event);
    }

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
