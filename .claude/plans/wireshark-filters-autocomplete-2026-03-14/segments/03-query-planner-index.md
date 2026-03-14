---
segment: 3
title: "Build Query Planner with Index Usage"
depends_on: [1]
risk: 6/10
complexity: High
cycle_budget: 20
status: pending
commit_message: "feat(tui): Add query planner with EventIndex optimization for 100k+ events"
---

# Segment 3: Build Query Planner with Index Usage

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement query planner that uses existing EventIndex (transport, src, dst hashmaps) to avoid full scans for 100k+ events, with caching for parsed filters and compiled regexes.

**Depends on:** Segment 1 (port/IP field resolution must work correctly)

## Context: Issue 4 - EventIndex Exists But Unused

**Core Problem:**
- `prb-tui/src/event_store.rs` has EventIndex with hashmaps but filtering always full-scans
- Current filter: 87µs for 1000-event batch (incremental), 9ms for 100K events (full)
- Performance already exceeds targets (58x faster than 5ms), but can optimize further
- EventIndex structure:
  ```rust
  pub struct EventIndex {
      by_transport: HashMap<TransportKind, Vec<usize>>,  // TCP, UDP, etc.
      by_src: HashMap<String, Vec<usize>>,                // Source address
      by_dst: HashMap<String, Vec<usize>>,                // Dest address
  }
  ```

**Current filtering (event_store.rs):**
```rust
// Always full scan, even for simple filters like "transport == tcp"
pub fn apply_filter(&self, filter: &FilterExpr) -> Vec<usize> {
    self.events.iter()
        .enumerate()
        .filter(|(_, e)| evaluate_expr(e, filter).unwrap_or(false))
        .map(|(i, _)| i)
        .collect()
}
```

**Root Cause:**
No query planner to optimize filter execution. Simple queries like `transport == "tcp"` should use `index.by_transport` instead of scanning all events.

**Proposed Fix:**
Add query planner that:
1. Analyzes FilterExpr AST
2. Identifies indexable predicates
3. Uses indices for filtering when possible
4. Falls back to full scan for complex queries
5. Caches parsed FilterExpr and compiled Regex

