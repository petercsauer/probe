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

    /// Plugin directory (default: ~/.prb/plugins/)
    #[arg(long, global = true)]
    pub plugin_dir: Option<Utf8PathBuf>,

    /// Disable automatic plugin loading
    #[arg(long, global = true)]
    pub no_plugins: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Ingest a JSON fixture file and convert to NDJSON debug events
    Ingest(IngestArgs),
    /// Inspect debug events from NDJSON format
    Inspect(InspectArgs),
    /// Manage protobuf schemas
    Schemas(SchemasArgs),
    /// Open interactive TUI for exploring captured events
    Tui(TuiArgs),
    /// Export events to developer ecosystem formats (CSV, HAR, OTLP, Parquet, HTML)
    Export(ExportArgs),
    /// Merge OTLP traces with captured packet events
    Merge(MergeArgs),
    // /// Explain an event using AI (LLM-powered plain-English explanation)
    // Explain(ExplainArgs),
    /// Capture live network traffic with real-time protocol decoding
    Capture(CaptureArgs),
    /// Manage protocol decoder plugins
    Plugins(PluginsArgs),
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

    /// Force protocol detection to a specific protocol (bypasses auto-detection).
    /// Valid values: grpc, zmtp, rtps
    #[arg(long, value_parser = ["grpc", "zmtp", "rtps"])]
    pub protocol: Option<String>,

    /// Filter events by OpenTelemetry trace ID
    #[arg(long)]
    pub trace_id: Option<String>,

    /// Filter events by OpenTelemetry span ID
    #[arg(long)]
    pub span_id: Option<String>,

    /// Number of parallel workers (0 = auto-detect, 1 = sequential)
    #[arg(short = 'j', long = "jobs", default_value = "0")]
    pub jobs: usize,
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

    /// Filter events with a query expression (e.g. 'transport == "gRPC"')
    #[arg(long, name = "where")]
    pub where_clause: Option<String>,

    /// Filter events by OpenTelemetry trace ID
    #[arg(long)]
    pub trace_id: Option<String>,

    /// Filter events by OpenTelemetry span ID
    #[arg(long)]
    pub span_id: Option<String>,

    /// Group events by trace ID and display as conversation trees
    #[arg(long)]
    pub group_by_trace: bool,

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
pub struct TuiArgs {
    /// Path to input file (JSON, PCAP, pcapng, or MCAP). Optional if --demo or --interface is used.
    pub input: Option<Utf8PathBuf>,

    /// Pre-apply a filter expression on open
    #[arg(long, name = "where")]
    pub where_clause: Option<String>,

    /// Path to .proto files for schema-based decoding
    #[arg(long, value_name = "PATH")]
    pub proto: Vec<Utf8PathBuf>,

    /// Path to .desc descriptor set files
    #[arg(long, value_name = "PATH")]
    pub descriptor_set: Vec<Utf8PathBuf>,

    /// Load demo dataset with synthetic events (no input file required)
    #[arg(long)]
    pub demo: bool,

    /// Launch TUI in live capture mode on specified network interface
    #[arg(short = 'i', long = "interface")]
    pub interface: Option<String>,

    /// BPF filter expression for live capture (requires --interface)
    #[arg(short = 'f', long = "filter")]
    pub bpf_filter: Option<String>,

    /// Path to TLS keylog file for decrypting captured traffic (requires --interface)
    #[arg(long = "tls-keylog")]
    pub tls_keylog: Option<Utf8PathBuf>,

    /// Restore session from file (loads input file, filter, and view state)
    #[arg(long = "session")]
    pub session: Option<Utf8PathBuf>,

    /// Compare two capture files side-by-side (requires exactly 2 file arguments)
    #[arg(long)]
    pub diff: bool,

    /// Second file for diff comparison (used with --diff and first positional input)
    #[arg(long = "diff-file")]
    pub diff_file: Option<Utf8PathBuf>,
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

#[derive(clap::Args, Debug)]
pub struct ExportArgs {
    /// Path to input file (JSON, PCAP, pcapng, or MCAP)
    pub input: Utf8PathBuf,

    /// Export format
    #[arg(short, long)]
    pub format: ExportFormat,

    /// Output file path (defaults to stdout for text formats, required for binary formats)
    #[arg(short, long)]
    pub output: Option<Utf8PathBuf>,

