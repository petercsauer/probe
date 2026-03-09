//! PRB CLI entry point.

mod cli;
mod commands;
mod output;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

fn main() -> Result<()> {
    // Initialize tracing
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .with_writer(std::io::stderr)
        .init();

    // Parse CLI arguments
    let cli = cli::Cli::parse();

    // Dispatch to command handlers
    match cli.command {
        cli::Commands::Ingest(args) => commands::run_ingest(args),
        cli::Commands::Inspect(args) => commands::run_inspect(args),
    }
}
