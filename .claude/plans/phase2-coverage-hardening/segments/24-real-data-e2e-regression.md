---
segment: 24
title: "Real-Data Tests: End-to-End Regression Suite and Fixture Registry"
depends_on: [13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]
risk: 3
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(workspace): add real-data e2e regression suite, fixture registry, and CI integration"
---

# Segment 24: Real-Data Tests — End-to-End Regression Suite and Fixture Registry

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Create a unified regression test suite that runs all captures through the full pipeline, a fixture registry documenting all test data, and CI integration to run real-data tests on every PR.

**Depends on:** All real-data segments (13-23)

## Data Sources

No new downloads — this segment aggregates all fixtures from segments 13-23.

## Scope

- `tests/fixtures/README.md` — Fixture registry with source URLs, licenses, file descriptions
- `tests/fixtures/captures/manifest.json` — Machine-readable fixture metadata
- `crates/prb-pcap/tests/regression_suite.rs` — Unified regression test
- `crates/prb-cli/tests/real_data_cli_tests.rs` — CLI real-data smoke tests
- `.github/workflows/` or CI config — Real-data test integration (if CI exists)

## Implementation Approach

### Fixture registry (README.md)
Create `tests/fixtures/README.md` documenting:
```markdown
# Test Fixtures

## Capture Files

| File | Protocol | Source | License | Size | Notes |
|------|----------|--------|---------|------|-------|
| captures/grpc/grpc_person_search*.pcapng | gRPC/H2 | Wireshark Wiki | GPL-2.0 | ~150KB | Person search service |
| captures/tls/tls13_sample.pcapng | TLS 1.3 | tex2e/openssl-playground | MIT | ~50KB | With keylog |
| ... | ... | ... | ... | ... | ... |
```

### Machine-readable manifest
Create `tests/fixtures/captures/manifest.json`:
```json
{
  "fixtures": [
    {
      "path": "grpc/grpc_person_search_protobuf_with_image.pcapng",
      "protocols": ["grpc", "http2", "tcp", "ip"],
      "source_url": "https://wiki.wireshark.org/...",
      "license": "GPL-2.0",
      "size_bytes": 153600,
      "expected_events_min": 10,
      "expected_protocols": ["grpc"],
      "has_keylog": false
    }
  ]
}
```

### Unified regression test
```rust
use std::fs;

#[derive(Deserialize)]
struct FixtureManifest { fixtures: Vec<Fixture> }

#[derive(Deserialize)]
struct Fixture {
    path: String,
    expected_events_min: usize,
    expected_protocols: Vec<String>,
    has_keylog: bool,
}

#[test]
fn test_all_fixtures_parse_without_panic() {
    let manifest = load_manifest();
    for fixture in &manifest.fixtures {
        let path = fixture_path(&fixture.path);
        if !path.exists() { continue; } // skip if not downloaded yet
        let result = std::panic::catch_unwind(|| {
            run_pipeline(&path)
        });
        assert!(result.is_ok(), "Panic on fixture: {}", fixture.path);
    }
}

#[test]
fn test_all_fixtures_produce_minimum_events() {
    let manifest = load_manifest();
    for fixture in &manifest.fixtures {
        let path = fixture_path(&fixture.path);
        if !path.exists() { continue; }
        let events = run_pipeline(&path);
        assert!(
            events.len() >= fixture.expected_events_min,
            "Fixture {} produced {} events, expected >= {}",
            fixture.path, events.len(), fixture.expected_events_min
        );
    }
}

#[test]
fn test_all_fixtures_detect_expected_protocols() {
    let manifest = load_manifest();
    for fixture in &manifest.fixtures {
        let path = fixture_path(&fixture.path);
        if !path.exists() { continue; }
        let events = run_pipeline(&path);
        let detected: HashSet<&str> = events.iter()
            .map(|e| e.protocol.as_str())
            .collect();
        for expected in &fixture.expected_protocols {
            assert!(
                detected.contains(expected.as_str()),
                "Fixture {} missing protocol {}, got {:?}",
                fixture.path, expected, detected
            );
        }
    }
}
```

### CLI smoke tests with real data
```rust
#[test]
fn test_cli_inspect_real_grpc_capture() {
    // Run: prb inspect tests/fixtures/captures/grpc/grpc_person_search*.pcapng
    // Assert: exit code 0
    // Assert: stdout contains expected protocol identifiers
}

#[test]
fn test_cli_inspect_all_fixtures() {
    // Iterate manifest, run CLI inspect on each
    // Assert: zero non-zero exit codes
}

#[test]
fn test_cli_export_csv_real_capture() {
    // Run: prb inspect --format csv <capture>
    // Assert: valid CSV output
}

#[test]
fn test_cli_export_har_real_capture() {
    // Run: prb inspect --format har <capture>
    // Assert: valid JSON output
}
```

### Performance baseline
```rust
#[test]
#[ignore] // Run manually or in CI nightly
fn test_performance_baseline_all_fixtures() {
    let manifest = load_manifest();
    for fixture in &manifest.fixtures {
        let path = fixture_path(&fixture.path);
        if !path.exists() { continue; }
        let start = Instant::now();
        let events = run_pipeline(&path);
        let elapsed = start.elapsed();
        eprintln!(
            "PERF: {} → {} events in {:?} ({:.0} events/sec)",
            fixture.path, events.len(), elapsed,
            events.len() as f64 / elapsed.as_secs_f64()
        );
        // Assert reasonable upper bound
        assert!(elapsed < Duration::from_secs(30),
            "Fixture {} took too long: {:?}", fixture.path, elapsed);
    }
}
```

### .gitattributes for fixtures
```
tests/fixtures/captures/**/*.pcap filter=lfs diff=lfs merge=lfs -text
tests/fixtures/captures/**/*.pcapng filter=lfs diff=lfs merge=lfs -text
```
Only if total fixture size exceeds 50MB — otherwise commit directly.

## Pre-Mortem Risks

- This segment MUST run last since it depends on all fixture files being present
- Some fixtures may not exist if prior segments were skipped — manifest tests handle this with `if !path.exists() { continue; }`
- Performance baseline numbers will vary per machine — use generous upper bounds
- Git LFS setup may be needed if fixture total exceeds 50MB

## Build and Test Commands

- Build: `cargo check --workspace`
- Test (targeted): `cargo nextest run -E 'test(regression_suite) | test(real_data_cli)'`
- Test (ignored/perf): `cargo nextest run -E 'test(performance_baseline)' -- --ignored`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Fixture registry:** README.md documents all fixtures with source URLs and licenses
2. **Machine manifest:** manifest.json with metadata for every fixture file
3. **Regression suite:** Unified test iterates all fixtures — no panics, minimum event counts met
4. **CLI smoke tests:** `prb inspect` succeeds on all fixtures
5. **Export validation:** CSV + HAR export from CLI produces valid output
6. **Performance baseline:** All fixtures process in < 30 seconds each
7. **Regression tests:** `cargo nextest run --workspace` — no regressions
8. **Full build gate:** `cargo build --workspace`
9. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
10. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
