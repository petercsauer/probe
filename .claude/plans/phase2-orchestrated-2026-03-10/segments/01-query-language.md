---
segment: 1
title: "Query Language Engine (prb-query)"
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-query): add nom-based filter language engine"
---

# Subsection 1: Query Language Engine (`prb-query`)

## Purpose

A domain-specific filter language for protocol-aware filtering of `DebugEvent`s.
Compiles expressions to predicate closures. Used by both the TUI (live filter bar)
and CLI (`prb inspect --where "..."`).

## Filter Language Specification

### Grammar (PEG-style)

```
expr       = or_expr
or_expr    = and_expr ("||" and_expr)*
and_expr   = not_expr ("&&" not_expr)*
not_expr   = "!" atom | atom
atom       = comparison | contains_expr | exists_expr | "(" expr ")"
comparison = field_path op value
contains_expr = field_path "contains" string_lit
exists_expr   = field_path "exists"
field_path = ident ("." ident)*
op         = "==" | "!=" | ">" | ">=" | "<" | "<="
value      = string_lit | number_lit | bool_lit
string_lit = '"' [^"]* '"'
number_lit = [0-9]+ ("." [0-9]+)?
bool_lit   = "true" | "false"
ident      = [a-zA-Z_][a-zA-Z0-9_]*
```

### Built-in Fields

| Field | Type | Source |
|-------|------|--------|
| `id` | u64 | `event.id` |
| `timestamp` | string (ISO8601) | `event.timestamp` |
| `transport` | string | `event.transport` display name |
| `direction` | string | `event.direction` |
| `source.adapter` | string | `event.source.adapter` |
| `source.src` | string | `event.source.network.src` |
| `source.dst` | string | `event.source.network.dst` |
| `warnings` | bool (any) | `!event.warnings.is_empty()` |
| `grpc.method` | string | metadata key |
| `grpc.status` | string | metadata key |
| `zmq.topic` | string | metadata key |
| `dds.domain_id` | string | metadata key |
| `dds.topic_name` | string | metadata key |
| `*` (any key) | string | metadata lookup |

Dotted field paths first check built-in fields, then fall through to
`event.metadata` lookup (joining dots back: `grpc.method` → key `"grpc.method"`).

### Examples

```
transport == "gRPC" && grpc.method contains "Users"
direction == "inbound" && warnings exists
dds.domain_id == "0" && dds.topic_name contains "chatter"
source.src contains ":50051"
```

---

## Segment S1.1: Lexer + AST Types

**Files**: `crates/prb-query/src/lib.rs`, `crates/prb-query/src/ast.rs`,
`crates/prb-query/src/lexer.rs`

**AST types**:
```rust
pub enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare { field: FieldPath, op: CmpOp, value: Value },
    Contains { field: FieldPath, substring: String },
    Exists { field: FieldPath },
}

pub struct FieldPath(pub Vec<String>);

pub enum CmpOp { Eq, Ne, Gt, Ge, Lt, Le }

pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
}
```

---

## Segment S1.2: Parser + Evaluator

**Files**: `crates/prb-query/src/parser.rs`, `crates/prb-query/src/eval.rs`

**Parser**: nom combinators parsing the grammar above into `Expr` AST.

**Evaluator**: `eval(expr: &Expr, event: &DebugEvent) -> bool`
- Resolves `FieldPath` against event fields and metadata
- Performs type-aware comparison (string, number, bool)
- Short-circuits on And/Or

---

## Segment S1.3: prb-core Integration

**Files**: `crates/prb-query/src/lib.rs` (public API)

**Public API**:
```rust
pub fn parse_filter(input: &str) -> Result<Filter, QueryError>;

pub struct Filter { expr: Expr }
impl Filter {
    pub fn matches(&self, event: &DebugEvent) -> bool;
}
```

---

## Tests

- Parse + eval for every operator (==, !=, >, <, contains, exists)
- Boolean composition (&&, ||, !)
- Parenthesized grouping
- Field path resolution (built-in fields, metadata fallback)
- Error messages for malformed input
- Empty input → matches everything
