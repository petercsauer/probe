# prb-decode

Protobuf decoding library for PRB, providing two complementary strategies for turning raw protobuf bytes into human-readable structures. Wire-format decoding works without any schema for quick best-effort inspection, while schema-backed decoding produces fully named fields and typed values when descriptors are available.

## Key Types

### Wire-Format Decoding (no schema required)

| Type | Description |
|------|-------------|
| `WireMessage` | A decoded message as a list of `WireField`s identified by field number |
| `WireField` | A single field: field number + `WireValue` |
| `WireValue` | Enum of wire types: `Varint`, `Fixed64`, `Fixed32`, `Len` (bytes/nested/string) |
| `VarintValue` | Decoded varint with signed/unsigned/bool interpretations |
| `Fixed32Value` / `Fixed64Value` | Fixed-width values with float/int interpretations |
| `LenValue` | Length-delimited value with UTF-8 string and nested message attempts |
| `WireDecodeError` | Error type for malformed wire data |

### Schema-Backed Decoding

| Type | Description |
|------|-------------|
| `DecodedMessage` | Fully decoded message with field names, types, and nested structures |
| `DecodeError` | Error type for schema mismatches and decoding failures |

## Usage

```rust
use prb_decode::{decode_wire_format, decode_with_schema};

// Best-effort decode without schema
let wire_msg = decode_wire_format(&raw_bytes)?;
for field in &wire_msg.fields {
    println!("field {}: {:?}", field.field_number, field.value);
}

// Schema-backed decode with full field names
let decoded = decode_with_schema(&raw_bytes, &message_descriptor)?;
println!("{}", serde_json::to_string_pretty(&decoded)?);
```

## Relationship to Other Crates

`prb-decode` is a standalone library with no PRB crate dependencies — it operates purely on byte buffers and prost-reflect descriptors. It is used by `prb-grpc` to decode gRPC message payloads and by `prb-cli` for the `--wire-format` inspection flag. Schema descriptors are typically obtained from `prb-schema`'s `SchemaRegistry`.

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

Protobuf decoding library for PRB.

This crate provides two decoding strategies:
- `wire_format`: Best-effort decoding without schemas (field numbers only)
- `schema_backed`: Schema-based decoding with field names and types

<!-- cargo-rdme end -->
