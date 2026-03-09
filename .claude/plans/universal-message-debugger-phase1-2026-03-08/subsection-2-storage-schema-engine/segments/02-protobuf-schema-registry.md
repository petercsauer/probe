---
segment: 2
title: "Protobuf Schema Registry"
depends_on: [1]
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(schema): add protobuf schema registry with .proto and .desc loading"
---

# Segment 2: Protobuf Schema Registry

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement the protobuf schema registry that loads, stores, and resolves message types from both pre-compiled descriptor sets and raw .proto files.

**Depends on:** Segment 1 (MCAP storage layer for schema embedding in sessions)

## Context: Issues Addressed

### S2-2: Schema Storage and Session Self-Containment (registry side)

**Core Problem:** The plan does not specify whether protobuf schemas are stored inside MCAP session files (self-contained) or kept as external files. For a debug tool, self-contained sessions are critical: users share session files with teammates, and broken external references make sessions useless.

**Proposed Fix (registry side):** When a user loads schemas (via `prb schemas load foo.desc` or `prb schemas load foo.proto`), register them in SchemaRegistry. When writing a session, embed all schemas from the registry that were used during decode as MCAP Schema records. When reading a session, extract MCAP Schema records and populate SchemaRegistry automatically. Support `prb schemas export session.mcap` to extract stored schemas for reuse.

**Pre-Mortem:** Embedding full FileDescriptorSets can be large (100KB+ for complex services). Deduplicate schemas by content hash. Schema version conflicts: session might embed schema A, user's current .proto is version B. Session should always use embedded schemas for consistency.

### S2-5: Runtime .proto Compilation Path Under-Emphasized

**Core Problem:** The plan implies users must pre-compile .proto files into .desc using protoc. However, protox (already listed as a dependency) enables runtime .proto compilation without protoc. This is a major UX improvement that deserves first-class support.

**Proposed Fix:** Support two schema loading paths with equal prominence:
1. Pre-compiled descriptors: `prb schemas load service.desc` -- loads binary FileDescriptorSet.
2. Raw .proto files: `prb schemas load service.proto --include-path ./protos/` -- compiles at runtime via protox Compiler API, resolving imports. No protoc installation required.

protox Compiler API: `Compiler::new(include_paths)?.open_files(proto_files)?.file_descriptor_set()` or `.descriptor_pool()`.

SchemaRegistry methods: `load_descriptor_set(&mut self, bytes: &[u8])`, `load_proto_files(&mut self, files: &[&Path], includes: &[&Path])`.

**Pre-Mortem:** protox may not support all protobuf language features (editions, custom options). Document supported proto2/proto3 features; fall back to .desc for unsupported. Import resolution may fail if include paths wrong -- clear error messages naming missing import and searched paths. protox compilation adds latency (~100ms) -- cache compiled DescriptorPools.

## Scope

- New crate: `crates/schema/` (prb-schema)
- Modified: `crates/storage/` (schema embedding/extraction in MCAP sessions)
- Modified: `crates/cli/` (add `prb schemas` subcommand)

## Key Files and Context

Subsection 1 produces:
- `crates/core/src/traits.rs` -- `SchemaResolver` trait. Expected signature: `fn resolve(&self, type_name: &str) -> Option<SchemaInfo>` where SchemaInfo contains encoding, name, and raw descriptor bytes.
- `crates/core/src/error.rs` -- thiserror error types.

Segment 1 produces:
- `crates/storage/src/writer.rs` -- SessionWriter with `add_schema()` support via MCAP Schema records.
- `crates/storage/src/reader.rs` -- SessionReader with schema extraction from MCAP Schema records.

Key library APIs:

prost-reflect (v0.16.3):
- `DescriptorPool::decode(bytes)` -- load from serialized FileDescriptorSet
- `DescriptorPool::from_file_descriptor_set(fds)` -- load from in-memory FDS
- `pool.add_file_descriptor_set(fds)` -- add more descriptors to existing pool
- `pool.get_message_by_name("pkg.MessageName")` -- lookup by FQN
- `pool.all_messages()` -- iterate all known message types

protox (v0.9.1):
- `protox::compile(files, includes)` -- compile .proto files to FileDescriptorSet
- `Compiler::new(includes)?.open_files(files)?.file_descriptor_set()` -- more control
- `Compiler::new(includes)?.open_files(files)?.descriptor_pool()` -- get DescriptorPool directly
- Pure Rust, no protoc dependency. Handles imports and well-known types.

