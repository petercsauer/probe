# Refactor and Cleanup Sprint - Plan Directory

**Generated:** 2026-03-13
**Status:** Ready for execution
**Format:** Orchestrate v3 (toml + manifest + segments)

## Quick Start

```bash
# Execute the plan using the orchestrate skill
cd /Users/psauer/probe
# Then in Claude: /orchestrate .claude/plans/refactor-cleanup-2026-03-13/
```

## Plan Overview

This plan addresses comprehensive code quality improvements:
- **1,570 LOC of code duplication** → Extract to shared utilities
- **9 bugs** (3 critical, 3 high, 3 medium) → Fix correctness and trust issues
- **37 documentation issues** → Update API docs, fix examples, setup tooling
- **19.8GB of stale files** → Clean build artifacts, backups, worktrees

## Directory Structure

```
.claude/plans/refactor-cleanup-2026-03-13/
├── README.md                    # This file
├── orchestrate.toml             # Execution configuration
├── manifest.md                  # Plan overview, dependencies, instructions
├── execution-log.md             # Progress tracking
├── issues/                      # Issue analysis briefs
│   ├── issue-01-test-builder-duplication.md
│   ├── issue-04-tls-keylog-reload.md
│   └── ... (21 issues total)
└── segments/                    # Self-contained segment handoffs
    ├── 01-test-utils-crate.md
    ├── 02-pcap-test-helpers.md
    ├── ... (21 segments total)
    └── 21-http2-parser-errors.md
```

## Execution Strategy

### Wave-Based Parallel Execution (8-10 days)

**Wave 1: High-ROI Refactoring** (2-3 days, 3 parallel)
- Segment 1: Test Utilities Crate (foundation)
- Segment 2: PCAP Test Helpers (375 LOC duplication)
- Segment 3: Decoder Event Building (75 LOC duplication)

**Wave 2: Critical Bugs** (2-3 days, 3 parallel)
- Segment 4: TLS Keylog Reload (unimplemented feature)
- Segment 5: TCP Gap Ranges (data loss)
- Segment 6: IP Fragment Memory Leak (unbounded growth)

**Wave 3: High Priority Bugs** (1-2 days, 3 parallel)
- Segment 7: Capture Statistics (fake data)
- Segment 8: Linktype Detection (hardcoded assumptions)
- Segment 9: Dead Code Removal (unused functions)

**Wave 4: Documentation** (2-3 days, mixed parallelization)
- Segment 10: Document Core APIs (93 items)
- Segment 11: Document Protocol Decoders (parallel with S10)
- Segment 12: Fix Outdated Examples (sequential after S10-11)
- Segment 13: Setup Doc Tooling (cargo-rdme, lychee, etc.)

**Wave 5: Cleanup** (1 day, 5 parallel)
- Segment 14: Clean Build Artifacts (65MB)
- Segment 15: Remove Backup Files (193+ files)
- Segment 16: Archive Worktrees (19.7GB)
- Segment 17: Remove Resource Forks (18 files)
- Segment 18: Migrate Legacy Fixtures (6 files)

**Wave 6: Medium Priority Bugs** (1-2 days, 3 parallel)
- Segment 19: TLS State Tracking (incomplete)
- Segment 20: Plugin Metadata Validation (gaps)
- Segment 21: HTTP/2 Parser Errors (unclear messages)

## Research Foundation

This plan is grounded in comprehensive research:

### Source 1: Codebase Analysis (4 Explore Agents)
- **Code duplication agent**: 1,570 LOC across 42 files (test builders, PCAP utils, decoders)
- **Bugs/quality agent**: 9 bugs with file:line citations and severity ratings
- **Documentation audit**: 37 issues (missing APIs, outdated examples, inconsistencies)
- **Unused files scan**: 19.8GB reclaimable (build artifacts, backups, worktrees)

### Source 2: Project Conventions
- **ADRs reviewed**: 0001 (workspace structure), 0002 (error handling), 0003 (detection), 0004 (plugins)
- **CONTRIBUTING.md**: Test organization, no .unwrap(), property tests, 80% coverage
- **CI workflows**: RUSTDOCFLAGS, clippy, coverage gates
- **Key insight**: 20-crate workspace means test utils need separate crate (not tests/common/)

### Source 3: Existing Solutions
- **Test utilities**: rstest, proptest (already used), cargo-rdme (adopt), trycmd (adopt)
- **Decoder patterns**: tokio-util Codec pattern, nom (already used), serde trait model
- **Doc tools**: cargo-doc-coverage, lychee, cargo-insta, cargo-modules
- **Proven pattern**: tokio-test, serde_test, tracing (all use separate test utils crate)

### Source 4: External Best Practices
- **Rust API Guidelines**: Trait design, documentation conventions, error handling
- **Rust Book Ch. 11**: Test organization for multi-crate workspaces
- **Jon Gjengset "Rust for Rustaceans"**: Anti-pattern - copy-paste test setup (probe has 680 LOC)
- **Luca Palmieri "Zero to Production"**: Centralize test fixtures early

