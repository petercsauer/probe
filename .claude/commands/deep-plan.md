Enter deep-plan mode. This skill is the authoritative deep-plan workflow:

## Step 1: Understand the Big Picture

Determine the entry point:

**Entry Point A (Fresh Goal):** Restate the goal, ask clarifying questions about scope/priorities/constraints/non-goals, identify major functional areas, determine delivery order preference.

**Entry Point B (Enrich Existing Plan):** Ingest the existing plan, parse each fix and its rationale, confirm understanding, ask whether the issue list is complete and whether any issues have ordering constraints.

Both converge at Step 2.

## Step 2: Map the Landscape

Use parallel explore subagents to build a high-level map:
1. Architecture scan — modules, boundaries, communication patterns, existing abstractions
2. Existing test coverage — what tests exist, where gaps are
3. Dependency graph — which parts depend on others
4. Risk inventory — fragile, poorly understood, tightly coupled, or performance-sensitive areas

Present findings as a structured summary. Wait for user acknowledgement.

## Step 3: Deep Issue Research

For each issue, conduct four-source research using parallel explore subagents where issues are independent:

- **Source 1 (Codebase):** Read implementations, callers, tests, related modules. Find prior art in repo.
- **Source 2 (Project Conventions):** Check `.claude/commands/` (project skills), `.cursor/plans/` (Cursor-generated reference plans — read-only), README, docs, build system for patterns and conventions.
- **Source 3 (Existing Solutions):** Search package registries, GitHub, awesome lists. For each candidate: maintenance status, scope coverage, license, transitive deps, stack fit. Mark N/A only for purely internal refactoring (with one-sentence justification).
- **Source 4 (External Best Practices):** Web search for official docs, high-vote SO answers, GitHub issues/PRs, engineering blogs, RFCs.

Present research summaries per issue. Wait for acknowledgement.

## Step 4: Issue Analysis Briefs

For each issue, produce a structured brief:

```
### Issue N: [Short title]

**Core Problem:** [2-3 sentences. What is broken/missing/suboptimal. Cite specific files/lines.]
**Root Cause:** [1-2 sentences. Why — not symptoms, causes.]
**Proposed Fix:** [Technical description: what code changes, new abstractions, existing code modified vs replaced, pseudocode if non-obvious]
**Existing Solutions Evaluated:** [Libraries/tools found in Source 3. For each: name, repo/package link, maintenance status, adopt/adapt/reject rationale. If nothing suitable, state what was searched.]
**Alternatives Considered:** [1-2 rejected approaches with short rationale. Prevents subagent from re-evaluating dead ends.]
**Pre-Mortem — What Could Go Wrong:** [Coupling risks, behavioral regressions, performance implications, easy-to-miss edge cases, build/dependency complications]
**Risk Factor:** N/10 [1-3: isolated; 4-6: shared interfaces; 7-8: cross-cutting; 9-10: architectural]
**Evidence for Optimality:** [Cite ≥2 sources: codebase evidence, project conventions, existing solutions, external evidence. One sentence per source.]
**Blast Radius:** Direct changes: [...] / Potential ripple: [...]
```

Present all briefs. Wait for acknowledgement and corrections.

## Step 5: Slice into Segments

Group issues into vertical slices (interface + logic + tests together). Each segment must be:
- **Independent:** Can be built, tested, committed without waiting for future segments
- **Testable:** Clear exit criteria verifiable with automated tests
- **Small:** Completable in one iterative-builder session (20-cycle budget max)
- **Valuable:** Leaves codebase better — no partial scaffolding
- **Ordered:** Numbered; earlier segments establish foundations

### Segment Brief Format (Iterative-Builder Handoff Contract)

