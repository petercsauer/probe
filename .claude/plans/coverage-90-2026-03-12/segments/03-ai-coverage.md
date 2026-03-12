---
segment: 03
title: prb-ai to 85%
depends_on: []
risk: 2
complexity: Medium
cycle_budget: 6
estimated_lines: ~120 test lines
---

# Segment 03: prb-ai Coverage to 85%

## Context

**Current:** 76.61%
**Target:** 85%
**Gap:** +8.39 percentage points

**Critical gap:**
- `src/explain.rs` - **36.73% (115 lines uncovered)** - async LLM calls, error handling

**Strong modules:**
- `src/prompt.rs` - 99.43% ✅
- `src/config.rs` - 89.53% ✅
- `src/context.rs` - 84.45%

## Goal

Test async explain functions, API error paths, streaming responses.

## Exit Criteria

1. [ ] prb-ai ≥85%
2. [ ] explain.rs ≥70% (realistic given async/network code)
3. [ ] All 22 existing tests pass
4. [ ] Mock LLM responses tested

## Implementation Plan

### Priority 1: Explain Function Mocking (~80 lines)

```rust
// crates/prb-ai/src/explain.rs

#[cfg(test)]
mod tests {
    #[test]
    fn test_explain_event_validates_empty_events() {
        // Already exists, verify passes
    }

    #[tokio::test]
    async fn test_explain_event_api_error() {
        // Mock API failure response
        let config = AiConfig::default();
        let events = create_test_events(5);
        // Would need mock HTTP client or skip in unit tests
    }

    #[test]
    fn test_explain_stream_chunk_assembly() {
        // Test streaming response assembly
    }
}
```

### Priority 2: Context Builder Edge Cases (~40 lines)

```rust
// crates/prb-ai/src/context.rs - add more tests

#[test]
fn test_context_with_very_large_window() {
    let events = create_test_events(1000);
    let ctx = ExplainContext::build(&events, 500, 100);
    assert!(ctx.before.len() <= 100);
    assert!(ctx.after.len() <= 100);
}

#[test]
fn test_context_at_boundaries() {
    // Test when target_idx=0 or target_idx=events.len()-1
}
```

## Test Plan

1. `cargo llvm-cov -p prb-ai --summary-only`
2. Add mock-based tests for explain.rs
3. Focus on testable logic, accept lower coverage for network I/O
4. Verify: `cargo test -p prb-ai`

## Success Metrics

- prb-ai: 76.61% → 85%+
- explain.rs: 36.73% → 70%+ (realistic for async/network code)
- ~15-20 new tests