```rust
// New file: crates/prb-tui/src/query_planner.rs

use prb_query::FilterExpr;
use regex::Regex;
use std::collections::HashMap;

/// Query execution plan
#[derive(Debug)]
pub enum QueryPlan {
    /// Use transport index
    IndexedByTransport { kind: TransportKind, remaining: Option<Box<FilterExpr>> },

    /// Use src/dst index
    IndexedBySrc { addr: String, remaining: Option<Box<FilterExpr>> },
    IndexedByDst { addr: String, remaining: Option<Box<FilterExpr>> },

    /// Full scan required
    FullScan(FilterExpr),
}

/// Query planner with filter cache
pub struct QueryPlanner {
    /// Cache of parsed filter expressions
    filter_cache: HashMap<String, FilterExpr>,

    /// Cache of compiled regexes
    regex_cache: HashMap<String, Regex>,
}

impl QueryPlanner {
    pub fn new() -> Self {
        Self {
            filter_cache: HashMap::new(),
            regex_cache: HashMap::new(),
        }
    }

    /// Parse filter string and cache result
    pub fn parse_filter(&mut self, filter_str: &str) -> Result<&FilterExpr, String> {
        if !self.filter_cache.contains_key(filter_str) {
            let expr = prb_query::parse_filter(filter_str)?;
            self.filter_cache.insert(filter_str.to_string(), expr);
        }
        Ok(self.filter_cache.get(filter_str).unwrap())
    }

    /// Compile regex and cache result
    pub fn get_regex(&mut self, pattern: &str) -> Result<&Regex, regex::Error> {
        if !self.regex_cache.contains_key(pattern) {
            let regex = Regex::new(pattern)?;
            self.regex_cache.insert(pattern.to_string(), regex);
        }
        Ok(self.regex_cache.get(pattern).unwrap())
    }

    /// Plan query execution
    pub fn plan(&self, expr: &FilterExpr) -> QueryPlan {
        match expr {
            // Simple transport filter → use index
            FilterExpr::Comparison { field, op: CompOp::Eq, value }
                if field == "transport" => {
                    if let FilterValue::String(kind_str) = value {
                        if let Ok(kind) = kind_str.parse::<TransportKind>() {
                            return QueryPlan::IndexedByTransport { kind, remaining: None };
                        }
                    }
                }

            // AND with transport filter → use index + filter remaining
            FilterExpr::And(left, right) => {
                if let FilterExpr::Comparison { field, op: CompOp::Eq, value } = left.as_ref() {
                    if field == "transport" {
                        if let FilterValue::String(kind_str) = value {
                            if let Ok(kind) = kind_str.parse::<TransportKind>() {
                                return QueryPlan::IndexedByTransport {
                                    kind,
                                    remaining: Some(right.clone()),
                                };
                            }
                        }
                    }
                }
                // Try the reverse (right side is transport)
                if let FilterExpr::Comparison { field, op: CompOp::Eq, value } = right.as_ref() {
                    if field == "transport" {
                        if let FilterValue::String(kind_str) = value {
                            if let Ok(kind) = kind_str.parse::<TransportKind>() {
                                return QueryPlan::IndexedByTransport {
                                    kind,
                                    remaining: Some(left.clone()),
                                };
                            }
                        }
                    }
                }
            }

            // TODO: Similar logic for src/dst indices

            _ => {}
        }

        // Default: full scan
        QueryPlan::FullScan(expr.clone())
    }
}

// Update EventStore to use QueryPlanner
impl EventStore {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            index: EventIndex::new(),
            planner: QueryPlanner::new(),
        }
    }

    pub fn apply_filter_with_plan(&mut self, filter_str: &str) -> Result<Vec<usize>, String> {
        // Parse and cache filter
        let expr = self.planner.parse_filter(filter_str)?;

        // Plan execution
        let plan = self.planner.plan(expr);

        // Execute plan
        match plan {
            QueryPlan::IndexedByTransport { kind, remaining } => {
                // Get candidates from index
                let candidates = self.index.by_transport.get(&kind)
                    .map(|v| v.clone())
                    .unwrap_or_default();

                // Apply remaining filter if present
                if let Some(remaining_expr) = remaining {
                    Ok(candidates.into_iter()
                        .filter(|&i| {
                            let event = &self.events[i];
                            evaluate_expr(event, &remaining_expr).unwrap_or(false)
                        })
                        .collect())
                } else {
                    Ok(candidates)
                }
            }

            QueryPlan::IndexedBySrc { addr, remaining } => {
                // Similar to transport case
                let candidates = self.index.by_src.get(&addr)
                    .map(|v| v.clone())
                    .unwrap_or_default();

                if let Some(remaining_expr) = remaining {
                    Ok(candidates.into_iter()
                        .filter(|&i| evaluate_expr(&self.events[i], &remaining_expr).unwrap_or(false))
                        .collect())
                } else {
                    Ok(candidates)
                }
            }

            QueryPlan::FullScan(expr) => {
                // Fallback to full scan
                Ok(self.events.iter()
                    .enumerate()
                    .filter(|(_, e)| evaluate_expr(e, &expr).unwrap_or(false))
                    .map(|(i, _)| i)
                    .collect())
            }

            _ => Ok(vec![])
        }
    }
}
```

**Pre-Mortem Risks:**
1. **Cache invalidation**: Filter cache never evicts (memory grows unbounded) - limit to 100 entries with LRU
2. **Index staleness**: If events are mutated after indexing, index is invalid - but EventStore is append-only
3. **Complex query planning**: Heuristics may choose wrong index for complex ANDs/ORs - add stats tracking to tune
4. **Regex cache memory**: Large regexes accumulate - limit to 50 entries with LRU
5. **Plan correctness**: Bug in planner could filter out valid events - comprehensive tests critical

**Alternatives Ruled Out:**
- **Always use indices**: Some queries (complex ORs) are slower with indices
- **Parse filter on every keystroke**: 9ms for 100K events is already fast enough for debounced updates (100ms)
- **Dedicated index per field**: Would require 60+ indices (one per field), memory overhead too high
- **Skip query planner**: Current performance adequate, but planner enables real-time filtering during typing

