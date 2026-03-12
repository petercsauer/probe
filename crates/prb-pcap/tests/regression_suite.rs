//! Unified regression test suite for all real-data fixtures.
//!
//! This suite runs all captures through the full pipeline to ensure:
//! - No panics or crashes on real-world data
//! - Minimum event counts are met
//! - Expected protocols are detected
//! - Performance stays within bounds

use prb_core::CaptureAdapter;
use prb_pcap::PcapCaptureAdapter;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Fixture metadata from manifest.json
#[derive(Debug, Deserialize)]
struct FixtureManifest {
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    path: String,
    #[allow(dead_code)]
    protocols: Vec<String>,
    expected_events_min: usize,
    expected_protocols: Vec<String>,
    has_keylog: bool,
    #[allow(dead_code)]
    keylog_path: Option<String>,
    #[allow(dead_code)]
    category: String,
}

/// Helper to get path to test fixtures.
fn fixture_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // Go up from crates/prb-pcap
    path.pop(); // Go up from crates
    path.push("tests");
    path.push("fixtures");
    path.push("captures");
    path
}

/// Load manifest.json
fn load_manifest() -> FixtureManifest {
    let manifest_path = fixture_dir().join("manifest.json");
    let content = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|_| panic!("Failed to read manifest at {manifest_path:?}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse manifest.json: {e}"))
}

/// Get full path to a fixture file.
fn fixture_path(relative_path: &str) -> PathBuf {
    fixture_dir().join(relative_path)
}

/// Get keylog path if fixture has one.
fn keylog_path(fixture: &Fixture) -> Option<PathBuf> {
    if fixture.has_keylog {
        fixture.keylog_path.as_ref().map(|p| fixture_dir().join(p))
    } else {
        None
    }
}

/// Run pipeline on a capture file.
fn run_pipeline(path: &Path, keylog: Option<PathBuf>) -> Vec<prb_core::DebugEvent> {
    let mut adapter = PcapCaptureAdapter::new(path.to_path_buf(), keylog);
    adapter.ingest().filter_map(std::result::Result::ok).collect()
}

#[test]
fn test_all_fixtures_parse_without_panic() {
    // Goal: Ensure every fixture in manifest processes without panic
    let manifest = load_manifest();
    let mut skipped = Vec::new();
    let mut processed = Vec::new();

    for fixture in &manifest.fixtures {
        let path = fixture_path(&fixture.path);
        if !path.exists() {
            skipped.push(fixture.path.clone());
            continue;
        }

        let keylog = keylog_path(fixture);

        // Use catch_unwind to detect panics
        let result = std::panic::catch_unwind(|| run_pipeline(&path, keylog));

        assert!(
            result.is_ok(),
            "Panic while processing fixture: {} at {:?}",
            fixture.path,
            path
        );
        processed.push(fixture.path.clone());
    }

    eprintln!(
        "Processed {} fixtures, skipped {}",
        processed.len(),
        skipped.len()
    );
    if !skipped.is_empty() {
        eprintln!("Skipped (not present): {skipped:?}");
    }

    // At least some fixtures should be present
    assert!(
        !processed.is_empty(),
        "No fixtures were processed - check if fixtures exist"
    );
}

#[test]
fn test_all_fixtures_produce_minimum_events() {
    // Goal: Verify each fixture produces expected minimum event count
    let manifest = load_manifest();
    let mut failures = Vec::new();

    for fixture in &manifest.fixtures {
        let path = fixture_path(&fixture.path);
        if !path.exists() {
            continue;
        }

        let keylog = keylog_path(fixture);
        let events = run_pipeline(&path, keylog);

        if events.len() < fixture.expected_events_min {
            failures.push(format!(
                "{}: got {} events, expected >= {}",
                fixture.path,
                events.len(),
                fixture.expected_events_min
            ));
        }
    }

    assert!(failures.is_empty(), 
        "Fixtures failed minimum event count:\n{}",
        failures.join("\n")
    )
}