MCAP Schema convention for protobuf:
- encoding = "protobuf"
- name = fully qualified message name (e.g., "foo.bar.MyMessage")
- data = binary FileDescriptorSet (with --include_imports)

## Implementation Approach

1. Create `crates/schema/` crate with deps: `prost-reflect = "0.16"`, `protox = "0.9"`, `prost-types`, plus workspace deps (prb-core, thiserror, tracing, camino).
2. Implement `SchemaRegistry` struct wrapping prost-reflect's DescriptorPool:
   ```rust
   pub struct SchemaRegistry {
       pool: DescriptorPool,
       loaded_sets: Vec<Vec<u8>>, // raw FDS bytes for MCAP embedding
   }
   ```
3. Loading methods:
   - `load_descriptor_set(&mut self, bytes: &[u8])` -- decode and add to pool. Store raw bytes for later MCAP embedding.
   - `load_descriptor_set_file(&mut self, path: &Path)` -- read file, delegate to load_descriptor_set.
   - `load_proto_files(&mut self, files: &[&Path], includes: &[&Path])` -- compile via protox, encode result to bytes, delegate to load_descriptor_set.
4. Query methods:
   - `get_message(&self, fqn: &str) -> Option<MessageDescriptor>` -- delegate to pool.
   - `list_messages(&self) -> Vec<String>` -- collect all message FQNs.
   - `list_services(&self) -> Vec<String>` -- for gRPC discovery.
   - `descriptor_sets(&self) -> &[Vec<u8>]` -- raw bytes for MCAP embedding.
5. Implement the core `SchemaResolver` trait from prb-core for SchemaRegistry.
6. Integrate with SessionWriter: when finishing a session, embed all loaded schemas as MCAP Schema records.
7. Integrate with SessionReader: when opening a session, extract MCAP Schema records and populate a SchemaRegistry.
8. Add CLI subcommand `prb schemas`:
   - `prb schemas load <path>` -- accepts .desc or .proto files (detected by extension). For .proto, accepts `--include-path`.
   - `prb schemas list <session.mcap>` -- list message types in a session's embedded schemas.
   - `prb schemas export <session.mcap> --output schemas.desc` -- export embedded schemas.

## Alternatives Ruled Out

- Use the protobuf (rust-protobuf) crate instead of prost-reflect. Rejected: ecosystem mismatch with prost/tonic stack.
- Store schemas only externally. Rejected: breaks session self-containment.
- Support only .desc files (no protox). Rejected: poor UX forcing users to install and run protoc.

## Pre-Mortem Risks

- protox may fail on complex .proto files with unusual features (custom options, proto2 extensions). Write a test with a non-trivial .proto that uses imports and nested messages.
- DescriptorPool::decode may reject malformed descriptor sets silently. Test with truncated/corrupted .desc files.
- Schema embedding bloats MCAP files if many schemas are loaded but few are used. Consider tracking which schemas were actually referenced during the session.

## Build and Test Commands

- Build: `cargo build -p prb-schema`
- Test (targeted): `cargo nextest run -p prb-schema`
- Test (regression): `cargo nextest run -p prb-core -p prb-storage -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_load_descriptor_set`: create a FileDescriptorSet programmatically (using prost-types), load it, verify message lookup by FQN succeeds.
   - `test_load_proto_file`: write a .proto file to tempdir, load via protox, verify message lookup succeeds.
   - `test_load_proto_with_imports`: write two .proto files (one imports the other), load the root, verify both message types are available.
   - `test_list_messages`: load a schema with 3 message types, verify list_messages returns all 3 FQNs.
   - `test_schema_roundtrip_mcap`: load schemas, write session with embedded schemas, read session, verify schemas are recovered and messages can be looked up.
   - `test_load_invalid_descriptor`: load garbage bytes as a descriptor set, verify a typed error is returned (not a panic).
   - `test_cli_schemas_load`: run `prb schemas load test.proto --include-path ./protos/`, verify success output.
   - `test_cli_schemas_list`: run `prb schemas list session.mcap`, verify message types are printed.
2. **Regression tests:** All Segment 1 tests and Subsection 1 tests pass.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are in `crates/schema/`, `crates/storage/` (schema integration), `crates/cli/` (schemas subcommand), and `Cargo.toml`. No other crates modified beyond Cargo.toml dependency additions.
