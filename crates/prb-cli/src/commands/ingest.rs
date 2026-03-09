//! Ingest command implementation.

use crate::cli::IngestArgs;
use anyhow::{Context, Result};
use prb_core::CaptureAdapter;
use prb_fixture::JsonFixtureAdapter;
use std::fs::File;
use std::io::{self, BufWriter, Write};

pub fn run_ingest(args: IngestArgs) -> Result<()> {
    tracing::info!("Ingesting fixture file: {}", args.input);

    // Create adapter
    let mut adapter = JsonFixtureAdapter::new(args.input.clone());

    // Choose output destination
    let mut writer: Box<dyn Write> = if let Some(output_path) = args.output {
        tracing::info!("Writing output to: {}", output_path);
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
