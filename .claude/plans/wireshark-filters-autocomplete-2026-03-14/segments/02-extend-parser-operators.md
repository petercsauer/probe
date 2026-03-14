---
segment: 2
title: "Extend Parser with Protocol Operators"
depends_on: []
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(query): Add matches, in, slice, and function operators to filter parser"
---

# Segment 2: Extend Parser with Protocol Operators

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Extend nom-based filter parser to support Wireshark operators: `matches` (regex), `in` (set membership), `[n:m]` (slices), and functions (`len()`, `lower()`, `upper()`).

**Depends on:** None

## Context: Issue 2 - Incomplete Filter Syntax

**Core Problem:**
- `prb-query/src/parser.rs` (552 lines) supports basic operators but missing Wireshark features
- Current: `==`, `!=`, `<`, `>`, `contains`, `exists`, `&&`, `||`, `!`, parentheses
- Missing: `matches` (regex), `in` (set membership), `[n:m]` (byte slices), functions

**Current parser capabilities:**
```rust
// parser.rs - existing
tcp.port == 443
frame.len > 1000
src contains "192.168"
transport == "tcp" && dst == "10.0.0.1"
!(udp.port == 53)
```

**Missing Wireshark operators:**
```
tcp.payload matches "^GET"          // Regex match
tcp.port in {80, 443, 8080}         // Set membership
tcp.payload[0:4] == "\x47\x45\x54"  // Byte slice
len(tcp.payload) > 100              // Function call
lower(http.host) == "example.com"   // String function
```

**Root Cause:**
Parser was built for basic filtering only. Wireshark uses these operators for:
- `matches`: Protocol detection without full parsing (HTTP verb in payload)
- `in`: Port lists, IP ranges, common patterns
- `[n:m]`: Magic number detection, header inspection
- Functions: String normalization, length checks

**Proposed Fix:**
Extend parser.rs AST and nom parsers:

```rust
// Add to FilterExpr enum in parser.rs
pub enum FilterExpr {
    // Existing variants...
    Comparison { field: String, op: CompOp, value: FilterValue },
    Contains { field: String, value: String },
    Exists { field: String },
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),

    // NEW variants
    Matches { field: String, pattern: String },  // Regex
    In { field: String, values: Vec<FilterValue> },  // Set membership
    Slice { field: String, start: usize, end: usize },  // Byte slice
    Function { name: String, args: Vec<FilterExpr> },  // Function call
}

// Add to CompOp enum
pub enum CompOp {
    Eq, Ne, Lt, Gt, Le, Ge,
    // Already supported, no changes needed
}

// New parser functions
fn parse_matches(input: &str) -> IResult<&str, FilterExpr> {
    let (input, field) = parse_field(input)?;
    let (input, _) = tag("matches")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, pattern) = parse_string_literal(input)?;
    Ok((input, FilterExpr::Matches { field, pattern }))
}

fn parse_in(input: &str) -> IResult<&str, FilterExpr> {
    let (input, field) = parse_field(input)?;
    let (input, _) = tag("in")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('{')(input)?;
    let (input, values) = separated_list1(
        delimited(multispace0, char(','), multispace0),
        parse_value
    )(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, FilterExpr::In { field, values }))
}

fn parse_slice(input: &str) -> IResult<&str, FilterExpr> {
    let (input, field) = parse_field(input)?;
    let (input, _) = char('[')(input)?;
    let (input, start) = nom::character::complete::u32(input)?;
    let (input, _) = char(':')(input)?;
    let (input, end) = nom::character::complete::u32(input)?;
    let (input, _) = char(']')(input)?;
    Ok((input, FilterExpr::Slice { field, start: start as usize, end: end as usize }))
}

fn parse_function(input: &str) -> IResult<&str, FilterExpr> {
    let (input, name) = alphanumeric1(input)?;
    let (input, _) = char('(')(input)?;
    let (input, args) = separated_list0(
        delimited(multispace0, char(','), multispace0),
        parse_expr
    )(input)?;
    let (input, _) = char(')')(input)?;
    Ok((input, FilterExpr::Function { name: name.to_string(), args }))
}
```

**Evaluation in eval.rs:**
```rust
// Add to evaluate_expr() in eval.rs
FilterExpr::Matches { field, pattern } => {
    let value = resolve_field(event, field)?;
    let regex = regex::Regex::new(pattern).ok()?;
    Some(regex.is_match(&value))
}

FilterExpr::In { field, values } => {
    let value = resolve_field(event, field)?;
    Some(values.iter().any(|v| match_value(&value, v)))
}

FilterExpr::Slice { field, start, end } => {
    // Return slice as hex string for comparison
    let value = resolve_field(event, field)?;
    let bytes = value.as_bytes();
    if bytes.len() < end {
        return Some(false);
    }
    Some(format!("{:?}", &bytes[start..end]))
}

FilterExpr::Function { name, args } => {
    match name.as_str() {
        "len" => {
            let value = evaluate_expr(event, &args[0])?;
            Some(value.len().to_string())
        }
        "lower" => {
            let value = evaluate_expr(event, &args[0])?;
            Some(value.to_lowercase())
        }
        "upper" => {
            let value = evaluate_expr(event, &args[0])?;
            Some(value.to_uppercase())
        }
        _ => None
    }
}
```

