//! Live packet capture command implementation.

use crate::cli::{CaptureArgs, CaptureOutputFormat};
use anyhow::{Context, Result};
use prb_capture::{CaptureConfig, CaptureError, InterfaceEnumerator, LiveCaptureAdapter};
use prb_core::CaptureAdapter;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Run the capture command.
pub fn run_capture(args: CaptureArgs) -> Result<()> {
    // List interfaces and exit
    if args.list_interfaces {
        return list_interfaces();
    }

    // Resolve interface
    let interface = match &args.interface {
        Some(name) => name.clone(),
        None => {
            let default = InterfaceEnumerator::default_device()
                .context("failed to find default capture device")?;
            tracing::info!("Using default interface: {}", default.name);
            default.name
        }
    };

    // Build capture config
    let config = CaptureConfig {
        interface: interface.clone(),
        bpf_filter: args.bpf_filter.clone(),
        snaplen: args.snaplen,
        promisc: !args.no_promisc,
        immediate_mode: true,
        buffer_size: args.buffer_size,
        timeout_ms: 1000,
        tls_keylog_path: args.tls_keylog.as_ref().map(|p| PathBuf::from(p.as_str())),
    };

    // Create and start adapter
    let mut adapter = LiveCaptureAdapter::new(config).map_err(|e| match e {
        CaptureError::InsufficientPrivileges {
            ref message,
            ref remediation,
        } => anyhow::anyhow!("{message}\n\n  {remediation}"),
        _ => anyhow::anyhow!("{e}"),
    })?;

    adapter
        .start()
        .map_err(|e| match e {
            CaptureError::InsufficientPrivileges {
                ref message,
                ref remediation,
            } => anyhow::anyhow!("{message}\n\n  {remediation}"),
            CaptureError::FilterCompilationFailed(ref msg) => {
                anyhow::anyhow!("BPF filter compilation failed: {}", msg)
            }
            _ => anyhow::anyhow!("{e}"),
        })
        .context("failed to start capture")?;

    // Launch TUI in live mode if requested
    if args.tui {
        use prb_tui::{App, EventStore, LiveDataSource};

        // Create live data source (adapter already started)
        let live_source = LiveDataSource::start(adapter, interface.clone())
            .context("failed to start live capture data source")?;

        // Create empty event store for live mode
        let store = EventStore::empty();

        // Ring buffer capacity: 100K events (configurable)
        const RING_BUFFER_CAPACITY: usize = 100_000;

        // Create and run TUI in live mode
        let mut app = App::new_live(store, live_source, RING_BUFFER_CAPACITY, None);
        return app.run_live();
    }

    // Install Ctrl+C handler
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    ctrlc::set_handler(move || {
        stop_clone.store(true, Ordering::SeqCst);
    })
    .context("failed to set Ctrl+C handler")?;

    // Header
    if !args.quiet {
        eprintln!("Capturing on interface {} ...", interface);
        if let Some(ref filter) = args.bpf_filter {
            eprintln!("BPF filter: {}", filter);
        }
    }

    // Event loop
    let mut count = 0u64;
    let start_time = Instant::now();
    let mut stdout = std::io::stdout();

    for event_result in adapter.ingest() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Some(max) = args.count
            && count >= max
        {
            break;
        }
        if let Some(dur) = args.duration
            && start_time.elapsed().as_secs() >= dur
        {
            break;
        }

        match event_result {
            Ok(event) => {
                if !args.quiet {
                    match args.format {
                        CaptureOutputFormat::Summary => {
                            // One-line summary
                            let (src, dst) = if let Some(ref network) = event.source.network {
                                (network.src.as_str(), network.dst.as_str())
                            } else {
                                ("unknown", "unknown")
                            };
                            writeln!(
                                stdout,
                                "{:.6} {} {} -> {}",
                                event.timestamp.as_nanos() as f64 / 1_000_000_000.0,
                                event.transport,
                                src,
                                dst
                            )?;
                        }
                        CaptureOutputFormat::Json => {
                            // Full NDJSON event
                            serde_json::to_writer(&mut stdout, &event)?;
                            writeln!(stdout)?;
                        }
                    }
                    stdout.flush()?;
                }
                count += 1;
            }
            Err(e) => {
                tracing::warn!("Event processing error: {}", e);
            }
        }
    }

    // Stop and get final statistics
    let stats = adapter.stop().unwrap_or_else(|e| {
        tracing::warn!("Failed to stop capture cleanly: {}", e);
        // Return a default stats object
        prb_capture::CaptureStats {
            packets_received: count,
            packets_dropped_kernel: 0,
            packets_dropped_channel: 0,
            bytes_received: 0,
            capture_duration: start_time.elapsed(),
            packets_per_second: 0.0,
            bytes_per_second: 0.0,
        }
    });

    eprintln!();
    eprintln!("{} packets captured", count);
    eprintln!("{} packets received by filter", stats.packets_received);
    eprintln!("{} packets dropped by kernel", stats.packets_dropped_kernel);
    eprintln!(
        "{} packets dropped by channel",
        stats.packets_dropped_channel
    );

    Ok(())
}

/// List available network interfaces.
fn list_interfaces() -> Result<()> {
    let interfaces =
        InterfaceEnumerator::list().context("failed to enumerate network interfaces")?;

    if interfaces.is_empty() {
        eprintln!("No capture interfaces found.");
        eprintln!("Ensure you have appropriate permissions (see: prb capture --help)");
        return Ok(());
    }

    // Header
    println!(
        "{:<16} {:<8} {:<40} Description",
        "Interface", "Status", "Addresses"
    );
    println!("{}", "─".repeat(90));

    for iface in &interfaces {
        let status = iface.status();
        let addrs_str = iface.addresses_display();
        let desc = iface.description.as_deref().unwrap_or("");
        let suffix = if iface.is_loopback {
            " [loopback]"
        } else {
            ""
        };

        println!(
            "{:<16} {:<8} {:<40} {}{}",
            iface.name, status, addrs_str, desc, suffix
        );
    }

    Ok(())
}