    /// Filter events with a query expression (e.g. 'transport == "gRPC"')
    #[arg(long, name = "where")]
    pub where_clause: Option<String>,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ExportFormat {
    Csv,
    Har,
    Otlp,
    Html,
    #[allow(unexpected_cfgs)]
    #[cfg(feature = "parquet")]
    Parquet,
}

#[derive(clap::Args, Debug)]
pub struct MergeArgs {
    /// Packet events file (NDJSON or MCAP)
    pub packets: Utf8PathBuf,

    /// OTLP JSON trace file
    pub traces: Utf8PathBuf,

    /// Output file (defaults to stdout NDJSON)
    #[arg(short, long)]
    pub output: Option<Utf8PathBuf>,
}

/* Temporarily disabled due to async-openai API changes
#[derive(clap::Args, Debug)]
pub struct ExplainArgs {
    /// Path to input file (JSON, PCAP, pcapng, NDJSON, or MCAP)
    pub input: Utf8PathBuf,

    /// Event ID to explain (default: last event)
    #[arg(long)]
    pub event_id: Option<u64>,

    /// Number of surrounding events for context (default: 5)
    #[arg(long, default_value = "5")]
    pub context: usize,

    /// AI provider: ollama, openai, custom (default: ollama)
    #[arg(long, default_value = "ollama")]
    pub provider: String,

    /// Model name (default: provider-dependent)
    #[arg(long)]
    pub model: Option<String>,

    /// Custom API base URL
    #[arg(long)]
    pub base_url: Option<String>,

    /// API key (or set PRB_AI_API_KEY env var)
    #[arg(long)]
    pub api_key: Option<String>,

    /// Generation temperature 0.0-1.0 (default: 0.3)
    #[arg(long, default_value = "0.3")]
    pub temperature: f32,

    /// Disable streaming output
    #[arg(long)]
    pub no_stream: bool,
}
*/

#[derive(clap::Args, Debug)]
pub struct CaptureArgs {
    /// Network interface to capture on (e.g., eth0, lo, en0)
    #[arg(short = 'i', long = "interface")]
    pub interface: Option<String>,

    /// BPF filter expression (same syntax as tcpdump)
    #[arg(short = 'f', long = "filter")]
    pub bpf_filter: Option<String>,

    /// Write decoded events to file (NDJSON or MCAP based on extension)
    #[arg(short = 'o', long = "output")]
    pub output: Option<Utf8PathBuf>,

    /// Write raw packets to pcap savefile
    #[arg(short = 'w', long = "write")]
    pub write_pcap: Option<Utf8PathBuf>,

    /// Path to TLS keylog file for decrypting captured traffic
    #[arg(long = "tls-keylog")]
    pub tls_keylog: Option<Utf8PathBuf>,

    /// Maximum bytes to capture per packet (default: 65535)
    #[arg(long = "snaplen", default_value = "65535")]
    pub snaplen: u32,

    /// Disable promiscuous mode
    #[arg(long = "no-promisc")]
    pub no_promisc: bool,

    /// Stop after capturing N packets
    #[arg(short = 'c', long = "count")]
    pub count: Option<u64>,

    /// Stop after N seconds
    #[arg(long = "duration")]
    pub duration: Option<u64>,

    /// List available interfaces and exit
    #[arg(long = "list-interfaces", alias = "list-if")]
    pub list_interfaces: bool,

    /// Open TUI for live interactive analysis
    #[arg(long = "tui")]
    pub tui: bool,

    /// Output format for non-TUI mode (default: summary)
    #[arg(long = "format", default_value = "summary")]
    pub format: CaptureOutputFormat,

    /// Quiet mode: suppress per-packet output, only show final stats
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Kernel capture buffer size in bytes (default: 16MB)
    #[arg(long = "buffer-size", default_value = "16777216")]
    pub buffer_size: u32,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum CaptureOutputFormat {
    /// One-line summary per packet (like tcpdump)
    Summary,
    /// Full NDJSON event per packet
    Json,
}

#[derive(clap::Args, Debug)]
pub struct PluginsArgs {
    #[command(subcommand)]
    pub command: PluginsCommand,
}

#[derive(Subcommand, Debug)]
pub enum PluginsCommand {
    /// List all available decoders and plugins
    List,
    /// Show detailed info about a decoder
    Info {
        /// Decoder name or protocol ID
        name: String,
    },
    /// Install a plugin from a file path
    Install {
        /// Path to .so/.dylib/.dll or .wasm file
        path: Utf8PathBuf,
        /// Optional plugin name (defaults to plugin's self-reported name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove an installed plugin
    Remove {
        /// Plugin name to remove
        name: String,
    },
}
