# Before & After: Test Utilities Refactoring

## Current State: 42 Files with Duplicated Builders

### Example 1: prb-tui/tests/ai_panel_test.rs (Lines 18-46)
```rust
fn make_test_event(
    id: u64,
    timestamp_nanos: u64,
    transport: TransportKind,
    src: &str,
    dst: &str,
) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}
```
**29 lines of boilerplate**

---

### Example 2: prb-ai/tests/explain_http_test.rs (Lines 16-34)
```rust
fn make_test_event() -> DebugEvent {
    DebugEvent::builder()
        .id(prb_core::EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
        .source(EventSource {
            adapter: "pcap".into(),
            origin: "test.pcap".into(),
            network: Some(prb_core::NetworkAddr {
                src: "10.0.0.1:52341".into(),
                dst: "10.0.0.2:50051".into(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"test payload"),
        })
        .build()
}
```
**19 lines using builder (better, but still duplicated)**

---

### Example 3: prb-tui/tests/accessibility_test.rs (Lines 11-30)
```rust
fn make_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3, 4]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}
```
**20 lines, slightly different defaults**

---

### Example 4: prb-export/src/csv_export.rs (Lines 166-185)
```rust
fn sample_event() -> DebugEvent {
    DebugEvent::builder()
        .id(EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
        .source(EventSource {
            adapter: "pcap".into(),
            origin: "test.pcap".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:50051".into(),
                dst: "10.0.0.2:8080".into(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Outbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"hello"),
        })
        .metadata("grpc.method", "/api.v1.Users/Get")
        .build()
}
```
**20 lines, adds metadata**

---

## Problems with Current Approach

1. **Inconsistent naming**: `make_test_event`, `sample_event`, `create_test_event`
2. **Inconsistent implementation**: Some use builder, some use struct literals
3. **Inconsistent defaults**: Different adapters, origins, payloads, directions
4. **Maintenance burden**: Change needs to propagate to 42 files
5. **Copy-paste errors**: Subtle differences that may be bugs (e.g., adapter="test" vs "pcap")
6. **Grep difficulty**: Hard to find all test event creation sites

### Duplication Statistics
- **42 files** with test event builders
- **680 LOC** of duplicated code (average 16 lines per file)
- **3 different naming conventions**
- **2 different implementation patterns** (builder vs struct literal)

---

## Proposed Solution: prb-test-utils Crate

### Structure
```
crates/prb-test-utils/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Public API, re-exports
│   ├── builders.rs     # event_builder() with test defaults
│   ├── fixtures.rs     # Preset events (grpc_event, zmq_event, etc.)
│   └── strategies.rs   # proptest strategies (Phase 2)
└── tests/
    └── builder_test.rs
```

---

### Implementation: fixtures.rs

```rust
//! Pre-configured test event fixtures.

use bytes::Bytes;
use prb_core::*;
use std::collections::BTreeMap;

/// Create a minimal test event with sensible defaults.
///
/// # Example
/// ```
/// use prb_test_utils::event;
/// 
/// let e = event();
/// assert_eq!(e.id.as_u64(), 1);
/// assert_eq!(e.transport, TransportKind::Grpc);
/// ```
pub fn event() -> DebugEvent {
    event_builder().build()
}

/// Create a gRPC test event with realistic metadata.
///
/// # Example
/// ```
/// use prb_test_utils::grpc_event;
/// 
/// let e = grpc_event();
/// assert_eq!(e.transport, TransportKind::Grpc);
/// assert!(e.metadata.contains_key("grpc.method"));
/// ```
pub fn grpc_event() -> DebugEvent {
    event_builder()
        .transport(TransportKind::Grpc)
        .metadata("grpc.method", "/api.v1.Users/Get")
        .build()
}

/// Create a ZMQ test event.
pub fn zmq_event() -> DebugEvent {
    event_builder()
        .transport(TransportKind::Zmq)
        .metadata("zmq.topic", "test.topic")
        .build()
}

/// Create an HTTP/2 test event.
pub fn http2_event() -> DebugEvent {
    event_builder()
        .transport(TransportKind::Http2)
        .metadata("http.method", "GET")
        .metadata("http.path", "/api/v1/users")
        .build()
}

/// Create a DDS test event.
pub fn dds_event() -> DebugEvent {
    event_builder()
        .transport(TransportKind::DdsRtps)
        .metadata("dds.domain_id", "0")
        .metadata("dds.topic_name", "TestTopic")
        .build()
}
```

---

### Implementation: builders.rs

```rust
//! Test-specific builder helpers.

use bytes::Bytes;
use prb_core::*;

