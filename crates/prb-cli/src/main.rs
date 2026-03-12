//! PRB CLI entry point.

#![allow(unexpected_cfgs)]

use anyhow::Result;
use clap::Parser;
use prb_cli::{cli, commands};
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    // Parse CLI arguments first to check if we're in TUI mode
    let cli = cli::Cli::parse();

    // Initialize tracing - redirect to file for TUI mode to avoid garbled output
    let is_tui_mode = matches!(cli.command, cli::Commands::Tui(_))
        || matches!(&cli.command, cli::Commands::Capture(args) if args.tui);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    if is_tui_mode {
        // TUI mode: log to file to avoid interfering with display
        if let Ok(log_file) = std::fs::File::create("/tmp/prb-tui.log") {
            // Allow trace level for debugging layout issues
            let tui_filter = EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("prb_tui::panes::event_list=trace,warn"));
            fmt()
                .with_env_filter(tui_filter)
                .with_target(true)
                .with_ansi(false)
                .compact()
                .with_writer(std::sync::Mutex::new(log_file))
                .init();
        } else {
            // If we can't create log file, just disable logging
            fmt()
                .with_env_filter(EnvFilter::new("off"))
                .with_target(false)
                .compact()
                .with_writer(std::io::sink)
                .init();
        }
    } else {
        // Non-TUI mode: log to stderr as normal
        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .compact()
            .with_writer(std::io::stderr)
            .init();
    }

    // Dispatch to command handlers
    match cli.command {
        cli::Commands::Ingest(args) => commands::run_ingest(args),
        cli::Commands::Inspect(args) => commands::run_inspect(args),
        cli::Commands::Schemas(args) => commands::run_schemas(args),
        cli::Commands::Tui(args) => commands::run_tui(args),
        cli::Commands::Export(args) => commands::run_export(args),
        cli::Commands::Merge(args) => commands::run_merge(args),
        // cli::Commands::Explain(args) => commands::run_explain(args),
        cli::Commands::Capture(args) => commands::run_capture(args),
        cli::Commands::Plugins(args) => commands::run_plugins(args, cli.plugin_dir.as_ref()),
    }
}
