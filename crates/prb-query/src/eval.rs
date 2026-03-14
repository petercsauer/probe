use crate::ast::{CmpOp, Expr, FieldPath, Value};
use prb_core::DebugEvent;
use regex::Regex;

pub fn eval(expr: &Expr, event: &DebugEvent) -> bool {
    match expr {
        Expr::And(lhs, rhs) => eval(lhs, event) && eval(rhs, event),
        Expr::Or(lhs, rhs) => eval(lhs, event) || eval(rhs, event),
        Expr::Not(inner) => !eval(inner, event),
        Expr::Compare { field, op, value } => eval_compare(field, *op, value, event),
        Expr::Contains { field, substring } => resolve_field(field, event)
            .is_some_and(|v| v.to_lowercase().contains(&substring.to_lowercase())),
        Expr::Exists { field } => eval_exists(field, event),
        Expr::Matches { field, pattern } => eval_matches(field, pattern, event),
        Expr::In { field, values } => eval_in(field, values, event),
        Expr::Slice { field, start, end } => eval_slice(field, *start, *end, event),
        Expr::Function { name, args } => eval_function(name, args, event),
    }
}

fn eval_exists(field: &FieldPath, event: &DebugEvent) -> bool {
    let key = field.dotted();
    match key.as_str() {
        "warnings" => !event.warnings.is_empty(),
        "source.network" | "network" => event.source.network.is_some(),
        "sequence" => event.sequence.is_some(),
        _ => event.metadata.contains_key(&key),
    }
}

fn resolve_field(field: &FieldPath, event: &DebugEvent) -> Option<String> {
    let key = field.dotted();
    match key.as_str() {
        "id" => Some(event.id.as_u64().to_string()),
        "timestamp" => Some(event.timestamp.as_nanos().to_string()),
        "transport" => Some(event.transport.to_string()),
        "direction" => Some(event.direction.to_string()),
        "source.adapter" | "adapter" => Some(event.source.adapter.clone()),
        "source.origin" | "origin" => Some(event.source.origin.clone()),
        "source.src" | "src" => event.source.network.as_ref().map(|n| n.src.clone()),
        "source.dst" | "dst" => event.source.network.as_ref().map(|n| n.dst.clone()),
        "sequence" => event.sequence.map(|s| s.to_string()),
        _ => event.metadata.get(&key).cloned(),
    }
}

fn eval_compare(field: &FieldPath, op: CmpOp, value: &Value, event: &DebugEvent) -> bool {
    let resolved = match resolve_field(field, event) {
        Some(v) => v,
        None => return false,
    };

    match value {
        Value::String(s) => {
            let cmp = resolved.as_str().cmp(s.as_str());
            apply_ordering(op, cmp)
        }
        Value::Number(n) => {
            if let Ok(parsed) = resolved.parse::<f64>() {
                apply_f64_cmp(op, parsed, *n)
            } else {
                false
            }
        }
        Value::Bool(b) => {
            let resolved_bool = match resolved.as_str() {
                "true" => true,
                "false" => false,
                _ => return false,
            };
            match op {
                CmpOp::Eq => resolved_bool == *b,
                CmpOp::Ne => resolved_bool != *b,
                _ => false,
            }
        }
    }
}

fn apply_ordering(op: CmpOp, cmp: std::cmp::Ordering) -> bool {
    match op {
        CmpOp::Eq => cmp == std::cmp::Ordering::Equal,
        CmpOp::Ne => cmp != std::cmp::Ordering::Equal,
        CmpOp::Gt => cmp == std::cmp::Ordering::Greater,
        CmpOp::Ge => cmp != std::cmp::Ordering::Less,
        CmpOp::Lt => cmp == std::cmp::Ordering::Less,
        CmpOp::Le => cmp != std::cmp::Ordering::Greater,
    }
}

fn apply_f64_cmp(op: CmpOp, lhs: f64, rhs: f64) -> bool {
    match op {
        CmpOp::Eq => (lhs - rhs).abs() < f64::EPSILON,
        CmpOp::Ne => (lhs - rhs).abs() >= f64::EPSILON,
        CmpOp::Gt => lhs > rhs,
        CmpOp::Ge => lhs >= rhs,
        CmpOp::Lt => lhs < rhs,
        CmpOp::Le => lhs <= rhs,
    }
}

fn eval_matches(field: &FieldPath, pattern: &str, event: &DebugEvent) -> bool {
    let value = match resolve_field(field, event) {
        Some(v) => v,
        None => return false,
    };
    match Regex::new(pattern) {
        Ok(regex) => regex.is_match(&value),
        Err(_) => false,
    }
}