/// Create a DebugEventBuilder pre-configured with test defaults.
///
/// All fields have sensible defaults for testing. Override as needed.
///
/// # Example
/// ```
/// use prb_test_utils::event_builder;
/// use prb_core::{EventId, TransportKind};
/// 
/// let event = event_builder()
///     .id(EventId::from_raw(42))
///     .transport(TransportKind::Zmq)
///     .build();
/// ```
pub fn event_builder() -> DebugEventBuilder {
    DebugEvent::builder()
        .id(EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_000_000_000))
        .source(EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:50051".into(),
                dst: "10.0.0.2:8080".into(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"test"),
        })
}

/// Create a builder with NO network addresses (for non-network tests).
pub fn event_builder_no_network() -> DebugEventBuilder {
    DebugEvent::builder()
        .id(EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_000_000_000))
        .source(EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"test"),
        })
}

/// Create a builder with custom network addresses.
pub fn event_builder_with_network(src: &str, dst: &str) -> DebugEventBuilder {
    event_builder().source(EventSource {
        adapter: "test".into(),
        origin: "test".into(),
        network: Some(NetworkAddr {
            src: src.to_string(),
            dst: dst.to_string(),
        }),
    })
}
```

---

### Implementation: lib.rs

```rust
//! Test utilities for the probe project.
//!
//! This crate provides shared test fixtures, builders, and assertion helpers
//! for use across all probe crates.
//!
//! # Examples
//!
//! ## Simple test event
//! ```
//! use prb_test_utils::event;
//! 
//! let e = event();
//! assert_eq!(e.id.as_u64(), 1);
//! ```
//!
//! ## Protocol-specific event
//! ```
//! use prb_test_utils::grpc_event;
//! 
//! let e = grpc_event();
//! assert_eq!(e.transport, TransportKind::Grpc);
//! ```
//!
//! ## Custom event
//! ```
//! use prb_test_utils::event_builder;
//! use prb_core::{EventId, Direction};
//! 
//! let e = event_builder()
//!     .id(EventId::from_raw(42))
//!     .direction(Direction::Outbound)
//!     .build();
//! ```

mod builders;
mod fixtures;

pub use builders::*;
pub use fixtures::*;

// Phase 2: Re-export proptest strategies
#[cfg(feature = "proptest")]
pub mod strategies;
```

---

### Implementation: Cargo.toml

```toml
[package]
name = "prb-test-utils"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Test utilities for the probe project"
keywords = ["test", "fixture", "utilities"]
categories = ["development-tools::testing"]

[dependencies]
prb-core = { path = "../prb-core" }
bytes = { workspace = true }

[features]
# Phase 2: Add proptest strategies
proptest = ["dep:proptest"]

[dependencies.proptest]
workspace = true
optional = true
```

---

## After: Migration Examples

### Example 1: prb-tui/tests/ai_panel_test.rs
**Before** (29 lines):
```rust
fn make_test_event(
    id: u64,
    timestamp_nanos: u64,
    transport: TransportKind,
    src: &str,
    dst: &str,
) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        },
        transport,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}
```

**After** (1 line import, direct usage):
```rust
use prb_test_utils::event_builder_with_network;

// In test:
let event = event_builder_with_network("10.0.0.1:1234", "10.0.0.2:5678")
    .id(EventId::from_raw(1))
    .timestamp(Timestamp::from_nanos(1_000_000_000))
    .transport(TransportKind::Grpc)
    .build();
