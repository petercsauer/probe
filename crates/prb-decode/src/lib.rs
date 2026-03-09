//! Protobuf decoding library for PRB.
//!
//! This crate provides two decoding strategies:
//! - `wire_format`: Best-effort decoding without schemas (field numbers only)
//! - `schema_backed`: Schema-based decoding with field names and types

pub mod wire_format;
pub mod schema_backed;

pub use wire_format::{
    decode_wire_format, Fixed32Value, Fixed64Value, LenValue, VarintValue, WireDecodeError,
    WireField, WireMessage, WireValue,
};

pub use schema_backed::{decode_with_schema, DecodeError, DecodedMessage};
