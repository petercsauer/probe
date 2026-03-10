---
segment: 14
title: "Capture CLI Integration"
depends_on: [12, 13]
risk: 8
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-cli): add prb capture subcommand with interface selection and BPF filters"
---

# S3: CLI Integration

**Goal**: Add `prb capture` subcommand to the CLI with interface selection, BPF
filters, output mode control, and the ergonomics users expect from tcpdump/tshark.

**UX reference**: `tcpdump -i eth0 -w capture.pcap tcp port 443`
→ `prb capture -i eth0 -w capture.pcap -f "tcp port 443"`

---

## S3.1: `CaptureArgs` Struct + `Commands::Capture` Variant

### CLI Definition

```rust
// crates/prb-cli/src/cli.rs

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Ingest a JSON fixture file and convert to NDJSON debug events
    Ingest(IngestArgs),
    /// Inspect debug events from NDJSON format
    Inspect(InspectArgs),
    /// Manage protobuf schemas
    Schemas(SchemasArgs),
    /// Capture live network traffic with real-time protocol decoding
    Capture(CaptureArgs),
}

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

    /// Output format for non-TUI mode (default: ndjson to stdout)
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
```

### Usage Examples

```
# Basic live capture on loopback
prb capture -i lo

# Capture gRPC traffic with BPF filter
prb capture -i eth0 -f "tcp port 50051"

# Capture and save raw pcap for later analysis
prb capture -i eth0 -w traffic.pcap -f "host 10.0.0.1"

# Capture with decoded output to NDJSON file
prb capture -i eth0 -o events.ndjson -c 1000

# Capture with TUI (interactive live analysis)
prb capture -i eth0 --tui

# List available interfaces
prb capture --list-interfaces

# Capture with TLS decryption
SSLKEYLOGFILE=/tmp/keys.log prb capture -i eth0 -f "tcp port 443" --tls-keylog /tmp/keys.log

# Capture 100 packets and stop
prb capture -i eth0 -c 100

# Capture for 30 seconds
prb capture -i eth0 --duration 30
```

---

## S3.2: `run_capture()` Orchestration

The command handler that wires everything together.

### Flow

```
1. --list-interfaces? → list and exit
2. Resolve interface (--interface or default)
3. Check privileges (CaptureError::InsufficientPrivileges → print fix)
4. Build CaptureConfig from CLI args
5. Create LiveCaptureAdapter
6. Start capture engine
7. Install Ctrl+C handler
8. Select output sink based on args:
   a. --tui → launch TUI (S4)
   b. --write → PcapSaveSink + optional decoded output
   c. --output → file sink
   d. default → stdout sink
9. Event loop: receive events, write to sink(s)
10. On stop signal: print stats summary
```

### Implementation Sketch

```rust
// crates/prb-cli/src/commands/capture.rs

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
    let mut adapter = LiveCaptureAdapter::new(config)
        .map_err(|e| match e {
            CaptureError::InsufficientPrivileges { ref message, ref remediation } => {
                anyhow::anyhow!("{message}\n\n  {remediation}")
            }
            _ => anyhow::anyhow!("{e}"),
        })?;

    adapter.start().context("failed to start capture")?;

    // Install Ctrl+C handler
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    ctrlc::set_handler(move || {
        stop_clone.store(true, Ordering::SeqCst);
    })?;

    // Header
    if !args.quiet {
        eprintln!("Capturing on interface {} ...", interface);
        if let Some(ref filter) = args.bpf_filter {
            eprintln!("BPF filter: {}", filter);
        }
    }

    // Event loop
    let mut count = 0u64;
    let mut writer = build_output_sink(&args)?;
    let start_time = std::time::Instant::now();

    for event_result in adapter.ingest() {
        if stop.load(Ordering::Relaxed) { break; }
        if let Some(max) = args.count {
            if count >= max { break; }
        }
        if let Some(dur) = args.duration {
            if start_time.elapsed().as_secs() >= dur { break; }
        }

        match event_result {
            Ok(event) => {
                writer.write_event(&event)?;
                count += 1;
            }
            Err(e) => {
                tracing::warn!("Event processing error: {}", e);
            }
        }
    }

    // Stop and print summary
    adapter.stop();
    let stats = adapter.stats();

    eprintln!();
    eprintln!("{} packets captured", count);
    eprintln!("{} packets received by filter", stats.packets_received);
    eprintln!("{} packets dropped by kernel", stats.packets_dropped_kernel);
    eprintln!("{} packets dropped by channel", stats.packets_dropped_channel);

    Ok(())
}
```

