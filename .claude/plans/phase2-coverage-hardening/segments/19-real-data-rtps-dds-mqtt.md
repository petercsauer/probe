---
segment: 19
title: "Real-Data Tests: RTPS/DDS, MQTT, and IoT Protocol Captures"
depends_on: [11, 13]
risk: 4
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(prb-pcap,prb-decode,prb-detect): add real-data RTPS/DDS, MQTT, and IoT protocol tests"
---

# Segment 19: Real-Data Tests — RTPS/DDS, MQTT, and IoT Protocol Captures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world RTPS/DDS, MQTT, CoAP, and other IoT protocol captures. Write tests verifying the protocol detection and decode pipeline handles them.

**Depends on:** Segments 11 (integration tests), 13 (gRPC/H2 real data — same pattern)

## Data Sources

### RTPS / DDS
1. **From RTI Connext documentation** — Example captures at:
   - `https://community.rti.com/static/documentation/wireshark/2020-07/doc/examples.html`
   - Contains RTPS traffic examples with annotated packet analysis
2. **From Wireshark wiki** — search for "RTPS" samples
   - `rtps_sample.pcap` (if available)
3. **From Wireshark GitLab issues** — Bug reports with RTPS pcap attachments
   - Search: `https://gitlab.com/wireshark/wireshark/-/issues?search=rtps+pcap`
4. **Self-generated** (if no public captures available):
   ```bash
   # Using eProsima Fast DDS examples
   # Terminal 1: run publisher
   # Terminal 2: run subscriber
   # Terminal 3: capture
   tcpdump -i lo0 port 7400-7500 -w rtps_sample.pcap
   ```

### MQTT
5. **From Wireshark wiki** — MQTT sample captures
   - `mqtt.pcap` or `mqtt-v5.pcap` (check SampleCaptures page)
6. **From CloudShark** — public MQTT capture examples
7. **Self-generated** (easy to create):
   ```bash
   tcpdump -i lo0 port 1883 -w mqtt_sample.pcap &
   mosquitto_sub -t test/# &
   mosquitto_pub -t test/hello -m "world"
   ```

### CoAP (Constrained Application Protocol)
8. **From Wireshark wiki** — CoAP sample captures
   - `coap.pcap` (check SampleCaptures page)
9. **From Eclipse Californium** test suite — CoAP reference implementation

### AMQP
10. **From Wireshark wiki** — AMQP 0-9-1 and AMQP 1.0 samples
    - `amqp.pcap` (if available)

Store fixtures in `tests/fixtures/captures/rtps/`, `tests/fixtures/captures/mqtt/`, `tests/fixtures/captures/iot/`.

## Scope

- `tests/fixtures/captures/rtps/*.pcap` — RTPS/DDS captures
- `tests/fixtures/captures/mqtt/*.pcap` — MQTT captures
- `tests/fixtures/captures/iot/*.pcap` — CoAP, AMQP, etc.
- `crates/prb-pcap/tests/real_data_iot_tests.rs` — New test file
- `crates/prb-detect/tests/real_data_detect_tests.rs` — Protocol detection tests

## Implementation Approach

### RTPS/DDS tests
```rust
#[test]
fn test_rtps_discovery_real() {
    // Load RTPS capture
    // Assert: RTPS protocol detected by protocol detector
    // Assert: participant discovery messages found (SPDP)
    // Assert: domain ID and participant GUID extractable
}

#[test]
fn test_rtps_data_exchange() {
    // Load RTPS capture with actual data exchange
    // Assert: DATA submessages found
    // Assert: topic name and type name extractable from discovery
    // Assert: serialized data payloads present
}

#[test]
fn test_dds_multicast_discovery() {
    // RTPS uses multicast for discovery (239.255.0.x)
    // Assert: multicast packets correctly handled in normalization
    // Assert: multiple participants discovered
}
```

### MQTT tests
```rust
#[test]
fn test_mqtt_connect_publish_subscribe() {
    // Load MQTT capture
    // Assert: CONNECT, CONNACK, PUBLISH, SUBSCRIBE, SUBACK detected
    // Assert: topic names extracted
    // Assert: QoS levels identified
}

#[test]
fn test_mqtt_v5_properties() {
    // If MQTT v5 capture available
    // Assert: v5 properties parsed (session expiry, topic alias, etc.)
}

#[test]
fn test_mqtt_protocol_detection() {
    // Feed MQTT capture to protocol detector
    // Assert: protocol detected as "mqtt" on port 1883
}
```

### CoAP tests
```rust
#[test]
fn test_coap_request_response() {
    // Load CoAP capture
    // Assert: CoAP GET/POST/PUT/DELETE detected
    // Assert: resource URIs extracted
    // Assert: CoAP options parsed (Content-Format, etc.)
}
```

### AMQP tests
```rust
#[test]
fn test_amqp_connection_and_publish() {
    // Load AMQP capture
    // Assert: AMQP protocol header detected
    // Assert: Connection.Open, Channel.Open, Basic.Publish found
    // Assert: exchange and routing key extracted
}
```

### Protocol detection accuracy
```rust
#[test]
fn test_protocol_detector_accuracy_across_iot_protocols() {
    // Load each IoT capture
    // Run only through protocol detection (no full decode)
    // Assert: each capture's protocol detected correctly
    // Report: false positive / false negative rate = 0
}
```

## Pre-Mortem Risks

- RTPS public captures may be scarce — may need to self-generate using Fast DDS
- MQTT and CoAP may not have dedicated decoders yet — test at protocol detection level
- IoT protocols often use UDP — ensure normalization handles UDP correctly
- Multicast traffic in RTPS may confuse flow tracking

## Build and Test Commands

- Build: `cargo check -p prb-pcap -p prb-decode -p prb-detect`
- Test (targeted): `cargo nextest run -E 'test(real_data_iot) | test(real_data_detect)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 8 tests across RTPS, MQTT, CoAP/AMQP captures, all passing
2. **Fixture files:** At least 1 capture per protocol (RTPS, MQTT, CoAP or AMQP) committed
3. **Protocol detection:** Protocol detector correctly identifies each IoT protocol
4. **Regression tests:** `cargo nextest run --workspace` — no regressions
5. **Full build gate:** `cargo build --workspace`
6. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
7. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
