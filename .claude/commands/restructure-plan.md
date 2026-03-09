Restructure a monolithic deep-plan file into a per-segment handoff directory. This skill is the authoritative restructure-plan workflow.

Also invocable as "verify cross-plan consistency for [directory]" to run only Step 7 without restructuring.

---

## Step 1: Parse the Monolithic Plan

Read the monolithic plan file and identify:
1. Metadata header — Goal, Generated, Rules version, Entry point, Status, Parent plan
2. Overview — summary paragraph
3. Dependency diagram — mermaid or text
4. Issue Analysis Briefs — each `### Issue {ID}: {Title}` block through `**Blast Radius:**`
5. Segment Briefs — each `### Segment {N}: {Title}` block through `**Commit message:**`
6. Parallelization Opportunities
7. Execution Instructions
8. Execution Log

If any component is missing, warn the user but continue.

---

## Step 2: Create the Directory Structure

```
.claude/plans/<plan-slug>/
  manifest.md
  issues/
    issue-{ID}-{slug}.md          # one per issue analysis brief
  segments/
    {NN}-{slug}.md                # one per segment brief (self-contained)
  execution-log.md
  cross-plan-report.md            # generated if sibling plans exist

Note: `.cursor/plans/` contains Cursor-generated plans (read-only reference). Restructured Claude plans go in `.claude/plans/`.
```

- `{NN}` is zero-padded (01, 02, ...)
- `{slug}` is lowercase-kebab from the title
- If sibling plans exist in parent directory, create a subdirectory for this subsection

---

## Step 3: Generate the Manifest

```markdown
---
plan: "<plan title>"
goal: "<one-sentence goal>"
generated: YYYY-MM-DD
status: Ready for execution | In progress | Complete
parent_plan: "<relative path to parent plan, if any>"
rules_version: YYYY-MM-DD
---

# <Plan Title> -- Manifest

## Dependency Diagram
<copy mermaid/text diagram from original plan>

## Segment Index
| # | Title | File | Depends On | Risk | Complexity | Status |
|---|-------|------|------------|------|------------|--------|
| 1 | <title> | segments/01-<slug>.md | None | N/10 | Medium | pending |

## Parallelization
<copy parallelization opportunities from original plan>

## Preamble Injection
Before launching any builder subagent, the orchestration agent assembles the prompt:
1. Read `.claude/commands/iterative-builder.md`
2. Read `.claude/commands/devcontainer-exec.md`
3. Read the segment file from `segments/{NN}-{slug}.md`

Assembled prompt = [preamble contents] + [segment file contents]

## Execution Instructions
<copy execution instructions from original plan, updated to reference segment files>
```

The manifest does NOT contain issue briefs or segment briefs.

---

## Step 4: Extract Issue Files

For each Issue Analysis Brief, create `issues/issue-{ID}-{slug}.md`:

```markdown
---
id: "{issue ID}"
title: "{issue title}"
risk: N/10
addressed_by_segments: [1, 2]
---

# Issue {ID}: {Title}

<full issue analysis brief content verbatim: Core Problem, Root Cause, Proposed Fix,
Existing Solutions Evaluated, Alternatives Considered, Pre-Mortem, Risk Factor,
Evidence for Optimality, Blast Radius>
```

These are reference material for humans and the orchestration agent — builders do not read them directly.

---

## Step 5: Extract Segment Files (Core Deliverable)

For each Segment Brief, create `segments/{NN}-{slug}.md`. Each file must be **completely self-contained**.

```markdown
---
segment: N
title: "{segment title}"
depends_on: [1, 2] or []
risk: N/10
complexity: Low | Medium | High
cycle_budget: N
status: pending
commit_message: "type(scope): description"
---

# Segment {N}: {Title}

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** <one sentence>
**Depends on:** <prior segment numbers, or "None">

## Context: Issues Addressed

<For each issue this segment addresses, inline condensed summary:
- Core Problem (2-3 sentences)
- Proposed Fix (key technical details only)
- Pre-Mortem risks relevant to this segment
Do NOT reference the issue file — paste essential context here.>

## Scope
<subsystem / module / area list>

## Key Files and Context
<specific files, functions, contracts, invariants>

## Implementation Approach
<concrete pattern guidance>

## Alternatives Ruled Out
<approaches already rejected — prevents builder from re-discovering dead ends>

## Pre-Mortem Risks
<things to watch for and write defensive tests against>

## Build and Test Commands
- Build: <exact command>
- Test (targeted): <exact command>
- Test (regression): <exact command>
- Test (full gate): <exact command>

## Exit Criteria
1. **Targeted tests:** [test: what it validates]
2. **Regression tests:** [existing targets that must pass]
3. **Full build gate:** [exact command]
4. **Full test gate:** [exact command]
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no out-of-scope changes.
6. **Scope verification gate:** Changed files match stated scope.
```

### Inlining Issue Context
For each issue addressed by the segment:
1. Read the issue's Core Problem, Proposed Fix, and Pre-Mortem sections
2. Condense to essential builder-facing context (skip Existing Solutions Evaluated, Evidence for Optimality, full Blast Radius — those are planning artifacts)
3. Paste directly into "Context: Issues Addressed"