### Error Handling for Missing Privileges

When `CaptureEngine::start()` returns `CaptureError::InsufficientPrivileges`:

```
Error: Permission denied: cannot open capture device eth0

  Fix with one of:
    sudo setcap cap_net_raw,cap_net_admin=eip /path/to/prb
    sudo prb capture -i eth0
```

This message is generated by the privilege check in S6 and surfaced by the CLI.

---

## S3.3: Interface List Subcommand

```rust
fn list_interfaces() -> Result<()> {
    let interfaces = InterfaceEnumerator::list()
        .context("failed to enumerate network interfaces")?;

    if interfaces.is_empty() {
        eprintln!("No capture interfaces found.");
        eprintln!("Ensure you have appropriate permissions (see: prb capture --help)");
        return Ok(());
    }

    // Header
    println!(
        "{:<16} {:<8} {:<40} {}",
        "Interface", "Status", "Addresses", "Description"
    );
    println!("{}", "─".repeat(90));

    for iface in &interfaces {
        let status = if iface.is_up && iface.is_running {
            "UP"
        } else if iface.is_up {
            "UP/IDLE"
        } else {
            "DOWN"
        };

        let addrs: Vec<String> = iface.addresses.iter().map(|a| a.to_string()).collect();
        let addrs_str = if addrs.is_empty() {
            "(none)".to_string()
        } else {
            addrs.join(", ")
        };

        let desc = iface.description.as_deref().unwrap_or("");
        let suffix = if iface.is_loopback { " [loopback]" } else { "" };

        println!(
            "{:<16} {:<8} {:<40} {}{}",
            iface.name, status, addrs_str, desc, suffix
        );
    }

    Ok(())
}
```

### `prb-cli/Cargo.toml` Changes

```toml
[dependencies]
# ... existing deps ...
prb-capture = { path = "../prb-capture" }
ctrlc = { version = "3", features = ["termination"] }
```

---

## Ctrl+C Handling

The `ctrlc` crate (14M+ downloads) provides cross-platform signal handling.
On Ctrl+C:

1. Set `stop` flag (atomic bool)
2. The `ingest()` iterator checks the flag and returns `None`
3. `adapter.stop()` signals the capture thread to exit
4. The capture thread completes its current `next_packet()` call and exits
5. Statistics are printed

For TUI mode, the TUI event loop handles Ctrl+C through crossterm's event system
and calls `adapter.stop()` directly.

---

## Implementation Checklist

- [ ] Add `Commands::Capture(CaptureArgs)` to CLI enum
- [ ] Define `CaptureArgs` with all flags
- [ ] Define `CaptureOutputFormat` enum
- [ ] Create `crates/prb-cli/src/commands/capture.rs`
- [ ] Implement `run_capture()` orchestration function
- [ ] Implement `list_interfaces()` display function
- [ ] Add `ctrlc` dependency to `prb-cli`
- [ ] Add `prb-capture` dependency to `prb-cli`
- [ ] Wire `Commands::Capture` in `main.rs` dispatch
- [ ] Implement `build_output_sink()` to select NDJSON/pcap/MCAP/TUI output
- [ ] Integration test: `prb capture --list-interfaces` succeeds
- [ ] Integration test: `prb capture -i lo -c 0` exits immediately with stats
- [ ] Test: missing interface produces clear error
- [ ] Test: invalid BPF filter produces clear error with the expression