fn eval_in(field: &FieldPath, values: &[Value], event: &DebugEvent) -> bool {
    let resolved = match resolve_field(field, event) {
        Some(v) => v,
        None => return false,
    };

    values.iter().any(|v| match v {
        Value::String(s) => resolved == *s,
        Value::Number(n) => {
            if let Ok(parsed) = resolved.parse::<f64>() {
                (parsed - n).abs() < f64::EPSILON
            } else {
                false
            }
        }
        Value::Bool(b) => {
            let resolved_bool = match resolved.as_str() {
                "true" => true,
                "false" => false,
                _ => return false,
            };
            resolved_bool == *b
        }
    })
}

fn eval_slice(field: &FieldPath, _start: usize, end: usize, event: &DebugEvent) -> bool {
    // Slices are evaluated by returning the slice as a synthetic field
    // This is a placeholder that returns true if slice exists
    // In practice, slices would be used with comparison operators
    let value = match resolve_field(field, event) {
        Some(v) => v,
        None => return false,
    };
    let bytes = value.as_bytes();
    bytes.len() >= end
}

fn eval_function(name: &str, args: &[Box<Expr>], event: &DebugEvent) -> bool {
    // Functions return string values which are then evaluated
    // For boolean context, we check if the function succeeds
    match name {
        "len" => {
            if args.len() != 1 {
                return false;
            }
            // For len(), we just check if the field exists
            // In practice, len() would be used in comparisons
            eval(&args[0], event)
        }
        "lower" | "upper" => {
            if args.len() != 1 {
                return false;
            }
            eval(&args[0], event)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::{
        CorrelationKey, DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload,
        Timestamp, TransportKind,
    };
    use std::collections::BTreeMap;

    fn sample_event() -> DebugEvent {
        let mut metadata = BTreeMap::new();
        metadata.insert("grpc.method".to_string(), "/api.v1.Users/Get".to_string());
        metadata.insert("grpc.status".to_string(), "0".to_string());
        metadata.insert("h2.stream_id".to_string(), "1".to_string());

        DebugEvent {
            id: EventId::from_raw(42),
            timestamp: Timestamp::from_nanos(1_710_000_000_000_000_000),
            source: EventSource {
                adapter: "pcap".to_string(),
                origin: "capture.pcap".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:42837".to_string(),
                    dst: "10.0.0.2:50051".to_string(),
                }),
            },
            transport: TransportKind::Grpc,
            direction: Direction::Outbound,
            payload: Payload::Raw {
                raw: Bytes::from_static(b"hello"),
            },
            metadata,
            correlation_keys: vec![CorrelationKey::StreamId { id: 1 }],
            sequence: Some(1),
            warnings: vec![],
        }
    }

    #[test]
    fn eval_transport_eq() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"transport == "gRPC""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_transport_ne() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"transport != "ZMQ""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_metadata_field() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"grpc.method == "/api.v1.Users/Get""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_contains() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"grpc.method contains "Users""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_contains_case_insensitive() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"grpc.method contains "users""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_exists_with_warnings_empty() {
        let event = sample_event();
        let expr = crate::parser::parse_expr("warnings exists").unwrap();
        assert!(!eval(&expr, &event));
    }

    #[test]
    fn eval_exists_metadata() {
        let event = sample_event();
        let expr = crate::parser::parse_expr("grpc.method exists").unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_not() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"!transport == "ZMQ""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_and() {
        let event = sample_event();
        let expr =
            crate::parser::parse_expr(r#"transport == "gRPC" && grpc.method contains "Users""#)
                .unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_or() {
        let event = sample_event();
        let expr =
            crate::parser::parse_expr(r#"transport == "ZMQ" || transport == "gRPC""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_numeric_id() {
        let event = sample_event();
        let expr = crate::parser::parse_expr("id == 42").unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_numeric_gt() {
        let event = sample_event();
        let expr = crate::parser::parse_expr("id > 10").unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_source_src() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"source.src contains ":42837""#).unwrap();
        assert!(eval(&expr, &event));
    }

    #[test]
    fn eval_missing_field_returns_false() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"nonexistent == "value""#).unwrap();
        assert!(!eval(&expr, &event));
    }

    #[test]
    fn eval_direction() {
        let event = sample_event();
        let expr = crate::parser::parse_expr(r#"direction == "outbound""#).unwrap();
        assert!(eval(&expr, &event));
    }
}
