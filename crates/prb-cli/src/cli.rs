//! CLI argument definitions using clap.

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "prb")]
#[command(about = "Universal message debugger for gRPC, ZMTP, and DDS-RTPS")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Ingest a JSON fixture file and convert to NDJSON debug events
    Ingest(IngestArgs),
    /// Inspect debug events from NDJSON format
    Inspect(InspectArgs),
}

#[derive(clap::Args, Debug)]
pub struct IngestArgs {
    /// Path to the JSON fixture file
    pub input: Utf8PathBuf,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<Utf8PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct InspectArgs {
    /// Path to NDJSON file (reads from stdin if not provided)
    pub input: Option<Utf8PathBuf>,

    /// Output format
    #[arg(short, long, default_value = "table")]
    pub format: OutputFormat,

    /// Filter by transport kind
    #[arg(long)]
    pub filter: Option<String>,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum OutputFormat {
    Table,
    Json,
}
