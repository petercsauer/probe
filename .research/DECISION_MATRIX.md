# Test Utilities Decision Matrix

## Quick Reference: What to Adopt

| Crate/Pattern | Priority | Use Case | Effort | Impact |
|---------------|----------|----------|--------|--------|
| **prb-test-utils crate** | HIGH | Centralize 680 LOC of duplicated builders | 6-10h | Removes 90% duplication |
| **rstest** | MEDIUM | Parameterized tests, fixtures | 2-3h | Cleaner test organization |
| **test-case** | MEDIUM | Simpler parameterized tests | 2-3h | Alternative to rstest |
| **proptest strategies** | MEDIUM | Property-based testing, edge cases | 4-8h | Better test coverage |
| **fake** | LOW | Realistic test data (IPs, names) | 1-2h | Polish only |
| **derive_builder** | REJECT | Auto-generate builders | N/A | Manual builder is better |
| **typed-builder** | REJECT | Type-safe builders | N/A | Overkill for tests |
| **quickcheck** | REJECT | Property-based testing | N/A | Use proptest instead |
| **arbitrary** | DEFER | Fuzzing support | 8+ h | Future enhancement |

## Recommended Implementation Order

### Phase 1: Eliminate Duplication (High ROI)
**Timeline**: 1-2 days  
**Effort**: 6-10 hours  
**Impact**: Removes 600+ LOC of duplicate code

1. Create `crates/prb-test-utils/` crate
2. Implement core APIs:
   - `event()` - minimal default event
   - `grpc_event()`, `zmq_event()`, `http2_event()` - protocol-specific presets
   - `event_builder()` - returns pre-configured DebugEventBuilder
3. Add to `[dev-dependencies]` in all crates
4. Migrate 42 test files to use centralized builders
5. Document with rustdoc examples

**Success Metrics**:
- 600+ LOC removed from test files
- Single source of truth for test events
- All tests pass after migration

### Phase 2: Improve Test Organization (Medium ROI)
**Timeline**: 1 day  
**Effort**: 2-3 hours  
**Impact**: Cleaner, more maintainable tests

Choose ONE:
- **Option A**: Add `rstest` for fixture injection + parameterized tests
- **Option B**: Add `test-case` for simpler parameterized tests (recommended if you don't need fixtures)

Migrate parameterized tests to use chosen framework.

**Success Metrics**:
- Reduced test boilerplate
- Easier to add new test cases
- Clear test case descriptions

### Phase 3: Expand Test Coverage (Medium-High ROI)
**Timeline**: 2-3 days  
**Effort**: 4-8 hours  
**Impact**: Catch edge cases, improve robustness

1. Add proptest strategies to `prb-test-utils/src/strategies.rs`
2. Implement `arb_debug_event()` generator
3. Add property-based tests for:
   - Serialization/deserialization round-trips
   - Query evaluation invariants
   - Rendering stability (no panics)
   - Export format consistency

**Success Metrics**:
- Property-based tests find edge cases
- 100+ random events tested per property
- Edge case regression tests added

### Phase 4: Polish (Optional, Low ROI)
**Timeline**: 1 day  
**Effort**: 1-2 hours  
**Impact**: Realistic test data

1. Add `fake` crate for realistic IPs, ports, method names
2. Update integration tests to use realistic data

**Success Metrics**:
- Integration tests use realistic network addresses
- Easier to spot anomalies in test output

### Phase 5: Fuzzing (Future Enhancement)
**Timeline**: TBD (separate project)  
**Effort**: 8+ hours  
**Impact**: Security, robustness

1. Add `arbitrary` crate
2. Implement `Arbitrary` for DebugEvent
3. Create fuzz targets for parsers, decoders
4. Integrate with cargo-fuzz or AFL

**Success Metrics**:
- Fuzzing infrastructure in place
- Coverage reports available
- Fuzz testing in CI (optional)

## Cost-Benefit Summary

```
┌─────────────────────────────────────────────────────────────┐
│  IMPACT vs EFFORT                                           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  High Impact    │   prb-test-utils ★                       │
│                 │   (6-10h, removes 600 LOC)                │
│                 │                                            │
│                 │   proptest strategies                      │
│                 │   (4-8h, edge cases)                       │
│  ─────────────────────────────────────────────────          │
│                 │                                            │
│  Medium Impact  │   rstest/test-case                        │
│                 │   (2-3h, cleaner tests)                    │
│  ─────────────────────────────────────────────────          │
│                 │                                            │
│  Low Impact     │   fake (1-2h, polish)                     │
│                 │                                            │
│  ─────────────────────────────────────────────────          │
│                      Low         Medium         High        │
│                             EFFORT                           │
└─────────────────────────────────────────────────────────────┘
```

## Key Insights from Research

1. **The problem is NOT the builder quality** - `DebugEventBuilder` in prb-core is well-designed. The problem is inconsistent use across tests.

2. **Simple solution = big impact** - Creating a 100-line `prb-test-utils` crate eliminates 600+ LOC of duplication. Classic 80/20 rule.

3. **Learn from the ecosystem** - tokio, serde, tracing all have separate test utility crates. This is the idiomatic pattern.

4. **Avoid over-engineering** - Don't replace the manual builder with derive_builder or typed-builder. The manual builder has domain-specific methods that macros can't provide.

5. **Property-based testing is underutilized** - proptest is already in the project but not used extensively. Adding strategies would catch edge cases.

6. **Naming matters** - Current inconsistency (`make_test_event`, `sample_event`, `create_test_event`) makes code harder to grep and understand. Standardize on `event()`, `grpc_event()`, etc.

## Related Patterns in probe Codebase

The project already demonstrates good architectural patterns:
- **Workspace structure**: 24 crates with clear separation of concerns
- **prb-fixture crate**: Provides JSON fixture loading (but NOT test builders)
- **Existing builder**: DebugEventBuilder in prb-core (just needs to be used consistently)
- **proptest dependency**: Already added but strategies not centralized

The test utilities work complements these existing patterns.

