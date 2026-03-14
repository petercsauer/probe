# prb-schema

Protobuf schema registry for PRB, providing loading, storage, and resolution of protobuf message type descriptors. It supports both pre-compiled FileDescriptorSet files (`.desc`) and runtime compilation of `.proto` source files via the `protox` compiler, making it easy to work with schemas from any protobuf workflow.

## Key Types

| Type | Description |
|------|-------------|
| `SchemaRegistry` | Central registry that loads, stores, and resolves protobuf message descriptors |
| `SchemaError` | Error type for file I/O, compilation, and resolution failures |

The `SchemaRegistry` implements `prb-core`'s `SchemaResolver` trait, allowing it to be passed to any component that needs to look up message types by fully-qualified name.

## Usage

```rust
use prb_schema::SchemaRegistry;

let mut registry = SchemaRegistry::new();

// Load from a compiled descriptor set
registry.load_descriptor_set("protos/service.desc")?;

// Or compile .proto files at runtime
registry.load_proto_file("protos/api.proto", &["protos/"])?;

// Resolve a message type by name
if let Some(descriptor) = registry.resolve("mypackage.MyMessage") {
    println!("Found message with {} fields", descriptor.fields().len());
}
```

## Relationship to Other Crates

`prb-schema` depends on `prb-core` for the `SchemaResolver` trait. It is used by `prb-storage` to embed schemas into MCAP session files, and by `prb-cli` for the `schemas` subcommand family (`load`, `list`, `export`). Schema descriptors from this registry feed into `prb-decode` for schema-backed protobuf decoding with full field names and types.

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

Protobuf schema registry for PRB.

This crate provides schema loading, storage, and resolution for protobuf message types.
It supports both pre-compiled descriptor sets (.desc files) and runtime compilation of
.proto files via protox.

<!-- cargo-rdme end -->
