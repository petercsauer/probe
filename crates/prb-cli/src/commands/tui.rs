use crate::cli::TuiArgs;
use anyhow::{Context, Result};
use prb_capture::{CaptureConfig, CaptureError, InterfaceEnumerator, LiveCaptureAdapter};
use prb_tui::loader::{load_events, load_schemas};
use prb_tui::{generate_demo_events, App, EventStore, LiveDataSource};
use std::path::PathBuf;

pub fn run_tui(args: TuiArgs) -> Result<()> {
    // Live capture mode
    if let Some(ref interface) = args.interface {
        return run_tui_live(interface.clone(), args);
    }

    // File-based or demo mode
    let mut store = if args.demo {
        let events = generate_demo_events();
        tracing::info!("Generated {} demo events", events.len());
        EventStore::new(events)
    } else {
        let input = args.input.as_ref().context("Input file required (or use --demo or --interface)")?;
        let path = std::path::PathBuf::from(input.as_str());
        load_events(&path).context("Failed to load events")?
    };

    tracing::info!("Loaded {} events", store.len());

    // Build index in background for faster filtering (worthwhile for large files)
    if store.len() > 1000 {
        tracing::info!("Building event index for {} events...", store.len());
        store.build_index();
        tracing::info!("Index built successfully");
    }

    // Load schemas if provided (not applicable in demo mode)
    let schema_registry = if !args.demo && (!args.proto.is_empty() || !args.descriptor_set.is_empty()) {
        let input = args.input.as_ref().unwrap(); // Safe: we checked demo mode above
        let path = std::path::PathBuf::from(input.as_str());
        let proto_paths: Vec<std::path::PathBuf> = args.proto.iter().map(|p| p.as_std_path().to_path_buf()).collect();
        let desc_paths: Vec<std::path::PathBuf> = args.descriptor_set.iter().map(|p| p.as_std_path().to_path_buf()).collect();

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
            anyhow::anyhow!("Network interface '{}' not found.\n\n  Available interfaces:\n{}",
                dev, format_available_interfaces())
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
