//! Ingest command implementation.

use crate::cli::IngestArgs;
use anyhow::{Context, Result, bail};
use prb_core::{CaptureAdapter, DebugEvent, METADATA_KEY_OTEL_SPAN_ID, METADATA_KEY_OTEL_TRACE_ID};
use prb_fixture::JsonFixtureAdapter;
use prb_pcap::PcapCaptureAdapter;
use prb_pcap::parallel::{ParallelPipeline, PipelineConfig};
use prb_pcap::reader::PcapFileReader;
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
    let mut file = File::open(path)
        .with_context(|| format!("Failed to open input file {}", path.display()))?;
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

    match format {
        InputFormat::Json => {
            // JSON fixtures don't benefit from parallel pipeline
            run_json_ingest(args)
        }
        InputFormat::Pcap | InputFormat::Pcapng => {
            if args.jobs == 1 {
                run_sequential_pcap_ingest(args)
            } else {
                run_parallel_pcap_ingest(args)
            }
        }
    }
}

/// JSON ingest path (always sequential).
fn run_json_ingest(args: IngestArgs) -> Result<()> {
    tracing::info!("Detected JSON fixture format");
    let mut adapter: Box<dyn CaptureAdapter> =
        Box::new(JsonFixtureAdapter::new(args.input.clone()));

    // Determine output format based on file extension
    if let Some(ref output_path) = args.output
        && output_path.extension() == Some("mcap")
    {
        // Write to MCAP format
        return run_ingest_mcap(args, adapter);
    }

    write_events_ndjson(&args, &mut *adapter)
}

/// Sequential PCAP ingest using the existing PcapCaptureAdapter.
fn run_sequential_pcap_ingest(args: IngestArgs) -> Result<()> {
    tracing::info!("Detected PCAP capture format (sequential mode)");
    let capture_path = PathBuf::from(args.input.as_str());
    let tls_keylog_path = args.tls_keylog.as_ref().map(|p| PathBuf::from(p.as_str()));
    let mut adapter = PcapCaptureAdapter::new(capture_path, tls_keylog_path);

    // Apply protocol override if specified
    if let Some(ref protocol) = args.protocol {
        tracing::info!("Applying protocol override: {}", protocol);
        adapter.set_protocol_override(protocol);
    }

    // Determine output format based on file extension
    if let Some(ref output_path) = args.output
        && output_path.extension() == Some("mcap")
    {
        let boxed_adapter: Box<dyn CaptureAdapter> = Box::new(adapter);
        return run_ingest_mcap(args, boxed_adapter);
    }

    write_events_ndjson(&args, &mut adapter)
}

/// Parallel PCAP ingest using the parallel pipeline.
fn run_parallel_pcap_ingest(args: IngestArgs) -> Result<()> {
    tracing::info!("Detected PCAP capture format (parallel mode)");

    let effective_jobs = effective_jobs_with_env(args.jobs);
    tracing::info!("Using {} parallel workers", effective_jobs);

    let config = PipelineConfig {
        jobs: effective_jobs,
        ..Default::default()
    };

    let capture_path = PathBuf::from(args.input.as_str());

    // Load TLS keylog if provided
    let tls_keylog = if let Some(ref keylog_path) = args.tls_keylog {
        use prb_pcap::tls::TlsKeyLog;
        use std::sync::Arc;
        let keylog = TlsKeyLog::from_file(PathBuf::from(keylog_path.as_str()))
            .context("Failed to load TLS keylog file")?;
        Arc::new(keylog)
    } else {
        use prb_pcap::tls::TlsKeyLog;
        use std::sync::Arc;
        Arc::new(TlsKeyLog::new())
    };

    let pipeline = ParallelPipeline::new(config, capture_path.clone(), tls_keylog);

    // Read all packets
    let mut reader = PcapFileReader::open(&capture_path).context("Failed to open PCAP file")?;
    let packets = reader
        .read_all_packets()
        .context("Failed to read packets from PCAP file")?;

    tracing::info!("Read {} packets from capture", packets.len());

    // Convert PcapPacket to OwnedNormalizedPacket
    // For now, skip fragments since we need stateful defrag
    let normalized_packets: Vec<_> = packets
        .into_iter()
        .filter_map(|pkt| {
            use prb_pcap::{NormalizeResult, normalize_stateless};
            match normalize_stateless(pkt.linktype, pkt.timestamp_us, &pkt.data) {
                Ok(NormalizeResult::Packet(normalized)) => Some(normalized),
                Ok(NormalizeResult::Fragment { .. }) => None, // Skip fragments for now
                Err(_) => None,
            }
        })
        .collect();

    tracing::info!("Normalized {} packets", normalized_packets.len());

    let start = std::time::Instant::now();
    let events = pipeline
        .run(normalized_packets)
        .context("Parallel pipeline failed")?;
    let elapsed = start.elapsed();

    tracing::info!(
        "Parallel pipeline: {} events in {:.2}s ({:.0} events/s, {} workers)",
        events.len(),
        elapsed.as_secs_f64(),
        events.len() as f64 / elapsed.as_secs_f64(),
        effective_jobs,
    );

    // Write events
    write_events_from_vec(&args, events)
}

