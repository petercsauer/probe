//! Ingest command implementation.

use crate::cli::IngestArgs;
use anyhow::{bail, Context, Result};
use prb_core::CaptureAdapter;
use prb_fixture::JsonFixtureAdapter;
use prb_pcap::PcapCaptureAdapter;
use prb_storage::{SessionMetadata, SessionWriter};
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
enum InputFormat {
    Json,
    Pcap,
    Pcapng,
}

fn detect_format(path: &Path) -> Result<InputFormat> {
    let mut file = File::open(path).with_context(|| format!("Failed to open input file {}", path.display()))?;
    let mut magic = [0u8; 4];

    // Try to read magic bytes
    if file.read(&mut magic).is_err() {
        // File too short, fall back to extension
        return match path.extension().and_then(|e| e.to_str()) {
            Some("json") => Ok(InputFormat::Json),
            Some("pcap") => Ok(InputFormat::Pcap),
            Some("pcapng") => Ok(InputFormat::Pcapng),
            _ => bail!("Unsupported input format for {}", path.display()),
        };
    }

    match &magic {
        [0x0a, 0x0d, 0x0d, 0x0a] => Ok(InputFormat::Pcapng),
        [0xa1, 0xb2, 0xc3, 0xd4] | [0xd4, 0xc3, 0xb2, 0xa1] => Ok(InputFormat::Pcap),
        [b'{', ..] | [b'[', ..] => Ok(InputFormat::Json),
        _ => {
            // Fall back to extension
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => Ok(InputFormat::Json),
                Some("pcap") => Ok(InputFormat::Pcap),
                Some("pcapng") => Ok(InputFormat::Pcapng),
                _ => bail!("Unsupported input format for {}", path.display()),
            }
        }
    }
}

pub fn run_ingest(args: IngestArgs) -> Result<()> {
    tracing::info!("Ingesting input file: {}", args.input);

    // Detect input format based on magic bytes
    let format = detect_format(args.input.as_std_path())?;
    let mut adapter: Box<dyn CaptureAdapter> = match format {
        InputFormat::Json => {
            // JSON fixture format
            tracing::info!("Detected JSON fixture format");
            Box::new(JsonFixtureAdapter::new(args.input.clone()))
        }
        InputFormat::Pcap | InputFormat::Pcapng => {
            // PCAP/pcapng capture format
            tracing::info!("Detected PCAP capture format");
            let capture_path = PathBuf::from(args.input.as_str());
            let tls_keylog_path = args.tls_keylog.as_ref().map(|p| PathBuf::from(p.as_str()));
            Box::new(PcapCaptureAdapter::new(capture_path, tls_keylog_path))
        }
    };

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

fn run_ingest_mcap(args: IngestArgs, mut adapter: Box<dyn CaptureAdapter>) -> Result<()> {
    let output_path = args.output.as_ref().expect("output path must be set");
    tracing::info!("Writing MCAP output to: {}", output_path);

    // Create MCAP writer with metadata
    let metadata = SessionMetadata::new()
        .with_source_file(args.input.to_string())
        .with_capture_tool(adapter.name());

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