## Scope

**Files to create:**
- `crates/prb-tui/src/query_planner.rs` - New query planner module
- `crates/prb-tui/tests/query_planner_test.rs` - Test query planning logic

**Files to modify:**
- `crates/prb-tui/src/event_store.rs` - Add `planner: QueryPlanner` field, update `apply_filter()`
- `crates/prb-tui/src/lib.rs` - Export query_planner module
- `crates/prb-tui/src/filter_state.rs` - Use `apply_filter_with_plan()` instead of `apply_filter()`

**Unchanged files:**
- `crates/prb-query/src/parser.rs` - Query planner consumes AST, doesn't modify parser
- `crates/prb-core/src/event.rs` - No changes to event structure

## Implementation Approach

1. **Create query_planner.rs module**
   - Define `QueryPlan` enum with variants for indexed and full-scan execution
   - Define `QueryPlanner` struct with filter_cache and regex_cache
   - Implement `parse_filter()` with caching
   - Implement `plan()` with AST pattern matching

2. **Implement plan execution**
   - `IndexedByTransport`: Lookup in `by_transport`, apply remaining filter
   - `IndexedBySrc/Dst`: Lookup in `by_src`/`by_dst`, apply remaining filter
   - `FullScan`: Fall back to existing full-scan logic

3. **Add cache eviction**
   - Use LRU strategy: track access order, evict oldest when limit reached
   - Filter cache limit: 100 entries
   - Regex cache limit: 50 entries

4. **Update EventStore**
   - Add `planner: QueryPlanner` field
   - Replace `apply_filter()` calls with `apply_filter_with_plan()`
   - Keep old method for backwards compatibility

5. **Update FilterState**
   - Use new `apply_filter_with_plan()` method
   - Pass filter string (not parsed expr) to enable caching

6. **Write comprehensive tests**
   - Test query planning for each index type
   - Test cache hit/miss behavior
   - Test LRU eviction
   - Benchmark: verify index usage is faster than full scan
   - Integration test: complex filters with indices

## Build and Test Commands

**Build:** `cargo build --package prb-tui`

**Test (targeted):** `cargo test --package prb-tui query_planner`

**Test (regression):** `cargo test --package prb-tui`

**Test (full gate):** `cargo test --workspace --all-targets`

**Benchmark:** `cargo bench --package prb-tui -- filter_performance` (if benchmark exists)

## Exit Criteria

1. **Targeted tests:**
   - `test_plan_simple_transport` - Plan for `transport == "tcp"` uses index
   - `test_plan_and_transport` - Plan for `transport == "tcp" && port == 443` uses index + filter
   - `test_plan_complex_or` - Plan for complex OR uses full scan
   - `test_filter_cache_hit` - Parse same filter twice, cache hit
   - `test_regex_cache_hit` - Compile same regex twice, cache hit
   - `test_lru_eviction_filter` - Cache evicts oldest entry at limit
   - `test_lru_eviction_regex` - Regex cache evicts oldest at limit
   - `test_indexed_faster_than_scan` - Benchmark shows index is faster

2. **Regression tests:** All existing prb-tui tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - LRU eviction implemented for both caches
   - No unbounded memory growth
   - Plan correctness verified with integration tests
   - No dead code or commented-out blocks

6. **Scope verification gate:**
   - Only event_store.rs, filter_state.rs, lib.rs, and new files modified
   - EventIndex structure unchanged
   - Public API maintained (old apply_filter still works)

**Risk Factor:** 6/10 - Query planning involves heuristics that could introduce bugs or performance regressions

**Estimated Complexity:** High - Query planning logic is non-trivial, requires careful testing and benchmarking

**Evidence for Optimality:**
1. **Codebase evidence**: EventIndex already exists and is maintained correctly, just needs to be used
2. **Database literature**: Query planners use indices for equality predicates (standard optimization)
3. **Existing solutions**: SQLite query planner, PostgreSQL planner use similar index selection heuristics
4. **Performance data**: Current 9ms for 100K events is fast, but indexed queries can be 10-100x faster for selective filters