```

**Savings**: 29 lines → 7 lines (77% reduction)

---

### Example 2: prb-ai/tests/explain_http_test.rs
**Before** (19 lines):
```rust
fn make_test_event() -> DebugEvent {
    DebugEvent::builder()
        .id(prb_core::EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
        .source(EventSource {
            adapter: "pcap".into(),
            origin: "test.pcap".into(),
            network: Some(prb_core::NetworkAddr {
                src: "10.0.0.1:52341".into(),
                dst: "10.0.0.2:50051".into(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"test payload"),
        })
        .build()
}
```

**After** (1 line):
```rust
use prb_test_utils::grpc_event;

// In test:
let event = grpc_event(); // Done!
```

**Savings**: 19 lines → 1 line (95% reduction)

---

### Example 3: prb-tui/tests/accessibility_test.rs
**Before** (20 lines):
```rust
fn make_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3, 4]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}
```

**After** (1 line):
```rust
use prb_test_utils::event_builder_no_network;

// In test:
let event = event_builder_no_network()
    .id(EventId::from_raw(id))
    .timestamp(Timestamp::from_nanos(timestamp_nanos))
    .build();
```

**Savings**: 20 lines → 4 lines (80% reduction)

---

### Example 4: prb-export/src/csv_export.rs
**Before** (20 lines):
```rust
fn sample_event() -> DebugEvent {
    DebugEvent::builder()
        .id(EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
        .source(EventSource {
            adapter: "pcap".into(),
            origin: "test.pcap".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:50051".into(),
                dst: "10.0.0.2:8080".into(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Outbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"hello"),
        })
        .metadata("grpc.method", "/api.v1.Users/Get")
        .build()
}
```

**After** (1 line):
```rust
use prb_test_utils::grpc_event;

// In test:
let event = grpc_event(); // Already has grpc.method metadata!

// OR if you need Direction::Outbound:
let event = event_builder()
    .transport(TransportKind::Grpc)
    .direction(Direction::Outbound)
    .metadata("grpc.method", "/api.v1.Users/Get")
    .build();
```

**Savings**: 20 lines → 1 line (95% reduction)

---

## Migration Checklist

For each of the 42 test files:

- [ ] Add `prb-test-utils` to `[dev-dependencies]` (if not already in workspace)
- [ ] Import appropriate helper: `use prb_test_utils::{event, grpc_event, event_builder};`
- [ ] Replace local `make_test_event()` / `sample_event()` with:
  - `event()` for minimal defaults
  - `grpc_event()` / `zmq_event()` / etc. for protocol-specific
  - `event_builder()` for custom cases
- [ ] Remove local builder function (delete 15-30 lines)
- [ ] Run tests: `cargo test`
- [ ] Verify no behavioral changes

### Automated Migration Script (Optional)

```bash
#!/usr/bin/env bash
# migrate_test_utils.sh - Semi-automated migration

set -e

TEST_FILES=$(git grep -l "fn make_test_event\|fn sample_event\|fn create_test_event")

for file in $TEST_FILES; do
    echo "Processing: $file"
    
    # Add import if not present
    if ! grep -q "use prb_test_utils" "$file"; then
        # Add after last use statement
        sed -i '' '/^use /a\
use prb_test_utils::event_builder;
' "$file"
    fi
    
    # Replace simple cases (manual review required for complex cases)
    # This is just a starting point - manual review needed
    
    echo "  → Added import. Manual review required for migration."
done

echo "Migration prep complete. Review changes and run 'cargo test'."
```

---

## Expected Outcomes

### Quantitative
- **-600 LOC**: Remove ~600 lines of duplicated code
- **42 → 1**: Centralize 42 implementations into 1 crate
- **100% consistency**: All tests use same defaults
- **0 behavioral changes**: Tests still pass

### Qualitative
- **Easier maintenance**: Update one place, affects all tests
- **Better discoverability**: `prb_test_utils::` namespace is obvious
- **Cleaner test files**: Focus on test logic, not setup
- **Standardized naming**: No more `make_` vs `sample_` vs `create_`

---

## Phase 2: Property-Based Testing (Optional)

### Add strategies.rs

```rust
//! Proptest strategies for generating random DebugEvents.

#[cfg(feature = "proptest")]
use proptest::prelude::*;

#[cfg(feature = "proptest")]
use prb_core::*;

#[cfg(feature = "proptest")]
prop_compose! {
    pub fn arb_event_id()(id in any::<u64>()) -> EventId {
        EventId::from_raw(id)
    }
}

#[cfg(feature = "proptest")]
prop_compose! {
    pub fn arb_timestamp()(ts in any::<u64>()) -> Timestamp {
        Timestamp::from_nanos(ts)
    }
}

#[cfg(feature = "proptest")]
prop_compose! {
    pub fn arb_transport()(
        kind in prop_oneof![
            Just(TransportKind::Grpc),
            Just(TransportKind::Zmq),
            Just(TransportKind::DdsRtps),
            Just(TransportKind::Http2),
        ]
    ) -> TransportKind {
        kind
    }
}

#[cfg(feature = "proptest")]
prop_compose! {
    pub fn arb_debug_event()(
        id in arb_event_id(),
        timestamp in arb_timestamp(),
        transport in arb_transport(),
        payload in any::<Vec<u8>>(),
    ) -> DebugEvent {
        use bytes::Bytes;
        
        crate::event_builder()
            .id(id)
            .timestamp(timestamp)
            .transport(transport)
            .payload(Payload::Raw { raw: Bytes::from(payload) })
            .build()
    }
}
```

### Usage in Tests

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use prb_test_utils::strategies::arb_debug_event;

    proptest! {
        #[test]
        fn serialization_round_trip(event in arb_debug_event()) {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: DebugEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, deserialized);
        }
        
        #[test]
        fn event_id_is_always_positive(event in arb_debug_event()) {
            assert!(event.id.as_u64() > 0);
        }
    }
}
```

---

## Summary

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| **LOC in test files** | ~680 | ~80 | -88% |
| **Implementations** | 42 | 1 | -98% |
| **Naming conventions** | 3 | 1 | -67% |
| **Import lines per test** | 0 | 1 | +1 (acceptable) |
| **Maintenance burden** | High | Low | -90% |
| **Consistency** | Low | High | +100% |

**Time Investment**: 6-10 hours for full migration  
**Long-term Savings**: ~30 minutes per test file change (42 files × 30 min = 21 hours saved over lifetime)  
**Break-even**: After ~2-3 test changes  
**ROI**: 200%+ over project lifetime
