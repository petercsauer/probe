---
plan: "Coverage 90+ Hardening"
goal: "Push line coverage from 84% to 90%+ across all 19 workspace crates via trait-seam refactoring, mock infrastructure, and targeted test additions."
generated: 2026-03-10
status: Ready for execution
parent_plan: ""
rules_version: 2026-03-10
---

# Coverage 90+ Hardening — Manifest

## Overview

10 of 19 crates are below 90% line coverage. The gaps fall into three categories: (1) code tightly coupled to OS resources (pcap, terminal, FFI) that needs trait seams for dependency injection; (2) code requiring external services (LLM API) that needs mock servers; (3) pure logic with untested branches that just needs more test cases. Segments are ordered confidence-first: easy unit tests land first to build momentum, trait refactoring and mock infrastructure follow, plugin fixtures last.

All work is Rust test code. No production behavior changes except introducing trait-based DI seams in `prb-capture` (S4) and extracting helpers in `prb-cli` (S6).

## Dependency Diagram

```
S1 (unit test gaps)  ──────────────┐
S2 (grpc trace+H2)  ──────────────┤
S3 (tui app+loader)  ─────────────┤  all independent
S5 (ai wiremock)     ──────────────┤
S7 (plugin-native fixture) ────────┤
S8 (plugin-wasm fixture)  ─────────┘
                                    
S4 (capture trait seams)  ──┐
                            └── S6 (cli extract+test)
```

Wave 1: S1 ∥ S2 ∥ S3 ∥ S5 (low risk, independent)
Wave 2: S4 ∥ S7 ∥ S8 (medium-high risk, independent)
Wave 3: S6 (depends on S4 trait seams)

## Segment Index

| # | Title | File | Depends On | Risk | Complexity | Status |
|---|-------|------|------------|------|------------|--------|
| 1 | Unit test gap sweep | segments/01-unit-test-gaps.md | None | 2/10 | Low | pending |
| 2 | gRPC trace context + H2 edges | segments/02-grpc-trace-h2.md | None | 3/10 | Low | pending |
| 3 | TUI app key handlers + loader | segments/03-tui-app-loader.md | None | 3/10 | Medium | pending |
| 4 | Capture trait seam refactoring | segments/04-capture-trait-seams.md | None | 6/10 | High | pending |
| 5 | AI explain wiremock tests | segments/05-ai-wiremock.md | None | 4/10 | Medium | pending |
| 6 | CLI command extraction + tests | segments/06-cli-extract-test.md | 4 | 5/10 | Medium | pending |
| 7 | Plugin-native test fixture | segments/07-plugin-native-fixture.md | None | 6/10 | High | pending |
| 8 | Plugin-wasm test fixture | segments/08-plugin-wasm-fixture.md | None | 6/10 | High | pending |

## Parallelization

- **Wave 1 (S1 ∥ S2 ∥ S3 ∥ S5):** All independent, low risk. Max 4 parallel.
- **Wave 2 (S4 ∥ S7 ∥ S8):** All independent. S4 introduces trait seams in prb-capture. S7/S8 build plugin test fixtures.
- **Wave 3 (S6):** Depends on S4's `PacketSource` trait for testing `prb-cli/commands/capture.rs`.

## Preamble Injection

Before launching any builder subagent, the orchestration agent assembles the prompt:
1. Read `.cursor/rules/iterative-builder-prompt.mdc`
2. Read the segment file from `segments/{NN}-{slug}.md`

## Current Coverage Baseline

| Crate | Before | Target |
|-------|--------|--------|
| prb-capture | 47.6% | 80%+ |
| prb-plugin-native | 55.6% | 85%+ |
| prb-cli | 62.9% | 85%+ |
| prb-plugin-wasm | 66.1% | 85%+ |
| prb-grpc | 72.3% | 90%+ |
| prb-ai | 76.1% | 90%+ |
| prb-tui | 85.1% | 92%+ |
| prb-decode | 86.1% | 90%+ |
| prb-pcap | 87.5% | 90%+ |
| prb-dds | 89.8% | 92%+ |

## Execution Log

| Segment | Est. Complexity | Risk | Cycles Used | Status | Notes |
|---------|----------------|------|-------------|--------|-------|
| 1: unit test gaps | Low | 2/10 | -- | -- | -- |
| 2: grpc trace+H2 | Low | 3/10 | -- | -- | -- |
| 3: tui app+loader | Medium | 3/10 | -- | -- | -- |
| 4: capture trait seams | High | 6/10 | -- | -- | -- |
| 5: ai wiremock | Medium | 4/10 | -- | -- | -- |
| 6: cli extract+test | Medium | 5/10 | -- | -- | -- |
| 7: plugin-native fixture | High | 6/10 | -- | -- | -- |
| 8: plugin-wasm fixture | High | 6/10 | -- | -- | -- |

**Deep-verify result:** --
**Follow-up plans:** --
