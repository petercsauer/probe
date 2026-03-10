//! Explain command implementation.

use crate::cli::ExplainArgs;
use anyhow::{Context, Result};
use prb_ai::{explain_event, explain_event_stream, AiConfig, AiProvider};
use prb_core::{DebugEvent, TransportKind};
use prb_storage::SessionReader;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::str::FromStr;

pub fn run_explain(args: ExplainArgs) -> Result<()> {
    // Parse provider
    let provider = AiProvider::from_str(&args.provider)
        .map_err(|e| anyhow::anyhow!("Invalid provider: {}", e))?;

    // Build config
    let mut config = AiConfig::for_provider(provider).with_context_window(args.context);

    if let Some(model) = args.model {
        config = config.with_model(model);
    }
    if let Some(base_url) = args.base_url {
        config = config.with_base_url(base_url);
    }
    if let Some(api_key) = args.api_key {
        config = config.with_api_key(api_key);
    }
    config = config.with_temperature(args.temperature);
    config = config.with_stream(!args.no_stream);

    // Read events
    let events = if args.input.extension() == Some("mcap") {
        read_events_from_mcap(args.input.as_ref())?
    } else if args.input.extension() == Some("pcap") || args.input.extension() == Some("pcapng") {
        anyhow::bail!(
            "PCAP files must be ingested first: prb ingest {} | prb explain -",
            args.input
        );
    } else {
        read_events_from_ndjson(args.input.as_ref())?
    };

    if events.is_empty() {
        anyhow::bail!("No events found in {}", args.input);
    }

    // Determine target event index
    let target_idx = if let Some(event_id) = args.event_id {
        events
            .iter()
            .position(|e| e.id.as_u64() == event_id)
            .with_context(|| format!("Event ID {} not found", event_id))?
    } else {
        events.len() - 1 // Last event by default
    };

    tracing::info!(
        "Explaining event {} (index {}/{}) using provider {}",
        events[target_idx].id,
        target_idx + 1,
        events.len(),
        config.provider
    );

    // Print event summary
    print_event_summary(&events[target_idx]);

    // Create async runtime
    let rt = tokio::runtime::Runtime::new().context("Failed to create Tokio runtime")?;

    // Call AI engine
    let result = if config.stream {
        rt.block_on(explain_with_stream(&events, target_idx, &config))
    } else {
        rt.block_on(explain_without_stream(&events, target_idx, &config))
    };

    match result {
        Ok(_) => {
            println!();
            Ok(())
        }
        Err(e) => {
            if e.to_string().contains("connect")
                || e.to_string().contains("unreachable")
                || e.to_string().contains("Connection refused")
            {
                anyhow::bail!(
                    "Could not connect to {} at {}. Is the service running?\n\
                     For Ollama: install from https://ollama.ai and run 'ollama serve'\n\
                     Error: {}",
                    config.provider,
                    config.base_url,
                    e
                );
            }
            Err(e.into())
        }
    }
}

async fn explain_with_stream(
    events: &[DebugEvent],
    target_idx: usize,
    config: &AiConfig,
) -> Result<String, prb_ai::AiError> {
    println!("\n=== AI Explanation (streaming) ===\n");
    let result = explain_event_stream(events, target_idx, config, |chunk| {
        print!("{}", chunk);
        let _ = io::stdout().flush();
    })
    .await?;
    Ok(result)
}

async fn explain_without_stream(
    events: &[DebugEvent],
    target_idx: usize,
    config: &AiConfig,
) -> Result<String, prb_ai::AiError> {
    println!("\n=== AI Explanation ===\n");
    let explanation = explain_event(events, target_idx, config).await?;
    println!("{}", explanation);
    Ok(explanation)
}

fn print_event_summary(event: &DebugEvent) {
    println!("\n=== Target Event ===");
    println!("ID: {}", event.id);
    println!("Timestamp: {}", event.timestamp);
    println!("Transport: {}", event.transport);
    println!("Direction: {}", event.direction);

    if let Some(ref network) = event.source.network {
        println!("From: {} → To: {}", network.src, network.dst);
    }

    // Print transport-specific metadata
    match event.transport {
        TransportKind::Grpc => {
            if let Some(method) = event.metadata.get("grpc.method") {
                println!("gRPC method: {}", method);
            }
            if let Some(status) = event.metadata.get("grpc.status") {
                println!("gRPC status: {}", status);
            }
        }
        TransportKind::Zmq => {
            if let Some(topic) = event.metadata.get("zmq.topic") {
                println!("ZMQ topic: {}", topic);
            }
        }
        TransportKind::DdsRtps => {
            if let Some(topic) = event.metadata.get("dds.topic_name") {
                println!("DDS topic: {}", topic);
            }
        }
        _ => {}
    }

    if !event.warnings.is_empty() {
        println!("⚠ Warnings: {}", event.warnings.join("; "));
    }
}

fn read_events_from_mcap(path: &std::path::Path) -> Result<Vec<DebugEvent>> {
    tracing::info!("Reading MCAP from: {}", path.display());

    let reader = SessionReader::open(path)
        .with_context(|| format!("Failed to open MCAP file {}", path.display()))?;

    let mut events = Vec::new();
    for event_result in reader.events() {
        let event = event_result.context("Failed to read event from MCAP")?;
        events.push(event);
    }

    Ok(events)
}

fn read_events_from_ndjson(path: &std::path::Path) -> Result<Vec<DebugEvent>> {
    tracing::info!("Reading NDJSON from: {}", path.display());

    let file = File::open(path)
        .with_context(|| format!("Failed to open input file {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut events = Vec::new();
    for (line_num, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("Failed to read line {}", line_num + 1))?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        let event: DebugEvent = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse JSON on line {}", line_num + 1))?;

        events.push(event);
    }

    Ok(events)
}
