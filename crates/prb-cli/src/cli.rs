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
    /// Manage protobuf schemas
    Schemas(SchemasArgs),
}

#[derive(clap::Args, Debug)]
pub struct IngestArgs {
    /// Path to the input file (JSON fixture, PCAP, or pcapng)
    pub input: Utf8PathBuf,

    /// Output file path (defaults to stdout for NDJSON, required for MCAP)
    #[arg(short, long)]
    pub output: Option<Utf8PathBuf>,

    /// Path to TLS keylog file (SSLKEYLOGFILE format) for decrypting PCAP captures
    #[arg(long)]
    pub tls_keylog: Option<Utf8PathBuf>,
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

    /// Decode protobuf payloads as wire-format (best-effort, no schema)
    #[arg(long)]
    pub wire_format: bool,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum OutputFormat {
    Table,
    Json,
}

#[derive(clap::Args, Debug)]
pub struct SchemasArgs {
    #[command(subcommand)]
    pub command: SchemasCommand,
}

#[derive(Subcommand, Debug)]
pub enum SchemasCommand {
    /// Load schema from a .proto or .desc file
    Load(SchemaLoadArgs),
    /// List message types in a session's embedded schemas
    List(SchemaListArgs),
    /// Export schemas from a session to a .desc file
    Export(SchemaExportArgs),
}

#[derive(clap::Args, Debug)]
pub struct SchemaLoadArgs {
    /// Path to .proto or .desc file
    pub path: Utf8PathBuf,

    /// Include paths for resolving imports (only for .proto files)
    #[arg(short = 'I', long = "include-path")]
    pub include_paths: Vec<Utf8PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct SchemaListArgs {
    /// Path to MCAP session file
    pub session: Utf8PathBuf,
}

#[derive(clap::Args, Debug)]
pub struct SchemaExportArgs {
    /// Path to MCAP session file
    pub session: Utf8PathBuf,

    /// Output path for exported .desc file
    #[arg(short, long)]
    pub output: Utf8PathBuf,
}
