//! Ingest command implementation.

use crate::cli::IngestArgs;
use anyhow::{Context, Result};
use prb_core::CaptureAdapter;
use prb_fixture::JsonFixtureAdapter;
use prb_storage::{SessionMetadata, SessionWriter};
use std::fs::File;
use std::io::{self, BufWriter, Write};

pub fn run_ingest(args: IngestArgs) -> Result<()> {
    tracing::info!("Ingesting fixture file: {}", args.input);

    // Create adapter
    let mut adapter = JsonFixtureAdapter::new(args.input.clone());

    // Determine output format based on file extension
    if let Some(ref output_path) = args.output
        && output_path.extension() == Some("mcap") {
            // Write to MCAP format
            return run_ingest_mcap(args, adapter);
        }

    // Write to NDJSON format (default)
    let mut writer: Box<dyn Write> = if let Some(output_path) = args.output {
        tracing::info!("Writing NDJSON output to: {}", output_path);
        let file = File::create(&output_path)
            .with_context(|| format!("Failed to create output file {}", output_path))?;
        Box::new(BufWriter::new(file))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };

    // Process events
    let mut count = 0;
    for event_result in adapter.ingest() {
        let event = event_result.context("Failed to read event from adapter")?;

        // Serialize as NDJSON (one JSON object per line)
        serde_json::to_writer(&mut writer, &event)
            .context("Failed to serialize event to JSON")?;
        writeln!(&mut writer)?;

        count += 1;
    }

    writer.flush()?;
    tracing::info!("Ingested {} events", count);

    Ok(())
}

fn run_ingest_mcap(args: IngestArgs, mut adapter: JsonFixtureAdapter) -> Result<()> {
    let output_path = args.output.as_ref().expect("output path must be set");
    tracing::info!("Writing MCAP output to: {}", output_path);

    // Create MCAP writer with metadata
    let metadata = SessionMetadata::new()
        .with_source_file(args.input.to_string())
        .with_capture_tool("json-fixture");

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create output file {}", output_path))?;

    let mut writer = SessionWriter::new(file, metadata)
        .context("Failed to create MCAP session writer")?;

    // Process and write events
    let mut count = 0;
    for event_result in adapter.ingest() {
        let event = event_result.context("Failed to read event from adapter")?;
        writer
            .write_event(&event)
            .context("Failed to write event to MCAP")?;
        count += 1;
    }

    // Finalize the session
    writer.finish().context("Failed to finalize MCAP file")?;

    tracing::info!("Ingested {} events to MCAP", count);

    Ok(())
}
