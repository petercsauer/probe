use crate::cli::TuiArgs;
use anyhow::{Context, Result};
use prb_capture::{CaptureConfig, CaptureError, InterfaceEnumerator, LiveCaptureAdapter};
use prb_tui::loader::{LoadEvent, load_events, load_events_streaming, load_schemas};
use prb_tui::{App, EventStore, LiveDataSource, generate_demo_events};
use std::path::PathBuf;
use std::sync::mpsc;

pub fn run_tui(args: TuiArgs) -> Result<()> {
    // Diff mode
    if args.diff {
        return run_tui_diff(args);
    }

    // Live capture mode
    if let Some(ref interface) = args.interface {
        return run_tui_live(interface.clone(), args);
    }

    // File-based or demo mode
    let (mut store, input_file_path, input_file_size, tls_stats) = if args.demo {
        let events = generate_demo_events();
        tracing::info!("Generated {} demo events", events.len());
        (EventStore::new(events), None, 0, None)
    } else {
        let input = args
            .input
            .as_ref()
            .context("Input file required (or use --demo or --interface)")?;
        let path = std::path::PathBuf::from(input.as_str());

        // TLS keylog file (for pcap/pcapng decryption)
        let tls_keylog = args.tls_keylog.as_ref().map(|p| PathBuf::from(p.as_str()));

        // Check file size to decide whether to use streaming load
        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        // Use streaming load for files > 10MB or if they have > 10K estimated events
        let (store, tls_stats) = if file_size > 10 * 1024 * 1024 {
            tracing::info!(
                "Loading large file ({:.2} MB) with streaming...",
                file_size as f64 / (1024.0 * 1024.0)
            );
            load_with_streaming(&path, tls_keylog)?
        } else {
            load_events(&path, tls_keylog).context("Failed to load events")?
        };

        (store, Some(path), file_size, tls_stats)
    };

    tracing::info!("Loaded {} events", store.len());

    // Build index in background for faster filtering (worthwhile for large files)
    if store.len() > 1000 {
        tracing::info!("Building event index for {} events...", store.len());
        store.build_index();
        tracing::info!("Index built successfully");
    }

    // Load schemas if provided (not applicable in demo mode)
    let schema_registry =
        if !args.demo && (!args.proto.is_empty() || !args.descriptor_set.is_empty()) {
            let input = args.input.as_ref().unwrap(); // Safe: we checked demo mode above
            let path = std::path::PathBuf::from(input.as_str());
            let proto_paths: Vec<std::path::PathBuf> = args
                .proto
                .iter()
                .map(|p| p.as_std_path().to_path_buf())
                .collect();
            let desc_paths: Vec<std::path::PathBuf> = args
                .descriptor_set
                .iter()
                .map(|p| p.as_std_path().to_path_buf())
                .collect();

            let mcap_path = if path.extension().and_then(|e| e.to_str()) == Some("mcap") {
                Some(path.as_path())
            } else {
                None
            };

            match load_schemas(&proto_paths, &desc_paths, mcap_path) {
                Ok(registry) => {
                    let msg_count = registry.list_messages().len();
                    tracing::info!("Loaded schema registry with {} message types", msg_count);
                    Some(registry)
                }
                Err(e) => {
                    tracing::warn!("Failed to load schemas: {}", e);
                    None
                }
            }
        } else {
            None
        };

    let mut app = App::new(store, args.where_clause, schema_registry);

    // Set input file info for session display
    if let Some(path) = input_file_path {
        app.set_input_file(path, input_file_size);
    }

    // Set TLS stats if available
    if let Some(stats) = tls_stats {
        app.set_tls_stats(stats);
    }

    // Restore session if provided
    if let Some(ref session_path) = args.session {
        match prb_tui::Session::load(session_path.as_std_path()) {
            Ok(session) => {
                app.restore_session(session);
                tracing::info!("Restored session from {}", session_path);
            }
            Err(e) => {
                tracing::warn!("Failed to load session from {}: {}", session_path, e);
            }
        }
    }

    app.run()
}