**Pre-Mortem Risks:**
1. **Regex performance**: Compiling regex on every event evaluation (mitigated by caching in S3 query planner)
2. **Slice out of bounds**: Need to check field length before slicing
3. **Function recursion**: Nested functions could stack overflow (limit depth to 3)
4. **Memory**: Large `in` sets (100+ values) could impact performance (acceptable for user convenience)

**Alternatives Ruled Out:**
- **Custom regex engine**: std `regex` crate is battle-tested and fast
- **Precompiled regex cache in parser**: Parser should be stateless, cache belongs in evaluator
- **Skip slice operator**: Critical for protocol detection (magic numbers)
- **Skip functions**: String normalization essential for case-insensitive matching

## Scope

**Files to modify:**
- `crates/prb-query/src/parser.rs` - Add AST variants and nom parsers
- `crates/prb-query/src/eval.rs` - Add evaluation logic for new operators
- `crates/prb-query/Cargo.toml` - Add `regex = "1.10"` dependency

**Files to create:**
- `crates/prb-query/tests/parser_operators_test.rs` - Test new operator parsing
- `crates/prb-query/tests/eval_operators_test.rs` - Test new operator evaluation

**Unchanged files:**
- `crates/prb-tui/src/filter_state.rs` - No changes to TUI layer
- `crates/prb-query/src/lib.rs` - Public API unchanged

## Implementation Approach

1. **Add regex dependency**
   - Add `regex = "1.10"` to prb-query/Cargo.toml
   - Import `use regex::Regex` in eval.rs

2. **Extend AST in parser.rs**
   - Add `Matches`, `In`, `Slice`, `Function` variants to `FilterExpr`
   - Add `pattern: String` field to Matches
   - Add `values: Vec<FilterValue>` field to In

3. **Implement nom parsers**
   - `parse_matches()` - field, "matches", string literal
   - `parse_in()` - field, "in", `{val1, val2, val3}`
   - `parse_slice()` - field, `[start:end]`
   - `parse_function()` - name, `(arg1, arg2)`
   - Update `parse_primary()` to try new parsers before fallback

4. **Implement evaluation in eval.rs**
   - `Matches`: Compile regex and test
   - `In`: Iterate values and match
   - `Slice`: Extract byte range, compare as hex
   - `Function`: Match on function name, apply transformation

5. **Write comprehensive tests**
   - Parser tests: verify AST structure for each operator
   - Eval tests: verify behavior with sample events
   - Integration tests: complex filters with multiple operators
   - Error cases: invalid regex, out-of-bounds slices, unknown functions

## Build and Test Commands

**Build:** `cargo build --package prb-query`

**Test (targeted):** `cargo test --package prb-query parser_operators && cargo test --package prb-query eval_operators`

**Test (regression):** `cargo test --package prb-query`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:**
   - `test_parse_matches` - Parse `field matches "pattern"`
   - `test_parse_in` - Parse `field in {1, 2, 3}`
   - `test_parse_slice` - Parse `field[0:4]`
   - `test_parse_function` - Parse `len(field)`, `lower(field)`
   - `test_eval_matches` - Regex matching works
   - `test_eval_in` - Set membership works
   - `test_eval_slice` - Byte slice extraction works
   - `test_eval_function_len` - len() function works
   - `test_eval_function_lower` - lower() function works
   - `test_complex_filter` - `tcp.port in {80,443} && tcp.payload matches "^GET"`

2. **Regression tests:** All existing prb-query tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - No dead code or commented-out blocks
   - Regex compilation errors handled gracefully
   - Slice bounds checked before access
   - Function recursion depth limited

6. **Scope verification gate:**
   - Only parser.rs, eval.rs, Cargo.toml, and new test files modified
   - No changes to public API in lib.rs
   - regex dependency added with exact version

**Risk Factor:** 4/10 - Parser extension is well-isolated, but regex/slice evaluation needs careful error handling

**Estimated Complexity:** Medium - nom parser patterns are straightforward, evaluation logic requires careful bounds checking

**Evidence for Optimality:**
1. **Wireshark semantics**: matches, in, slices, and functions are standard Wireshark operators (verified in display filter docs)
2. **Codebase evidence**: Existing nom parsers follow same pattern, extension is natural fit
3. **Existing solutions**: `regex` crate is standard for Rust regex (used by ripgrep, skim, etc.)
4. **External best practices**: nom parser combinators for recursive descent (nom documentation, Rust parsing patterns)
