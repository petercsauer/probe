# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive documentation: README, docs/ directory, per-crate READMEs
- AGPL-3.0 license
- CONTRIBUTING.md, SECURITY.md

## [0.1.0] - 2026-03-10

### Added

#### Core
- `DebugEvent` data model with nanosecond timestamps, payload variants, metadata, and correlation keys
- `CaptureAdapter` trait for pluggable input sources
- `ProtocolDecoder` trait for protocol-specific decoding
- `CorrelationStrategy` trait for event grouping
- `ConversationEngine` for reconstructing request/response conversations
- OpenTelemetry trace context extraction (W3C traceparent, B3, Jaeger)
- Event metrics computation

#### Protocol Decoders
- gRPC/HTTP2: frame parsing, HPACK decompression, protobuf extraction
- ZMTP: greeting, handshake, commands, message frames, PUB/SUB topics
- DDS-RTPS: header parsing, submessage decoding, serialized data extraction

#### Network Pipeline
- PCAP and pcapng file reading
- TCP stream reassembly with out-of-order segment handling
- TLS decryption via SSLKEYLOGFILE and pcapng DSB
- Parallel pipeline with flow-based sharding
- Protocol auto-detection (port mapping, magic bytes, heuristics)

#### CLI
- `prb ingest` -- PCAP/fixture to NDJSON/MCAP conversion
- `prb inspect` -- NDJSON inspection with filters and trace grouping
- `prb tui` -- interactive terminal UI with event list, decode tree, hex dump, and timeline
- `prb export` -- CSV, HAR, OTLP JSON, HTML export
- `prb merge` -- OTLP trace and packet event merging
- `prb capture` -- live network capture with real-time decoding
- `prb schemas` -- protobuf schema management
- `prb plugins` -- decoder plugin management

#### Storage
- MCAP session storage with embedded schemas
- Protobuf schema registry (.proto and .desc files)
- Wire-format and schema-backed protobuf decoding

#### Query Language
- Boolean expressions with AND, OR, NOT, parentheses
- Comparison operators: ==, !=, >, >=, <, <=
- `contains` for case-insensitive substring matching
- `exists` for field presence checking
- Metadata field access via dotted paths

#### Export
- CSV with flat event columns
- HAR (HTTP Archive) for browser tools
- OTLP JSON for trace backends
- HTML self-contained reports
- OTLP import and trace-packet merging

#### Plugin System
- Plugin API with semver versioning
- Native shared library plugins (.so/.dylib/.dll)
- WebAssembly plugin runtime
- `prb_export_plugin!` macro for FFI boilerplate

#### Live Capture
- libpcap-based packet capture
- BPF filter support
- Live TUI mode
- Configurable buffer size and snap length
