---
segment: 23
title: "CLI Integration + Adaptive Parallelism"
depends_on: [21]
risk: 9
complexity: Low
cycle_budget: 2
status: pending
commit_message: "feat(prb-cli): add --jobs flag with adaptive parallelism and progress reporting"
---

# Subsection 8: CLI Integration + Adaptive Parallelism

## Purpose

Wire the parallel pipeline into the CLI so users can control parallelism via
`--jobs` flag, and the pipeline automatically selects the optimal strategy
based on input size and available cores.

---

## Segment S8.1: CLI `--jobs` Flag

Add a `--jobs` / `-j` flag to the `ingest` command:

```rust
// crates/prb-cli/src/commands/ingest.rs

#[derive(Parser)]
pub struct IngestArgs {
    /// Input file (PCAP, pcapng, or JSON)
    pub input: PathBuf,

    /// Output file (NDJSON or MCAP)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// TLS keylog file for decryption
    #[arg(long)]
    pub tls_keylog: Option<PathBuf>,

    /// Number of parallel workers (0 = auto-detect, 1 = sequential)
    #[arg(short = 'j', long = "jobs", default_value = "0")]
    pub jobs: usize,
}
```

### Behavior

| `--jobs` value | Behavior |
|---------------|----------|
| `0` (default) | Auto-detect: `std::thread::available_parallelism()` |
| `1` | Sequential pipeline (existing `PcapCaptureAdapter`) |
| `N > 1` | Parallel pipeline with N shards |

### Implementation

```rust
pub fn run_ingest(args: IngestArgs) -> Result<()> {
    let format = detect_format(&args.input)?;

    match format {
        InputFormat::Json => {
            // JSON fixtures don't benefit from parallel pipeline
            run_json_ingest(args)
        }
        InputFormat::Pcap | InputFormat::Pcapng => {
            if args.jobs == 1 {
                run_sequential_pcap_ingest(args)
            } else {
                run_parallel_pcap_ingest(args)
            }
        }
    }
}

fn run_parallel_pcap_ingest(args: IngestArgs) -> Result<()> {
    let tls_keylog = load_tls_keylog(&args)?;

    let config = PipelineConfig {
        jobs: args.jobs,
        ..Default::default()
    };

    let source = PacketSource::open(&args.input)?;

    let pipeline = ParallelPipeline::new(
        config,
        Arc::new(tls_keylog),
        args.input.clone(),
    );

    let start = std::time::Instant::now();

    let events = match source {
        PacketSource::Mmap(reader) => {
            let packets = reader.to_pcap_packets();
            pipeline.run(packets)?
        }
        PacketSource::Stream(mut reader) => {
            let packets = reader.read_all_packets()
                .map_err(|e| anyhow::anyhow!("read error: {}", e))?;
            pipeline.run(packets)?
        }
    };

    let elapsed = start.elapsed();

    tracing::info!(
        "Parallel pipeline: {} events in {:.2}s ({:.0} events/s, {} workers)",
        events.len(),
        elapsed.as_secs_f64(),
        events.len() as f64 / elapsed.as_secs_f64(),
        config.effective_jobs(),
    );

    write_events(events, &args)?;

    Ok(())
}
```

---

## Segment S8.2: Adaptive Parallelism

### Auto-detection logic

```rust
impl PipelineConfig {
    pub fn effective_jobs(&self) -> usize {
        if self.jobs == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        } else {
            self.jobs
        }
    }

    pub fn effective_shard_count(&self) -> usize {
        if self.shard_count == 0 {
            self.effective_jobs() * 2
        } else {
            self.shard_count
        }
    }
}
```

### Small-file optimization

For captures under 10,000 packets, parallel overhead exceeds benefit. The
pipeline detects this and falls back to sequential:

```rust
impl ParallelPipeline {
    const PARALLEL_THRESHOLD: usize = 10_000;

    pub fn run(&self, packets: Vec<PcapPacket>) -> Result<Vec<DebugEvent>, CoreError> {
        if packets.len() < Self::PARALLEL_THRESHOLD {
            tracing::debug!(
                "Small capture ({} packets < {} threshold), using sequential path",
                packets.len(),
                Self::PARALLEL_THRESHOLD,
            );
            return self.run_sequential(packets);
        }

        self.run_parallel(packets)
    }
}
```