---

## Step 6: Generate Execution Log

Create `execution-log.md`:

```markdown
# Execution Log

| Segment | Est. Complexity | Risk | Cycles Used | Status | Notes |
|---------|----------------|------|-------------|--------|-------|
| 1: <title> | Medium | 4/10 | -- | pending | -- |

**Deep-verify result:** --
**Follow-up plans:** --
```

---

## Step 7: Cross-Plan Verification

If sibling plans exist in the same parent directory, run these six verification categories:

**Category 1 — Path Consistency:** Extract all crate/module directory paths. Flag entities with inconsistent paths across plans.

**Category 2 — Interface Contract Consistency:** Extract trait/struct/enum definitions. For each type defined in an upstream plan, check downstream plans use the same signature (method names, parameter types, return types).

**Category 3 — Dependency Assumption Verification:** Check that artifacts assumed by downstream segments ("After Subsection N, the following exist: ...") appear in upstream plan exit criteria or scope.

**Category 4 — Build Command Consistency:** Extract all build/test commands. Verify package names match crate names defined in the creating plan.

**Category 5 — Scope Overlap Detection:** Collect all file paths from Scope sections across all plans. Flag files modified by segments in different plans (potential merge conflicts).

**Category 6 — Top-Level Plan Alignment:** If a parent plan exists, verify each sub-plan's scope stays within parent's subsection boundaries.

Write `cross-plan-report.md`:

```markdown
# Cross-Plan Verification Report

**Plans verified:** <list>
**Upstream authority:** <which plan defines canonical interfaces>
**Verdict:** CONSISTENT | INCONSISTENCIES FOUND

## Inconsistencies

### [Category N]: [Short description]
- **Upstream** (`<file>`): <what it defines>
- **Downstream** (`<file>`): <what it assumes>
- **Impact:** <what breaks if not fixed>
- **Recommended fix:** <which file to update and how>
- **Auto-correctable:** Yes | No (requires human decision)

## Reconciliation Actions
| File | Change | Rationale |
|------|--------|-----------|
```

**Reconciliation:** For auto-correctable inconsistencies: identify upstream authority (earlier subsections are upstream), present proposed corrections to user, apply on approval to segment files in `segments/`. Do NOT modify original monolithic plan files. For ambiguous inconsistencies, flag for human decision.

---

## Step 8: Factual Freshness Verification

### 8a: Extract verifiable claims from all plan files:
- Library versions (e.g., "mcap v0.24.0")
- Download/popularity metrics
- Maintenance status assertions
- API signatures from external libraries
- External URLs
- Crate/package existence claims

### 8b: Verify against live sources (web search):
- **Versions:** Check crates.io/npm/PyPI. Flag newer major/minor versions, yanked versions, renamed/deprecated packages.
- **Maintenance:** Check last commit date (flag "actively maintained" with no commits in 6+ months), open issues ratio, archival notices.
- **API signatures:** Spot-check key APIs in docs.rs or official docs for changes in signatures, return types, module paths.
- **Crate existence:** Verify on crates.io. Flag phantom crates and missed crates.
- **URLs:** Verify segment-brief URLs resolve (not 404). Don't fetch every URL — prioritize builder-facing URLs.

### 8c: Append to cross-plan report:

```markdown
## Factual Freshness Verification
**Claims checked:** N / **Stale/incorrect:** M / **Verified current:** K

### Stale Claims
#### [Library name]: Version outdated
- **Plan states:** v0.16.3 / **Current:** v0.17.0 (breaking changes in X)
- **Impact:** [does plan's API usage still work?]
- **Recommendation:** Update version pin and verify API compatibility
```

### 8d: Apply corrections:
- Patch/minor bumps with backward compatibility: update version numbers in segment files directly.
- Breaking API changes or maintenance status changes: flag for user, do NOT auto-correct.

---

## Step 9: Validate Self-Containment

Before finishing, verify every segment file passes:

- [ ] YAML frontmatter with all required fields (segment, title, depends_on, risk, complexity, cycle_budget, status, commit_message)
- [ ] Contains "self-contained handoff contract" header note
- [ ] Contains Goal, Scope, Key Files, Implementation Approach, Exit Criteria sections
- [ ] Does NOT contain "see Issue", "see Step", "see Segment", or "see above" references
- [ ] Contains build/test commands (not placeholders)
- [ ] Contains at least one targeted test in exit criteria
- [ ] All relevant issue context is inlined (not referenced)

Report any failures and fix them before declaring restructure complete.

---

## Step 10: Summary

```
Restructured: <plan title>
  Manifest: <path>
  Issues: N files in issues/
  Segments: N files in segments/
  Cross-plan: CONSISTENT | N inconsistencies found (M auto-corrected)
  Freshness: N claims checked, M stale (K auto-corrected)
```

Note that the original monolithic plan file is preserved as-is for reference. The orchestration protocol reads from the restructured directory.
