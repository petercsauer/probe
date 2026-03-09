---
id: "S2-5"
title: "Runtime .proto Compilation Path Under-Emphasized"
risk: 2/10
addressed_by_segments: [2]
---

# Issue S2-5: Runtime .proto Compilation Path Under-Emphasized

**Core Problem:**
The parent plan's Subsection 2 says "descriptor set loading, message lookup" for the schema subsystem. This implies users must pre-compile .proto files into .desc files using `protoc --descriptor_set_out`. However, `protox` (already listed as a dependency) enables runtime .proto compilation without protoc. This is a major UX improvement that deserves first-class support.

**Root Cause:**
The plan lists protox as a dependency but describes the schema workflow in terms of pre-compiled descriptor sets only.

**Proposed Fix:**
Support two schema loading paths with equal prominence:

1. **Pre-compiled descriptors:** `prb schemas load service.desc` -- loads a binary FileDescriptorSet produced by protoc or protox CLI.
2. **Raw .proto files:** `prb schemas load service.proto --include-path ./protos/` -- compiles .proto files at runtime using protox's Compiler API, resolving imports from the specified include paths. No protoc installation required.

The protox Compiler API:

```rust
let fds = protox::Compiler::new(include_paths)?
    .open_files(proto_files)?
    .file_descriptor_set();
```

The SchemaRegistry accepts both paths and normalizes them into a DescriptorPool:

```rust
impl SchemaRegistry {
    pub fn load_descriptor_set(&mut self, bytes: &[u8]) -> Result<()>;
    pub fn load_proto_files(
        &mut self,
        files: &[&Path],
        includes: &[&Path],
    ) -> Result<()>;
}
```

**Existing Solutions Evaluated:**

- `protox` (v0.9.1, crates.io, same author as prost-reflect, 3.7M downloads/90d, actively maintained) -- pure Rust protobuf compiler. Handles imports, dependencies, and well-known types. Produces FileDescriptorSet compatible with prost-reflect's DescriptorPool. Adopted.
- `protobuf-parse` (v3.7.2, crates.io, 5.9M downloads/90d) -- parses .proto files but belongs to the rust-protobuf ecosystem, not prost. Rejected: ecosystem mismatch.

**Alternatives Considered:**

- Require protoc installation for .proto compilation. Rejected: adds a heavyweight external dependency (protoc binary, system package), hurts cross-platform UX, and is unnecessary given protox.
- Support only .desc files. Rejected: forces users through a two-step workflow (compile then load) when a one-step workflow (load .proto directly) is possible.

**Pre-Mortem -- What Could Go Wrong:**

- protox may not support all protobuf language features (e.g., editions, custom options). Mitigation: document supported proto2/proto3 features and fall back to .desc loading for unsupported features.
- Import resolution may fail if include paths are not correctly specified. Mitigation: clear error messages naming the missing import and the searched paths.
- protox compilation adds latency (~100ms for typical schemas). Mitigation: cache compiled DescriptorPools; schemas rarely change during a debug session.

**Risk Factor:** 2/10

**Evidence for Optimality:**

- Existing solutions: protox is pure Rust, actively maintained by the prost-reflect author, and produces DescriptorPool directly -- zero impedance mismatch.
- External evidence: The trend in Rust protobuf tooling is toward removing protoc as a dependency (prost-build supports protox as a backend, tonic-build supports protox).

**Blast Radius:**

- Direct changes: SchemaRegistry (new load_proto_files method), CLI (prb schemas load accepts .proto files)
- Potential ripple: documentation, user-facing error messages