```
## Segment N: [Short title]
> **Execution method:** Launch as an `iterative-builder` subagent. The orchestration agent reads and prepends `.claude/commands/iterative-builder.md` and `.claude/commands/devcontainer-exec.md` at launch time per the `/orchestrate` skill.

**Goal:** [One sentence]
**Depends on:** [Prior segment numbers, or "None"]
**Issues addressed:** [Issue N, Issue M]
**Cycle budget:** [10 Low / 15 Medium / 20 High]

**Scope:** [Subsystem / module / area]

**Key files and context:** [Specific files, functions, contracts, invariants, coupling risks — pasted directly, never "see Step 2"]

**Implementation approach:** [Concrete enough to follow, not so prescriptive it can't adapt]

**Alternatives ruled out:** [Dead ends already evaluated]

**Pre-mortem risks:** [Watch-fors for defensive tests]

**Segment-specific commands:**
- Build: [exact command]
- Test (targeted): [exact command]
- Test (regression): [exact command]
- Test (full gate): [exact command]

**Exit criteria:**
1. Targeted tests: [test name: what it validates]
2. Regression tests: [existing targets that must pass]
3. Full build gate: [exact command]
4. Full test gate: [exact command]
5. Self-review gate: No dead code, no commented-out blocks, no TODO hacks, no out-of-scope changes.
6. Scope verification gate: Changed files match stated scope; out-of-scope supporting changes documented.

**Risk factor:** N/10
**Estimated complexity:** Low | Medium | High
**Commit message:** `type(scope): short description`
```

## Step 6: Build Exit Criteria

Explore the build system to find correct test targets, build commands, regression targets, and commands with correct flags, paths, and environment setup (devcontainer exec wrappers if needed).

Tiered gate model:
- Gate 1: Targeted tests pass (proves fix works)
- Gate 2: Regression tests pass (no regressions)
- Gate 3: Full build clean (no compilation/linking breaks)
- Gate 4: Full test suite passes
- Gate 5: Self-review (no dead code, no scope creep, no hacks)

## Step 7: Determine Execution Order

Present the dependency DAG and ask user to choose:
- **Fail-fast:** Highest-risk first
- **Confidence-first:** Lowest-risk first
- **Dependency-order:** Topological
- **User-specified:** Explicit ordering

Note which segments are independent and can run as parallel iterative-builder subagents (up to 4 concurrent).

## Step 8: Validate the Decomposition

Check before presenting the final plan:
1. Dependency chain is acyclic (DAG, no circular deps)
2. No orphan work (every piece of original goal covered)
3. No oversized segments (3+ subsystems or "High" → consider splitting)
4. Integration points are explicit (interfaces in exit criteria of defining segment, consumption in consuming segment)
5. First segment is a walking skeleton when possible
6. Risk budget: flag if 2+ segments at risk 8+
7. Handoff completeness: each brief is self-contained (subagent could begin work reading only the brief)
8. Exit criteria are concrete with actual commands, not placeholders

## Step 9: Present and Materialize the Plan

Present:
1. Overview paragraph (approach, delivery order, ordering strategy)
2. Dependency diagram (text or mermaid)
3. Issue Analysis Briefs (all, in order)
4. Segment briefs (all, in execution order)
5. Parallelization opportunities
6. Execution instructions: "To execute this plan, use the `/orchestrate` skill. For each segment in order, it launches an `iterative-builder` subagent with the full segment brief. Do not implement segments directly — always delegate to iterative-builder subagents. After all segments complete, run `/deep-verify`. If verification finds gaps, re-enter `/deep-plan` on unresolved items."
7. Total estimated scope (segment count, complexity, risk budget, caveats)

**No back-references:** The plan file must be readable top-to-bottom without "see Step N" or "see Issue N." Duplicate content or restructure so each section stands alone.

**Materialize large plans:** Plans with 4+ segments or 6+ issues must be saved to a plan file at `.claude/plans/[descriptive-slug]-YYYY-MM-DD.md`. Include a metadata header and an Execution Log section. Note: `.cursor/plans/` contains Cursor-generated plans (read-only reference material). Claude-generated plans go in `.claude/plans/`.

Ask the user to review, reorder, split, merge, or approve segments before any execution begins.

## Guardrails

- **This is Plan Mode.** Do not write code, create files, or run builds. Analysis and planning only.
- **Vertical slices, not horizontal layers.** Never produce a segment that is "build all models" or "write all interfaces."
- **Evidence required.** Every proposed fix must cite at least two sources.
- **Adopt-or-build required.** Every issue must include explicit evaluation of existing libraries/tools (unless purely internal refactoring).
- **Pre-mortem required.** Every issue must have pre-mortem analysis.
- **Risk budget.** Flag if 2+ segments at risk 8+.
- **Self-contained handoffs.** Segment briefs must be self-contained. Key context duplicated, not referenced by section number.
- **Exit criteria are non-negotiable.** All five gates with actual commands.
- **Commit message required.** Pre-written conventional-commit subject line per segment.
- **Build commands are project-specific.** Pull from workspace rules, BUILD files, Makefiles. Include environment setup.
- **Materialize large plans.** 4+ segments must be saved to file.
