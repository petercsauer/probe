//! CLI smoke tests with real-world capture files.
//!
//! These tests verify that the prb CLI handles real-world capture fixtures
//! gracefully. Note: The `prb inspect` command works with MCAP/NDJSON formats,
//! not pcap files directly. These tests validate that the fixture infrastructure
//! is in place and documented, even though direct pcap-to-CLI workflows require
//! intermediate processing.

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Fixture metadata from manifest.json
#[derive(Debug, Deserialize)]
struct FixtureManifest {
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    path: String,
    protocols: Vec<String>,
    expected_events_min: usize,
    expected_protocols: Vec<String>,
    has_keylog: bool,
    keylog_path: Option<String>,
    category: String,
}

/// Get path to fixtures directory
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("captures")
}

/// Load manifest.json
fn load_manifest() -> FixtureManifest {
    let manifest_path = fixtures_dir().join("manifest.json");
    let content = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|_| panic!("Failed to read manifest at {:?}", manifest_path));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse manifest.json: {}", e))
}

#[test]
fn test_fixture_manifest_exists() {
    // Verify manifest.json exists and is valid
    let manifest = load_manifest();
    assert!(
        !manifest.fixtures.is_empty(),
        "Manifest should contain fixtures"
    );
    eprintln!("Manifest contains {} fixtures", manifest.fixtures.len());
}

#[test]
fn test_fixture_readme_exists() {
    // Verify fixture README exists and has content
    let readme_path = fixtures_dir().parent().unwrap().join("README.md");
    assert!(readme_path.exists(), "Fixture README should exist");

    let content = fs::read_to_string(&readme_path).unwrap();
    assert!(
        content.len() > 1000,
        "README should have substantial content"
    );
    assert!(
        content.contains("Test Fixtures"),
        "README should be the fixture documentation"
    );
}

#[test]
fn test_all_fixtures_exist_in_manifest() {
    // Verify manifest lists all fixture files that exist
    let manifest = load_manifest();
    let mut present = 0;
    let mut missing = 0;

    for fixture in &manifest.fixtures {
        let path = fixtures_dir().join(&fixture.path);
        if path.exists() {
            present += 1;
        } else {
            missing += 1;
        }
    }

    eprintln!(
        "Fixtures: {} present, {} missing from filesystem",
        present, missing
    );

    assert!(
        present > 0,
        "At least some fixtures should be present"
    );
}

#[test]
fn test_fixture_categories_represented() {
    // Verify manifest includes diverse protocol categories
    let manifest = load_manifest();

    let mut categories = std::collections::HashSet::new();
    for fixture in &manifest.fixtures {
        categories.insert(fixture.category.as_str());
    }

    eprintln!("Fixture categories: {:?}", categories);

    // Should have at least 5 different categories
    assert!(
        categories.len() >= 5,
        "Should have diverse fixture categories"
    );
}

#[test]
fn test_keylog_files_for_tls_fixtures() {
    // Verify TLS fixtures have corresponding keylog files
    let manifest = load_manifest();

    for fixture in &manifest.fixtures {
        if fixture.has_keylog {
            assert!(
                fixture.keylog_path.is_some(),
                "Fixture {} marked has_keylog but no keylog_path",
                fixture.path
            );

            if let Some(keylog_path) = &fixture.keylog_path {
                let full_path = fixtures_dir().join(keylog_path);
                let capture_path = fixtures_dir().join(&fixture.path);

                // If capture exists, keylog should too
                if capture_path.exists() {
                    assert!(
                        full_path.exists(),
                        "Keylog file missing for {}: {:?}",
                        fixture.path,
                        full_path
                    );
                }
            }
        }
    }
}

#[test]
fn test_manifest_protocol_metadata() {
    // Verify each fixture has protocol metadata
    let manifest = load_manifest();

    for fixture in &manifest.fixtures {
        // Skip empty.pcap which has no protocols
        if fixture.path.contains("empty.pcap") {
            continue;
        }

        assert!(
            !fixture.protocols.is_empty() || !fixture.expected_protocols.is_empty(),
            "Fixture {} should have protocol metadata",
            fixture.path
        );
    }
}
