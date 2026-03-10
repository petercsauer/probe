# prb-query

Event filter query language for PRB. This crate provides a nom-based parser, an AST representation, and an evaluator that matches filter expressions against `DebugEvent`s. Filters support field comparisons, substring matching, existence checks, and boolean logic — enabling users to narrow the event list in the TUI or batch exports.

## Key types

| Type | Description |
|------|-------------|
| `Filter` | Top-level entry point — parses a query string and evaluates it against events |
| `Expr` | AST node: `Compare`, `Contains`, `Exists`, `And`, `Or`, `Not` |
| `FieldPath` | Dot-separated field reference (e.g. `grpc.method`) |
| `CmpOp` | Comparison operator: `==`, `!=`, `>`, `>=`, `<`, `<=` |
| `Value` | Literal value: `String`, `Number`, or `Bool` |
| `QueryError` | Parse and evaluation errors |

### Supported syntax

```text
transport == "gRPC"                    # field comparison
grpc.method contains "Users"           # substring match
metadata.trace_id exists               # existence check
transport == "gRPC" && direction == "inbound"   # AND
src.port > 1024 || dst.port > 1024     # OR
!(transport == "gRPC")                 # NOT
(a == "1" || b == "2") && c == "3"     # parentheses
```

## Usage

```rust
use prb_query::Filter;
use prb_core::DebugEvent;

let filter = Filter::parse(r#"transport == "gRPC" && grpc.method contains "Users""#)?;

for event in &events {
    if filter.matches(event) {
        println!("matched: {:?}", event.id);
    }
}
```

## Relationship to other crates

- **prb-core** — provides `DebugEvent`, the evaluation target for filter expressions
- **prb-tui** — uses `Filter` to drive the interactive filter bar

See the [PRB documentation](../../docs/) for the full user guide.