/// Run TUI in live capture mode.
fn run_tui_live(interface: String, args: TuiArgs) -> Result<()> {
    // Build capture config
    let config = CaptureConfig {
        interface: interface.clone(),
        bpf_filter: args.bpf_filter.clone(),
        snaplen: 65535,
        promisc: true,
        immediate_mode: true,
        buffer_size: 2 * 1024 * 1024, // 2MB default
        timeout_ms: 1000,
        tls_keylog_path: args.tls_keylog.as_ref().map(|p| PathBuf::from(p.as_str())),
    };

    // Create and start adapter
    let adapter = LiveCaptureAdapter::new(config).map_err(|e| match e {
        CaptureError::InsufficientPrivileges {
            ref message,
            ref remediation,
        } => anyhow::anyhow!("{message}\n\n  {remediation}"),
        CaptureError::InterfaceNotFound(ref dev) => {
            anyhow::anyhow!(
                "Network interface '{}' not found.\n\n  Available interfaces:\n{}",
                dev,
                format_available_interfaces()
            )
        }
        _ => anyhow::anyhow!("{e}"),
    })?;

    // Create live data source (adapter.start() will be called inside)
    let live_source = LiveDataSource::start(adapter, interface.clone())
        .context("failed to start live capture data source")?;

    // Create empty event store for live mode
    let store = EventStore::empty();

    // Ring buffer capacity: 100K events
    const RING_BUFFER_CAPACITY: usize = 100_000;

    // Create and run TUI in live mode
    let mut app = App::new_live(store, live_source, RING_BUFFER_CAPACITY, None);
    app.run_live()
}

/// Load events with streaming and progress display.
fn load_with_streaming(
    path: &std::path::Path,
    tls_keylog: Option<PathBuf>,
) -> Result<(EventStore, Option<prb_tui::loader::TlsStats>)> {
    let (tx, rx) = mpsc::channel();

    // Start streaming load in background thread
    let path_clone = path.to_owned();
    std::thread::spawn(move || {
        if let Err(e) = load_events_streaming(&path_clone, tx.clone(), tls_keylog) {
            let _ = tx.send(LoadEvent::Error(e.to_string()));
        }
    });

    // Receive events and build store incrementally
    let mut store = EventStore::empty();
    let mut last_progress = std::time::Instant::now();
    let mut total_loaded = 0;
    let mut tls_stats = None;

    loop {
        match rx.recv() {
            Ok(LoadEvent::Batch(events)) => {
                total_loaded += events.len();
                store.push_batch(events);

                // Show progress every 100ms
                if last_progress.elapsed() > std::time::Duration::from_millis(100) {
                    eprint!("\rLoading events... {} loaded", total_loaded);
                    last_progress = std::time::Instant::now();
                }
            }
            Ok(LoadEvent::Progress { loaded, total }) => {
                if let Some(t) = total {
                    eprint!(
                        "\rLoading events... {}/{} ({:.1}%)",
                        loaded,
                        t,
                        (loaded as f64 / t as f64) * 100.0
                    );
                } else {
                    eprint!("\rLoading events... {} loaded", loaded);
                }
            }
            Ok(LoadEvent::TlsStats(stats)) => {
                tls_stats = Some(stats);
            }
            Ok(LoadEvent::Done) => {
                eprintln!("\rLoaded {} events successfully", total_loaded);
                break;
            }
            Ok(LoadEvent::Error(e)) => {
                eprintln!("\rError loading events: {}", e);
                return Err(anyhow::anyhow!("Failed to load events: {}", e));
            }
            Err(_) => {
                return Err(anyhow::anyhow!("Channel error during streaming load"));
            }
        }
    }

    Ok((store, tls_stats))
}

/// Run TUI in diff mode comparing two captures.
fn run_tui_diff(args: TuiArgs) -> Result<()> {
    let file1 = args
        .input
        .as_ref()
        .context("First file required for diff mode (use positional argument)")?;
    let file2 = args
        .diff_file
        .as_ref()
        .context("Second file required for diff mode (use --diff-file)")?;

    let path1 = PathBuf::from(file1.as_str());
    let path2 = PathBuf::from(file2.as_str());

    tracing::info!("Loading first file: {:?}", path1);
    let (store1, _) = load_events(&path1, None).context("Failed to load first file")?;

    tracing::info!("Loading second file: {:?}", path2);
    let (store2, _) = load_events(&path2, None).context("Failed to load second file")?;

    tracing::info!(
        "Loaded {} events from file 1, {} from file 2",
        store1.len(),
        store2.len()
    );

    let mut app = App::new_diff(store1, store2, path1, path2);
    app.run()
}

/// Format available network interfaces for error messages.
fn format_available_interfaces() -> String {
    match InterfaceEnumerator::list() {
        Ok(interfaces) => {
            if interfaces.is_empty() {
                "    (no interfaces found)".to_string()
            } else {
                interfaces
                    .iter()
                    .map(|iface| {
                        let status = if iface.is_up { "UP" } else { "DOWN" };
                        format!("    {} [{}]", iface.name, status)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Err(_) => "    (failed to enumerate interfaces)".to_string(),
    }
}