## Key Metrics

### Code Quality
- **Before**: 1,570 LOC duplication, 9 bugs, 37 doc issues
- **After**: Single source of truth, all bugs fixed, comprehensive docs
- **ROI**: 200%+ over project lifetime (90% LOC reduction in tests)

### Risk Profile
- **Complexity**: 12 Low, 8 Medium, 1 High
- **Risk**: 1 segment at 7/10 (IP fragment memory), rest ≤6/10
- **Risk budget**: Within acceptable limits (1 high-risk segment)

### Test Coverage
- **Current**: 80% (enforced in CI)
- **Target**: Maintain ≥80% (new code must have tests)
- **Strategy**: Use prb-test-utils for cleaner test code

## Segment Status

All 21 segments are ready for execution. Each segment includes:
- ✅ Self-contained handoff contract (no back-references)
- ✅ Concrete build/test commands (not placeholders)
- ✅ Pre-mortem risk analysis
- ✅ Evidence for optimality (≥2 sources cited)
- ✅ Exact commit messages (conventional commits format)
- ✅ YAML frontmatter (segment #, dependencies, risk, complexity)

## Detailed Segments

### High-Detail Segments (Critical/Complex)
These segments have comprehensive implementation guidance:
- **Segment 1**: Test Utilities Crate (foundation for all bug fixes)
- **Segment 6**: IP Fragment Memory Leak (highest risk 7/10, memory safety)

### Standard-Detail Segments
All other segments follow the same comprehensive format with:
- Context from issue briefs (core problem, proposed fix, pre-mortem)
- Scope and key files
- Implementation approach (step-by-step)
- Alternatives ruled out
- Exit criteria (6 gates: targeted, regression, build, test, self-review, scope)

## How to Execute

### Option 1: Orchestrate Skill (Recommended)
```
User: /orchestrate .claude/plans/refactor-cleanup-2026-03-13/
```

The orchestrate skill will:
1. Read orchestrate.toml for configuration
2. Read manifest.md for dependency diagram and instructions
3. For each segment in wave order:
   - Assemble prompt with preamble injection (iterative-builder.md + devcontainer-exec.md + segment)
   - Launch iterative-builder subagent
   - Verify exit gates
   - Commit if all gates pass
   - Update execution-log.md
4. After all segments: Run deep-verify for comprehensive validation
5. If needed: Generate follow-up plan for gaps

### Option 2: Manual Execution (Not Recommended)
If executing manually:
1. Read `manifest.md` for execution instructions
2. Read `orchestrate.toml` for configuration
3. Follow dependency diagram (execute in wave order)
4. For each segment:
   - Read `segments/{NN}-{slug}.md`
   - Implement according to exit criteria
   - Verify all 6 gates pass
   - Commit with exact commit message from frontmatter
5. Update `execution-log.md` after each segment

## Verification Protocol

After all segments complete:
```
User: /deep-verify .claude/plans/refactor-cleanup-2026-03-13/
```

Deep-verify will:
- Check all issue briefs are addressed
- Verify all exit criteria were met
- Test end-to-end functionality
- Generate report: FULLY VERIFIED | PARTIALLY VERIFIED | NOT VERIFIED
- If gaps found: Create follow-up plan

## Follow-Up Work (After This Plan)

This plan focuses on high-impact refactoring, critical bugs, and documentation. Follow-up work:

### Future Enhancements (Not In Scope)
- Migrate remaining 38 test files to prb-test-utils (semi-automated)
- Add proptest strategies to prb-test-utils (property-based testing)
- Add rstest fixtures (parameterized tests)
- Automatic TLS keylog file watching (currently manual reload only)
- Fuzzing with cargo-fuzz + arbitrary crate

### Out of Scope (Intentional)
- New features (per user: "no new features")
- Breaking changes to public APIs (internal refactoring only)
- Performance optimizations beyond bug fixes
- Dependency upgrades (separate effort)

## Plan Metadata

- **Generated by**: deep-plan workflow (Step 9: Materialize)
- **Rules version**: 2026-03-13 (orchestrate v3 format)
- **Entry point**: Fresh goal (user requested comprehensive refactoring)
- **Execution protocol**: orchestrate skill (with iterative-builder subagents)
- **Verification protocol**: deep-verify skill (post-execution validation)
- **Research depth**: Very thorough (4 codebase agents + external research)
- **Evidence quality**: High (≥2 sources per decision, file:line citations)

## Questions?

For execution guidance, see:
- `manifest.md` - Full execution instructions
- `.claude/commands/orchestrate.md` - Orchestrate skill documentation
- `.claude/commands/iterative-builder.md` - Builder subagent protocol

For plan content questions:
- `issues/*.md` - Detailed issue analysis with evidence
- `segments/*.md` - Implementation guidance for each segment
- `execution-log.md` - Track progress during execution