/// Writes events from an adapter to NDJSON output.
fn write_events_ndjson(args: &IngestArgs, adapter: &mut dyn CaptureAdapter) -> Result<()> {
    let mut writer: Box<dyn Write> = if let Some(output_path) = &args.output {
        tracing::info!("Writing NDJSON output to: {}", output_path);
        let file = File::create(output_path)
            .with_context(|| format!("Failed to create output file {}", output_path))?;
        Box::new(BufWriter::new(file))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };

    let mut count = 0;
    for event_result in adapter.ingest() {
        let event = event_result.context("Failed to read event from adapter")?;

        // Apply trace ID filter if provided
        if let Some(ref trace_id) = args.trace_id
            && event.metadata.get(METADATA_KEY_OTEL_TRACE_ID) != Some(trace_id)
        {
            continue;
        }

        // Apply span ID filter if provided
        if let Some(ref span_id) = args.span_id
            && event.metadata.get(METADATA_KEY_OTEL_SPAN_ID) != Some(span_id)
        {
            continue;
        }

        serde_json::to_writer(&mut writer, &event).context("Failed to serialize event to JSON")?;
        writeln!(&mut writer)?;

        count += 1;
    }

    writer.flush()?;
    tracing::info!("Ingested {} events", count);

    Ok(())
}

/// Writes events from a Vec to NDJSON output.
fn write_events_from_vec(args: &IngestArgs, events: Vec<DebugEvent>) -> Result<()> {
    let mut writer: Box<dyn Write> = if let Some(output_path) = &args.output {
        tracing::info!("Writing NDJSON output to: {}", output_path);
        let file = File::create(output_path)
            .with_context(|| format!("Failed to create output file {}", output_path))?;
        Box::new(BufWriter::new(file))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };

    let mut count = 0;
    for event in events {
        // Apply trace ID filter if provided
        if let Some(ref trace_id) = args.trace_id
            && event.metadata.get(METADATA_KEY_OTEL_TRACE_ID) != Some(trace_id)
        {
            continue;
        }

        // Apply span ID filter if provided
        if let Some(ref span_id) = args.span_id
            && event.metadata.get(METADATA_KEY_OTEL_SPAN_ID) != Some(span_id)
        {
            continue;
        }

        serde_json::to_writer(&mut writer, &event).context("Failed to serialize event to JSON")?;
        writeln!(&mut writer)?;

        count += 1;
    }

    writer.flush()?;
    tracing::info!("Ingested {} events", count);

    Ok(())
}

/// Determines the effective number of jobs, considering environment variables.
fn effective_jobs_with_env(cli_jobs: usize) -> usize {
    if cli_jobs != 0 {
        return cli_jobs;
    }

    if let Ok(env_jobs) = std::env::var("PRB_JOBS")
        && let Ok(n) = env_jobs.parse::<usize>()
    {
        return n;
    }

    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
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

    let mut writer =
        SessionWriter::new(file, metadata).context("Failed to create MCAP session writer")?;

    // Process and write events
    let mut count = 0;
    for event_result in adapter.ingest() {
        let event = event_result.context("Failed to read event from adapter")?;

        // Apply trace ID filter if provided
        if let Some(ref trace_id) = args.trace_id
            && event.metadata.get(METADATA_KEY_OTEL_TRACE_ID) != Some(trace_id)
        {
            continue;
        }

        // Apply span ID filter if provided
        if let Some(ref span_id) = args.span_id
            && event.metadata.get(METADATA_KEY_OTEL_SPAN_ID) != Some(span_id)
        {
            continue;
        }

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
