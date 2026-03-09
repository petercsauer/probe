Enter deep-research mode. This skill is the authoritative deep-research workflow.

This is a **read-only workflow.** Do not write code, create source files, or run builds. The only file output is the research report.

---

## Phase 1: Scope and Decompose

1. **Restate the question.** Repeat the research question in your own words. Confirm understanding.

2. **Classify complexity:**

   | Tier | Sub-questions | Subagents | Follow-up rounds | When to use |
   |------|--------------|-----------|-----------------|-------------|
   | **Focused** | 1-2 | 1-2 | 1 | Single-topic deep dive |
   | **Comparative** | 3-5 | 2-4 | 2 | Multiple alternatives evaluated |
   | **Exploratory** | 5-10 | 4 | 2 | Broad investigation or design strategy |

3. **Decompose into sub-questions.** Tag each:
   - `[codebase]` — answerable by reading code, tests, build files, docs
   - `[external]` — answerable by web search (official docs, SO, GitHub issues, blogs, RFCs)
   - `[hybrid]` — requires both codebase evidence and external context

4. **User checkpoint.** Present decomposition and complexity tier. Wait for confirmation.

---

## Phase 2: Parallel Gathering

Launch parallel explore subagents (up to 4) to collect evidence per sub-question.

**Codebase subagents** handle `[codebase]` and `[hybrid]` questions:
- Scan architecture: modules, boundaries, communication patterns
- Trace code paths end-to-end (read implementations, not just signatures)
- Check test coverage for the area
- Map dependencies

**Web subagents** handle `[external]` and `[hybrid]` questions:

For **Rust projects**, search in this priority order:
1. **crates.io** — current version, 90-day download count, last release date, repository link (maintenance signal)
2. **docs.rs** — API documentation for the current version; check for breaking changes in recent releases
3. **lib.rs** — curated crate discovery; find alternatives with "similar crates to X"
4. **doc.rust-lang.org** — the Rust reference, std lib, Cargo book, async book, edition guide
5. **GitHub** — crate repository for CHANGELOG, recent commits, open issue ratio
6. **users.rust-lang.org / internals.rust-lang.org** — community discussion, RFC tracking

For all projects (general source order after Rust-specific sources above):
- Search official documentation first
- Search Stack Overflow for high-vote, accepted answers
- Search GitHub issues and PRs on relevant repos
- Search engineering blogs and RFCs
- Note the publication date of every source

Each subagent must return findings in this structure:

```
## Sub-question N: [question text]

### Sources consulted
- [codebase] [file:lines] -- [what was checked]
- [web] [URL] -- [what was found]

### Key findings
1. [Finding with inline citation.] [1]
2. [Finding with inline citation.] [2]

### Confidence: High | Medium | Low
[One sentence justifying confidence level.]

### Open questions
- [Anything that could not be answered or needs further investigation]
```

**Parallel limits:** Max 4 subagents at once. For Focused questions, use 1-2 — do not over-parallelize.

---

## Phase 3: Evidence Assembly and Cross-Verification

### 3a. Merge findings into an evidence table:

```
| # | Sub-question | Source type | Finding | Citation | Confidence |
|---|-------------|-------------|---------|----------|------------|
| 1 | [text] | codebase | [finding] | [file:lines] | High |
| 2 | [text] | external | [finding] | [URL] | Medium |
```

### 3b. Cross-source verification:
- **Aligned:** Codebase matches external docs — note as verified
- **Contradictory:** Codebase differs from docs — flag explicitly ("docs say X but implementation does Y")
- **Supplementary:** External adds context not verifiable in codebase — note as "external-only, not verified"

### 3c. Gap detection:
For sub-questions with Low confidence, no evidence, or stale sources (2+ years on fast-moving topics):
- Launch a follow-up round (budget: 1 for Focused, 2 for Comparative/Exploratory)
- Or mark as "insufficient evidence" if gap cannot be filled

### 3d. User checkpoint:
Present assembled evidence table, contradictions, and gaps. Wait for acknowledgement.

---

## Phase 4: Synthesis and Analysis

Performed directly (no subagents). For each sub-question, write a synthesis paragraph integrating all evidence with inline citations.

**Distinguish facts from interpretations:**
- **Facts:** Directly supported by cited evidence. State declaratively.
- **Interpretations:** Inferences drawn from evidence. Mark as "Based on [evidence], this suggests..." or "Interpretation:"

**For comparative questions:** Produce a comparison table:

```
| Criterion | Option A | Option B | Winner | Evidence |
|-----------|----------|----------|--------|----------|
| Performance | [assessment] | [assessment] | [A/B/tie] | [citations] |
```

**For exploratory questions:** Produce architecture or flow descriptions. Use mermaid diagrams for component relationships, data flows, state machines (not for simple linear sequences).

**Implications and recommendations:** Actionable findings supported by evidence. Do not introduce new claims here.

---

## Phase 5: Research Report

Save report to `.cursor/research/[slug]-YYYY-MM-DD.md`.

```
# Research Report: [Title]

**Question:** [Original research question]
**Date:** YYYY-MM-DD
**Complexity:** Focused | Comparative | Exploratory
**Sub-questions:** N
**Sources:** N codebase / N external

## Executive Summary

[3-5 sentences answering the original question with confidence level. Readable standalone.]

## Detailed Findings

### Sub-question 1: [text]
[Synthesis paragraph with inline citations [1], [2], ...]

...

## Comparison Table
[Only for Comparative. Omit for Focused/Exploratory.]

## Architecture / Flow
[Only for Exploratory. Omit for Focused/Comparative.]

## Contradictions and Caveats
- [Conflicts between codebase and external sources]
- [Areas of low confidence with explanation]
- [Assumptions made during analysis]
- [Sources that may be stale]
[If none: "None identified."]

## Implications and Recommendations
1. [Actionable recommendation with supporting evidence]
...

## Sources

### Codebase
| # | File | Lines | Description |
|---|------|-------|-------------|

### External
| # | URL | Title | Accessed |
|---|-----|-------|----------|
```

Present report to user. Offer to dive deeper into any finding or transition to deep-plan mode if findings call for action.

---

## Guardrails

- **Read-only.** No code, no source files, no builds. Only the research report as file output.
- **Cite everything.** Every factual claim must have an inline citation. Unsupported claims marked as interpretations.
- **No fabrication.** If evidence is insufficient, say so. "Insufficient evidence" is a valid finding.
- **Distinguish codebase from external.** Never conflate what code does with what docs say it should do.
- **Date your sources.** Flag sources older than 2 years on fast-moving topics.
- **Broad-to-narrow search.** Start with short, broad queries before narrowing. Long hyper-specific queries miss results.
- **Match subagent count to complexity.** Don't over-parallelize Focused questions; don't under-parallelize Exploratory.
- **Budget follow-up rounds.** Max 1 for Focused, 2 for Comparative/Exploratory. Then mark as "insufficient evidence."
- **Stay neutral.** Facts in Detailed Findings; opinions in Implications and Recommendations.
- **Acknowledge uncertainty.** Low confidence must be stated explicitly.
