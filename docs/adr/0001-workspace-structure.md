# ADR 0001: Cargo Workspace Structure

## Status

Accepted

## Context

PRB needs to support multiple protocol decoders (gRPC, ZMQ, DDS) with a clean
separation between core types, protocol-specific logic, and user interfaces.

## Decision

Use a Cargo workspace with 20 crates organized by responsibility:

- **Core**: prb-core (types, traits), prb-storage (persistence)
- **Ingestion**: prb-pcap (packet parsing), prb-capture (live capture), prb-fixture (test data)
- **Protocols**: prb-grpc, prb-zmq, prb-dds (protocol decoders)
- **Detection**: prb-detect (auto-detection), prb-schema, prb-decode (schema-based decoding)
- **Output**: prb-tui (terminal UI), prb-export (file export), prb-query (filtering)
- **Extensibility**: prb-plugin-api, prb-plugin-native, prb-plugin-wasm
- **Experimental**: prb-ai (LLM explanations)
- **CLI**: prb-cli (command-line interface)
- **Testing**: prb-integration-tests

## Consequences

**Positive:**
- Clear separation of concerns
- Independent versioning possible
- Parallel compilation
- Easy to add new protocols
- Optional dependencies for specific features

**Negative:**
- More Cargo.toml files to maintain
- Longer compile times for workspace
- More complex dependency graph

## Alternatives Considered

- Monolithic single-crate design: Rejected (poor modularity)
- Separate repositories: Rejected (harder to coordinate changes)
