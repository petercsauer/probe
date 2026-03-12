//! Query and filter language for DebugEvents.
//!
//! This crate provides a simple expression language for filtering DebugEvents
//! based on transport type, metadata fields, and other event properties.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![allow(missing_docs)] // TODO: Complete documentation in future segment

pub mod ast;
pub mod error;
pub mod eval;
pub mod parser;

pub use error::QueryError;

use ast::Expr;
use prb_core::DebugEvent;

#[derive(Debug, Clone)]
pub struct Filter {
    expr: Option<Expr>,
    source: String,
}

impl Filter {
    pub fn parse(input: &str) -> Result<Self, QueryError> {
        let trimmed = input.trim();
        let expr = if trimmed.is_empty() {
            None // Empty input matches everything
        } else {
            Some(parser::parse_expr(trimmed)?)
        };
        Ok(Filter {
            expr,
            source: input.to_string(),
        })
    }

    pub fn matches(&self, event: &DebugEvent) -> bool {
        match &self.expr {
            Some(expr) => eval::eval(expr, event),
            None => true, // Empty filter matches everything
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::{
        DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
    };
    use std::collections::BTreeMap;

    fn make_event(transport: TransportKind, metadata: BTreeMap<String, String>) -> DebugEvent {
        DebugEvent {
            id: EventId::from_raw(1),
            timestamp: Timestamp::from_nanos(1_000_000_000),
            source: EventSource {
                adapter: "test".to_string(),
                origin: "test.json".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:1234".to_string(),
                    dst: "10.0.0.2:5678".to_string(),
                }),
            },
            transport,
            direction: Direction::Inbound,
            payload: Payload::Raw { raw: Bytes::new() },
            metadata,
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        }
    }

    #[test]
    fn filter_matches_transport() {
        let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
        let event = make_event(TransportKind::Grpc, BTreeMap::new());
        assert!(filter.matches(&event));

        let event2 = make_event(TransportKind::Zmq, BTreeMap::new());
        assert!(!filter.matches(&event2));
    }

    #[test]
    fn filter_matches_metadata() {
        let filter = Filter::parse(r#"grpc.method contains "Users""#).unwrap();
        let mut meta = BTreeMap::new();
        meta.insert("grpc.method".to_string(), "/api/Users/Get".to_string());
        let event = make_event(TransportKind::Grpc, meta);
        assert!(filter.matches(&event));
    }

    #[test]
    fn filter_source_preserved() {
        let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
        assert_eq!(filter.source(), r#"transport == "gRPC""#);
    }

    #[test]
    fn empty_filter_matches_everything() {
        let filter = Filter::parse("").unwrap();
        let event1 = make_event(TransportKind::Grpc, BTreeMap::new());
        let event2 = make_event(TransportKind::Zmq, BTreeMap::new());
        assert!(filter.matches(&event1));
        assert!(filter.matches(&event2));

        let filter_whitespace = Filter::parse("   ").unwrap();
        assert!(filter_whitespace.matches(&event1));
    }
}