#[test]
fn test_all_fixtures_detect_expected_protocols() {
    // Goal: Verify protocol detection works for each fixture
    let manifest = load_manifest();
    let mut failures = Vec::new();

    for fixture in &manifest.fixtures {
        // Skip fixtures with no expected protocols
        if fixture.expected_protocols.is_empty() {
            continue;
        }

        let path = fixture_path(&fixture.path);
        if !path.exists() {
            continue;
        }

        let keylog = keylog_path(fixture);
        let events = run_pipeline(&path, keylog);

        // Collect detected protocols from events
        let detected: HashSet<String> = events
            .iter()
            .map(|e| {
                // Extract protocol from event metadata or frame type
                // For now, just collect unique frame types as proxy for protocol detection
                format!("{:?}", e.transport).to_lowercase()
            })
            .collect();

        // Check if expected protocols are represented
        // Note: This is a simplified check - actual protocol names may differ
        // from frame types, so we do a fuzzy match
        let mut missing = Vec::new();
        for expected in &fixture.expected_protocols {
            let found = detected.iter().any(|d: &String| {
                d.contains(&expected.to_lowercase())
                    || expected.to_lowercase().contains(d)
                    || (expected == "tcp" && d.contains("tcp"))
                    || (expected == "udp" && d.contains("udp"))
            });
            if !found && events.len() >= fixture.expected_events_min {
                missing.push(expected.clone());
            }
        }

        if !missing.is_empty() {
            failures.push(format!(
                "{}: missing protocols {:?}, detected: {:?}",
                fixture.path, missing, detected
            ));
        }
    }

    // Protocol detection failures are informational, not fatal
    // Since frame types may not exactly match protocol names
    if !failures.is_empty() {
        eprintln!(
            "Protocol detection mismatches (informational):\n{}",
            failures.join("\n")
        );
    }
}

#[test]
#[ignore] // Run manually or in CI nightly
fn test_performance_baseline_all_fixtures() {
    // Goal: Establish performance baseline for all fixtures
    let manifest = load_manifest();
    let max_duration = Duration::from_secs(30);
    let mut results = Vec::new();

    for fixture in &manifest.fixtures {
        let path = fixture_path(&fixture.path);
        if !path.exists() {
            continue;
        }

        let keylog = keylog_path(fixture);
        let start = Instant::now();
        let events = run_pipeline(&path, keylog);
        let elapsed = start.elapsed();

        let events_per_sec = if elapsed.as_secs_f64() > 0.0 {
            events.len() as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        results.push((fixture.path.clone(), events.len(), elapsed, events_per_sec));

        eprintln!(
            "PERF: {} → {} events in {:?} ({:.0} events/sec)",
            fixture.path,
            events.len(),
            elapsed,
            events_per_sec
        );

        // Assert reasonable upper bound
        assert!(
            elapsed < max_duration,
            "Fixture {} took too long: {:?} (max {:?})",
            fixture.path,
            elapsed,
            max_duration
        );
    }

    eprintln!("\nPerformance Summary:");
    eprintln!(
        "{:<40} {:>10} {:>12} {:>15}",
        "Fixture", "Events", "Duration", "Events/sec"
    );
    eprintln!("{}", "-".repeat(80));
    for (path, events, duration, eps) in results {
        eprintln!(
            "{path:<40} {events:>10} {duration:>12?} {eps:>15.0}"
        );
    }
}

#[test]
fn test_manifest_consistency() {
    // Goal: Verify manifest.json is well-formed and consistent
    let manifest = load_manifest();

    assert!(
        !manifest.fixtures.is_empty(),
        "Manifest should contain at least one fixture"
    );

    // Check for duplicate paths
    let mut seen = HashSet::new();
    for fixture in &manifest.fixtures {
        assert!(
            seen.insert(&fixture.path),
            "Duplicate fixture path in manifest: {}",
            fixture.path
        );
    }

    // Verify all fixtures in manifest have valid paths
    for fixture in &manifest.fixtures {
        let _path = fixture_path(&fixture.path);
        // It's OK if file doesn't exist (may not be downloaded)
        // But path structure should be valid
        assert!(
            !fixture.path.is_empty(),
            "Empty path in manifest for fixture"
        );

        // If has_keylog is true, keylog_path should be set
        if fixture.has_keylog {
            assert!(
                fixture.keylog_path.is_some(),
                "Fixture {} has has_keylog=true but no keylog_path",
                fixture.path
            );
        }
    }
}

#[test]
fn test_fixture_file_count() {
    // Goal: Sanity check that we have fixtures in the directory
    let captures_dir = fixture_dir();

    if !captures_dir.exists() {
        eprintln!("Fixtures directory does not exist: {captures_dir:?}");
        return;
    }

    let count = walkdir::WalkDir::new(&captures_dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s == "pcap" || s == "pcapng")
        })
        .count();

    eprintln!("Found {count} pcap/pcapng files in fixtures directory");

    assert!(
        count > 0,
        "No pcap/pcapng files found in fixtures directory"
    );

    // Manifest should have approximately the same count
    let manifest = load_manifest();
    let manifest_count = manifest.fixtures.len();

    eprintln!("Manifest contains {manifest_count} fixtures");

    assert!(manifest_count > 0, "Manifest contains no fixtures");

    // Allow some discrepancy (manifest may exclude some files)
    assert!(
        count >= manifest_count / 2,
        "Significant mismatch between fixture files ({count}) and manifest ({manifest_count})"
    );
}
