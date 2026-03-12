//! Protobuf decoding library for PRB.
//!
//! This crate provides two decoding strategies:
//! - `wire_format`: Best-effort decoding without schemas (field numbers only)
//! - `schema_backed`: Schema-based decoding with field names and types

pub mod schema_backed;
pub mod wire_format;

pub use wire_format::{
    Fixed32Value, Fixed64Value, LenValue, VarintValue, WireDecodeError, WireField, WireMessage,
    WireValue, decode_wire_format,
};

pub use schema_backed::{DecodeError, DecodedMessage, decode_with_schema};