### Progress reporting

For large captures, emit progress via tracing:

```rust
fn run_parallel(&self, packets: Vec<PcapPacket>) -> Result<Vec<DebugEvent>, CoreError> {
    let total = packets.len();
    tracing::info!("Processing {} packets with {} shards", total, self.num_shards);

    let (normalized, fragments) = self.parallel_normalize(&packets);
    tracing::info!("Normalized: {} packets, {} fragments", normalized.len(), fragments.len());

    let defragged = self.process_fragments(&packets, &fragments);
    let all_normalized: Vec<_> = [normalized, defragged].concat();

    let shards = FlowPartitioner::new(self.num_shards).partition(all_normalized);
    let shard_sizes: Vec<_> = shards.iter().map(|s| s.len()).collect();
    tracing::info!("Partitioned into {} shards: {:?}", shards.len(), shard_sizes);

    let shard_events: Vec<Vec<DebugEvent>> = shards
        .into_par_iter()
        .map(|shard| self.process_shard(shard))
        .collect();

    let mut events: Vec<DebugEvent> = shard_events.into_iter().flatten().collect();
    events.sort_by_key(|e| e.timestamp);

    tracing::info!("Pipeline complete: {} events", events.len());
    Ok(events)
}
```

### Output determinism

The parallel pipeline must produce identical output regardless of job count.
This is guaranteed by:
1. Per-shard processing is deterministic (same packets → same events)
2. Final sort by timestamp produces consistent ordering
3. Ties in timestamp are broken by (src_ip, src_port, dst_ip, dst_port) to
   ensure stable ordering

```rust
events.sort_by(|a, b| {
    a.timestamp.cmp(&b.timestamp)
        .then_with(|| a.source.network.cmp(&b.source.network))
});
```

---

## Backward Compatibility

The parallel pipeline is entirely additive:
- `--jobs 1` uses the existing `PcapCaptureAdapter` unchanged
- `--jobs 0` (default) uses the new parallel pipeline but produces identical output
- JSON fixtures always use the existing `JsonFixtureAdapter` (no parallelism needed)
- All existing tests pass without modification

### Migration path

1. Initially, `--jobs 0` defaults to `1` (sequential) for safety
2. After benchmarks confirm correctness on CI, change default to auto-detect
3. Eventually, remove sequential path and always use parallel (with shards=1
   for single-core)

---

## Environment Variables

For power users and CI:

| Variable | Effect | Example |
|----------|--------|---------|
| `PRB_JOBS` | Override `--jobs` default | `PRB_JOBS=4 prb ingest ...` |
| `PRB_PARALLEL_THRESHOLD` | Override 10k threshold | `PRB_PARALLEL_THRESHOLD=1000` |
| `RAYON_NUM_THREADS` | Limit rayon thread pool | `RAYON_NUM_THREADS=2` |

```rust
fn effective_jobs_with_env(cli_jobs: usize) -> usize {
    if cli_jobs != 0 {
        return cli_jobs;
    }

    if let Ok(env_jobs) = std::env::var("PRB_JOBS") {
        if let Ok(n) = env_jobs.parse::<usize>() {
            return n;
        }
    }

    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
```

---

## Files Changed

| File | Change |
|------|--------|
| `crates/prb-cli/src/commands/ingest.rs` | Add `--jobs` flag, `run_parallel_pcap_ingest` |
| `crates/prb-cli/Cargo.toml` | Add dependency on `prb-pcap/parallel` feature |
| `crates/prb-pcap/Cargo.toml` | Add `parallel` feature flag |

---

## Tests

- `test_cli_jobs_flag_parsing` — `--jobs 4` sets config correctly
- `test_cli_jobs_default_zero` — Default is 0 (auto-detect)
- `test_cli_parallel_ingest_basic` — `prb ingest --jobs 4 fixture.pcap` produces
  correct NDJSON output
- `test_cli_parallel_matches_sequential` — `--jobs 1` and `--jobs 4` produce
  byte-identical NDJSON output for same input
- `test_cli_env_prb_jobs` — `PRB_JOBS=2` overrides default
- `test_cli_small_file_sequential_fallback` — File with 100 packets uses
  sequential path even with `--jobs 0`
