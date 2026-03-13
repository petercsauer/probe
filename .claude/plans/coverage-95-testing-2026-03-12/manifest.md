---
plan: "Coverage 95% Testing Plan"
goal: "Increase Rust codebase test coverage from ~82% to 95%"
generated: 2026-03-12
status: Complete
parent_plan: null
rules_version: 2026-03-12
---

# Coverage 95% Testing Plan -- Manifest

## Execution Order (Fail-Fast)

1. Segment 1 (prerequisite) → Segment 2 (Risk 9/10)
2. Segment 5 (prerequisite) → Segment 6 (Risk 8/10)
3. Segment 4 (Risk 6/10) - Can run parallel with others
4. Segment 3 (Risk 6/10) - After Segment 2
5. Segment 7 (Risk 5/10) - Independent
6. Segment 8 (Risk 4/10) - Independent
7. Segment 9 (Risk 4/10) - Independent

## Dependency Diagram

```
Wave 1:     [1] ────┐
                    ├──→ [2] ──→ [3]
            [5] ────┼──→ [6]
                    │
            [4] ────┤
            [7] ────┤
            [8] ────┤
            [9] ────┘

Dependencies:
- Segment 2 depends on Segment 1
- Segment 3 depends on Segment 2
- Segment 6 depends on Segment 5
- Segments 4, 7, 8, 9 are independent
```

## Segment Index

| # | Title | File | Depends On | Risk | Complexity | Cycles | Status | Commit |
|---|-------|------|------------|------|------------|--------|--------|--------|
| 1 | TLS Keylog Parser + Fuzzing Infrastructure | segments/01-tls-keylog-fuzzing.md | None | 8/10 | Medium | 15 | merged | 8c0c74b |
| 2 | TLS Decryption RFC and Wycheproof Vectors | segments/02-tls-decryption-vectors.md | 1 | 9/10 | High | 20 | merged | 1801e4d |
| 3 | TLS Cipher Suite Coverage | segments/03-tls-cipher-suites.md | 2 | 6/10 | Low | 10 | merged | 550d6dc |
| 4 | Protobuf Testing Suite | segments/04-protobuf-testing.md | None | 6/10 | High | 18 | merged | 5a7cff0 |
| 5 | Packet Normalization Memory Safety | segments/05-packet-normalization.md | None | 7/10 | Medium | 15 | merged | b1064f6 |
| 6 | Pipeline Core Robustness | segments/06-pipeline-robustness.md | 5 | 8/10 | Medium | 12 | merged | f00497e |
| 7 | AI HTTP Mocking | segments/07-ai-http-mocking.md | None | 5/10 | Low | 10 | merged | b25d2d7 |
| 8 | TUI Snapshot Expansion | segments/08-tui-snapshots.md | None | 4/10 | Low | 12 | merged | 59a681a |
| 9 | TUI Interactive Testing | segments/09-tui-interactive.md | None | 4/10 | Medium | 15 | merged | 7ae76e9 |

**Total estimated effort:** 122 cycles (~12-15 hours)
**Actual effort:** ~70 cycles (~40% under budget)
**Execution time:** 66 minutes (2026-03-13 05:19 - 06:25)

## Parallelization

- **Wave 1:** Segments 1, 4, 5, 7, 8, 9 can run concurrently (independent)
- **Wave 2:** Segment 2 (after 1), Segment 6 (after 5) can run concurrently
- **Wave 3:** Segment 3 (after 2)

Maximum concurrency: 4 parallel builders per wave

## Preamble Injection

Before launching any builder subagent, the orchestration agent assembles the prompt:

1. Read `.claude/commands/iterative-builder.md`
2. Read `.claude/commands/devcontainer-exec.md`
3. Read the segment file from `segments/{NN}-{slug}.md`

Assembled prompt = [iterative-builder.md] + [devcontainer-exec.md] + [segment file contents]

## Execution Instructions

Use the external `orchestrate` tool to execute this plan:

```bash
orchestrate run .claude/plans/coverage-95-testing-2026-03-12
```

The orchestrator will:
1. Launch iterative-builder subagents for each segment in dependency order
2. Execute waves in parallel where dependencies allow (max 4 concurrent)
3. Track progress and gate each segment on its exit criteria
4. Squash WIP commits into final segment commits
5. Update execution log

After all segments complete, run `/deep-verify` to validate that coverage reached 95% and all exit criteria were met.

## Plan Status

**Status:** Complete (all 9 segments merged)
**Executed:** 2026-03-13 05:19 - 06:25 (66 minutes wall-clock time)
**Coverage achieved:** 92.2% (target was 95%)
**Packages improved:**
- prb-pcap: 90.13%
- prb-ai: 97.05%
- prb-core: 91.24%
- prb-decode: 92.20%

**Gap analysis:** Fell 2.8% short of 95% target. Remaining gaps likely in:
- prb-pcap: TLS edge cases, link-layer variants
- prb-core: Error propagation paths, concurrency edge cases

**Recommendation:** Consider follow-up plan to close final 2.8% gap if needed for production readiness.
