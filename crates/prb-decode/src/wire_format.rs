//! Wire-format protobuf decoder with heuristic disambiguation.

#![allow(missing_docs)]

use std::fmt;
use thiserror::Error;

const MAX_VARINT_BYTES: usize = 10;
const MAX_RECURSION_DEPTH: usize = 64;
const MIN_PRINTABLE_RATIO: f64 = 0.8;

#[derive(Error, Debug)]
pub enum WireDecodeError {
    #[error("Unexpected end of input at byte {0}")]
    UnexpectedEof(usize),
    #[error("Invalid varint encoding at byte {0}")]
    InvalidVarint(usize),
    #[error("Invalid wire type {0} at byte {1}")]
    InvalidWireType(u8, usize),
    #[error("Malformed tag at byte {0}")]
    MalformedTag(usize),
    #[error("Maximum recursion depth ({}) exceeded", MAX_RECURSION_DEPTH)]
    RecursionLimitExceeded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireMessage {
    pub fields: Vec<WireField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireField {
    pub field_number: u32,
    pub wire_type: u8,
    pub value: WireValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WireValue {
    Varint(VarintValue),
    Fixed64(Fixed64Value),
    Fixed32(Fixed32Value),
    LengthDelimited(LenValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarintValue {
    pub unsigned: u64,
    pub signed_zigzag: i64,
    pub as_bool: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fixed64Value {
    pub bytes: [u8; 8],
    pub as_u64: u64,
    pub as_i64: i64,
    pub as_f64: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fixed32Value {
    pub bytes: [u8; 4],
    pub as_u32: u32,
    pub as_i32: i32,
    pub as_f32: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LenValue {
    SubMessage(Box<WireMessage>),
    String(String),
    Bytes(Vec<u8>),
}

impl WireMessage {
    #[must_use] 
    pub const fn new() -> Self {
        Self { fields: Vec::new() }
    }
}

impl Default for WireMessage {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode protobuf wire format without a schema.
///
/// # Errors
/// Returns a `WireDecodeError` if the bytes are malformed or recursion limit is exceeded.
pub fn decode_wire_format(bytes: &[u8]) -> Result<WireMessage, WireDecodeError> {
    decode_with_depth(bytes, 0)
}

#[allow(clippy::cast_possible_truncation)]
fn decode_with_depth(bytes: &[u8], depth: usize) -> Result<WireMessage, WireDecodeError> {
    if depth >= MAX_RECURSION_DEPTH {
        return Err(WireDecodeError::RecursionLimitExceeded);
    }

    let mut message = WireMessage::new();
    let mut pos = 0;

    while pos < bytes.len() {
        let (tag, bytes_read) = read_varint(bytes, pos)?;
        pos += bytes_read;

        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x7) as u8;

        if field_number == 0 {
            return Err(WireDecodeError::MalformedTag(pos - bytes_read));
        }

        let value = match wire_type {
            0 => {
                let (raw_value, bytes_read) = read_varint(bytes, pos)?;
                pos += bytes_read;
                WireValue::Varint(decode_varint(raw_value))
            }
            1 => {
                if pos + 8 > bytes.len() {
                    return Err(WireDecodeError::UnexpectedEof(pos));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&bytes[pos..pos + 8]);
                pos += 8;
                WireValue::Fixed64(decode_fixed64(buf))
            }
            2 => {
                let (len, bytes_read) = read_varint(bytes, pos)?;
                pos += bytes_read;

                let len = len as usize;
                if pos + len > bytes.len() {
                    return Err(WireDecodeError::UnexpectedEof(pos));
                }

                let data = &bytes[pos..pos + len];
                pos += len;
                WireValue::LengthDelimited(decode_length_delimited(data, depth)?)
            }
            5 => {
                if pos + 4 > bytes.len() {
                    return Err(WireDecodeError::UnexpectedEof(pos));
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&bytes[pos..pos + 4]);
                pos += 4;
                WireValue::Fixed32(decode_fixed32(buf))
            }
            _ => {
                return Err(WireDecodeError::InvalidWireType(
                    wire_type,
                    pos - bytes_read,
                ));
            }
        };

        message.fields.push(WireField {
            field_number,
            wire_type,
            value,
        });
    }

    Ok(message)
}

fn read_varint(bytes: &[u8], start: usize) -> Result<(u64, usize), WireDecodeError> {
    let mut result = 0u64;
    let mut shift = 0;
    let mut pos = start;

    for _ in 0..MAX_VARINT_BYTES {
        if pos >= bytes.len() {
            return Err(WireDecodeError::UnexpectedEof(pos));
        }

        let byte = bytes[pos];
        pos += 1;

        result |= u64::from(byte & 0x7F) << shift;

        if byte & 0x80 == 0 {
            return Ok((result, pos - start));
        }

        shift += 7;
    }

    Err(WireDecodeError::InvalidVarint(start))
}

const fn decode_varint(value: u64) -> VarintValue {
    let signed_zigzag = zigzag_decode(value);
    let as_bool = if value == 0 {
        Some(false)
    } else if value == 1 {
        Some(true)
    } else {
        None
    };

    VarintValue {
        unsigned: value,
        signed_zigzag,
        as_bool,
    }
}

#[allow(clippy::cast_possible_wrap)]
const fn zigzag_decode(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

#[allow(clippy::similar_names)]
fn decode_fixed64(bytes: [u8; 8]) -> Fixed64Value {
    let as_u64 = u64::from_le_bytes(bytes);
    let as_i64 = i64::from_le_bytes(bytes);
    let f64_val = f64::from_le_bytes(bytes);

    let as_f64 = if f64_val.is_normal() || f64_val == 0.0 {
        Some(f64_val)
    } else {
        None
    };

    Fixed64Value {
        bytes,
        as_u64,
        as_i64,
        as_f64,
    }
}

#[allow(clippy::similar_names)]
fn decode_fixed32(bytes: [u8; 4]) -> Fixed32Value {
    let as_u32 = u32::from_le_bytes(bytes);
    let as_i32 = i32::from_le_bytes(bytes);
    let f32_val = f32::from_le_bytes(bytes);

    let as_f32 = if f32_val.is_normal() || f32_val == 0.0 {
        Some(f32_val)
    } else {
        None
    };

    Fixed32Value {
        bytes,
        as_u32,
        as_i32,
        as_f32,
    }
}

fn decode_length_delimited(data: &[u8], depth: usize) -> Result<LenValue, WireDecodeError> {
    match decode_with_depth(data, depth + 1) {
        Ok(submsg) => {
            if !submsg.fields.is_empty() {
                return Ok(LenValue::SubMessage(Box::new(submsg)));
            }
        }
        Err(WireDecodeError::RecursionLimitExceeded) => {
            return Err(WireDecodeError::RecursionLimitExceeded);
        }
        Err(_) => {}
    }

    if let Ok(s) = std::str::from_utf8(data)
        && is_mostly_printable(s)
    {
        return Ok(LenValue::String(s.to_string()));
    }

    Ok(LenValue::Bytes(data.to_vec()))
}

#[allow(clippy::cast_precision_loss)]
fn is_mostly_printable(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }

    let printable_count = s.chars().filter(|c| is_printable(*c)).count();
    let total_count = s.chars().count();

    (printable_count as f64) / (total_count as f64) >= MIN_PRINTABLE_RATIO
}

const fn is_printable(c: char) -> bool {
    c.is_ascii_graphic() || c.is_ascii_whitespace()
}

impl fmt::Display for WireMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "WIRE FORMAT DECODE (best-effort, no schema)")?;
        format_message(f, self, 0)
    }
}

fn format_message(f: &mut fmt::Formatter<'_>, msg: &WireMessage, indent: usize) -> fmt::Result {
    for field in &msg.fields {
        write!(
            f,
            "{:indent$}field {}: ",
            "",
            field.field_number,
            indent = indent
        )?;
        format_value(f, &field.value, indent)?;
        writeln!(f)?;
    }
    Ok(())
}

#[allow(clippy::branches_sharing_code)]
fn format_value(f: &mut fmt::Formatter<'_>, value: &WireValue, indent: usize) -> fmt::Result {
    match value {
        WireValue::Varint(v) => {
            write!(f, "{}", v.unsigned)?;
            write!(f, " (varint")?;
            if let Some(b) = v.as_bool {
                write!(f, "; bool={b}")?;
            } else {
                write!(f, "; bool=N/A")?;
            }
            write!(f, "; sint={})", v.signed_zigzag)?;
        }
        WireValue::Fixed64(v) => {
            write!(f, "{}", v.as_u64)?;
            write!(f, " (fixed64; u64={}; i64={}", v.as_u64, v.as_i64)?;
            if let Some(fval) = v.as_f64 {
                write!(f, "; f64={fval}")?;
            }
            write!(f, ")")?;
        }
        WireValue::Fixed32(v) => {
            write!(f, "{}", v.as_u32)?;
            write!(f, " (fixed32; u32={}; i32={}", v.as_u32, v.as_i32)?;
            if let Some(fval) = v.as_f32 {
                write!(f, "; f32={fval}")?;
            }
            write!(f, ")")?;
        }
        WireValue::LengthDelimited(len_val) => match len_val {
            LenValue::SubMessage(msg) => {
                writeln!(f, "{{")?;
                format_message(f, msg, indent + 2)?;
                write!(
                    f,
                    "{:indent$}}} (submessage; {} field{})",
                    "",
                    msg.fields.len(),
                    if msg.fields.len() == 1 { "" } else { "s" },
                    indent = indent
                )?;
            }
            LenValue::String(s) => {
                write!(f, "\"{}\" (string; {} bytes)", s, s.len())?;
            }
            LenValue::Bytes(b) => {
                if b.len() <= 16 {
                    write!(f, "0x")?;
                    for byte in b {
                        write!(f, "{byte:02x}")?;
                    }
                } else {
                    write!(f, "0x")?;
                    for byte in &b[..16] {
                        write!(f, "{byte:02x}")?;
                    }
                    write!(f, "...")?;
                }
                write!(f, " (bytes; {} bytes)", b.len())?;
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wire_varint() {
        let bytes = vec![0x08, 0x96, 0x01];
        let msg = decode_wire_format(&bytes).unwrap();
        assert_eq!(msg.fields.len(), 1);
        match &msg.fields[0].value {
            WireValue::Varint(v) => assert_eq!(v.unsigned, 150),
            _ => panic!(),
        }
    }

    #[test]
    fn test_wire_string() {
        let bytes = vec![0x12, 0x05, b'h', b'e', b'l', b'l', b'o'];
        let msg = decode_wire_format(&bytes).unwrap();
        match &msg.fields[0].value {
            WireValue::LengthDelimited(LenValue::String(s)) => assert_eq!(s, "hello"),
            _ => panic!(),
        }
    }

    #[test]
    fn test_wire_nested_message() {
        let bytes = vec![0x1a, 0x02, 0x08, 0x2a];
        let msg = decode_wire_format(&bytes).unwrap();
        match &msg.fields[0].value {
            WireValue::LengthDelimited(LenValue::SubMessage(_)) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn test_wire_bytes_fallback() {
        let bytes = vec![0x1a, 0x04, 0xff, 0xfe, 0xfd, 0xfc];
        let msg = decode_wire_format(&bytes).unwrap();
        match &msg.fields[0].value {
            WireValue::LengthDelimited(LenValue::Bytes(_)) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn test_wire_fixed32_float() {
        let value = 42u32;
        let bytes_val = value.to_le_bytes();
        let mut bytes = vec![0x25];
        bytes.extend_from_slice(&bytes_val);
        let msg = decode_wire_format(&bytes).unwrap();
        match &msg.fields[0].value {
            WireValue::Fixed32(v) => assert_eq!(v.as_u32, 42),
            _ => panic!(),
        }
    }

    #[test]
    fn test_wire_fixed64_double() {
        let value = 100u64;
        let bytes_val = value.to_le_bytes();
        let mut bytes = vec![0x29];
        bytes.extend_from_slice(&bytes_val);
        let msg = decode_wire_format(&bytes).unwrap();
        match &msg.fields[0].value {
            WireValue::Fixed64(v) => assert_eq!(v.as_u64, 100),
            _ => panic!(),
        }
    }

    #[test]
    fn test_wire_recursion_limit() {
        let mut bytes = vec![0x08, 0x01];
        for _ in 0..100 {
            let len = bytes.len();
            let mut wrapper = vec![0x0a];
            if len < 128 {
                wrapper.push(len as u8);
            } else {
                let mut remaining = len;
                loop {
                    let mut byte = (remaining & 0x7F) as u8;
                    remaining >>= 7;
                    if remaining > 0 {
                        byte |= 0x80;
                    }
                    wrapper.push(byte);
                    if remaining == 0 {
                        break;
                    }
                }
            }
            wrapper.extend_from_slice(&bytes);
            bytes = wrapper;
        }
        let result = decode_wire_format(&bytes);
        assert!(matches!(
            result,
            Err(WireDecodeError::RecursionLimitExceeded)
        ));
    }

    #[test]
    fn test_wire_malformed_varint() {
        let bytes = vec![
            0x08, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
        ];
        let result = decode_wire_format(&bytes);
        assert!(matches!(result, Err(WireDecodeError::InvalidVarint(_))));
    }

    #[test]
    fn test_wire_empty_input() {
        let bytes = vec![];
        let msg = decode_wire_format(&bytes).unwrap();
        assert_eq!(msg.fields.len(), 0);
    }

    #[test]
    fn test_wire_zigzag() {
        let bytes = vec![0x30, 0x53];
        let msg = decode_wire_format(&bytes).unwrap();
        match &msg.fields[0].value {
            WireValue::Varint(v) => assert_eq!(v.signed_zigzag, -42),
            _ => panic!(),
        }
    }
}
