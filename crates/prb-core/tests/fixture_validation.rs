//! Integration test to validate fixture file deserialization.

use prb_core::DebugEvent;
use std::fs;

#[test]
fn test_sample_fixture_deserializes() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.json"
    );
    let json_content = fs::read_to_string(fixture_path)
        .expect("failed to read sample.json fixture");

    let event: DebugEvent = serde_json::from_str(&json_content)
        .expect("failed to deserialize sample.json");

    // Verify key fields
    assert_eq!(event.id.as_u64(), 1);
    assert_eq!(event.timestamp.as_nanos(), 1678901234567890123);
    assert_eq!(event.source.adapter, "json-fixture");
    assert_eq!(event.source.origin, "fixtures/sample.json");
}
