Execute the approved deep-plan by following the orchestration protocol from `.cursor/rules/orchestration-protocol.mdc`.

**CRITICAL: Every segment MUST be executed by launching an `iterative-builder` subagent via the Agent tool (subagent_type="iterative-builder"). The orchestration agent does NOT implement segments directly.**

---

## Plan Format Detection

Detect which format the plan uses:

**Restructured directory format** (preferred): Plan path is a directory containing `manifest.md` and a `segments/` subdirectory. Read `manifest.md` for coordination; read individual segment files for handoff.

**Monolithic file format** (legacy): Plan path is a single `.md` file. Parse it to extract each segment brief.

### Pre-Execution: Cross-Plan Verification

If sibling plans exist in the same parent directory, run cross-plan verification first (follow `restructure-plan.mdc` Step 7). Present any inconsistencies to the user before launching builders.

---

## Pre-Launch: Preamble Injection

Before launching ANY builder subagent, assemble the full prompt from three sources:

1. **Read `.cursor/rules/iterative-builder-prompt.mdc`** — prepend its full contents. Provides iteration budget, structured reporting, checkpoint strategy, behavioral steering, scope verification.

2. **Read `.cursor/rules/devcontainer-exec.mdc`** — include its contents (if project uses devcontainer). Provides docker exec pattern, direnv setup, environment facts.

3. **Read the segment brief:**
   - Restructured: read `segments/{NN}-{slug}.md` directly — the entire file IS the brief.
   - Monolithic: extract `### Segment N:` block from the plan file.

**Assembled prompt = `[iterative-builder-prompt.mdc]` + `[devcontainer-exec.mdc]` + `[segment brief]`**

Always inject from rule files — do NOT rely on inline preamble in old plan files.

---

## For Each Segment (in approved execution order):

1. **Read the segment brief from disk** (not from conversation memory).

2. **Assemble the prompt** per Preamble Injection above.

3. **Launch an `iterative-builder` subagent** with the assembled prompt. Include everything — do not summarize.

4. **Parallel execution:** If the dependency graph allows it, launch up to 4 independent iterative-builder subagents concurrently.

5. **Monitor the builder.** When it completes, check its final report: PASS, PARTIAL, or BLOCKED.

6. **Verify exit gates independently.** Run the full build gate and full test gate commands from the segment brief. Do not trust the builder's self-report alone.

7. **Commit** if all gates pass:
   - Identify WIP commits: `git log --oneline | grep "WIP:"`
   - Count N WIP commits and squash: `git reset --soft HEAD~N && git commit -m "<pre-written commit message from segment brief>"`
   - If no WIP commits, commit directly with pre-written message.

8. **Update execution log:**
   - Restructured: update `execution-log.md` and segment frontmatter `status` field + manifest Segment Index.
   - Monolithic: update Execution Log table in plan file (cycles used, status, notes).

9. **Incremental verification:** For segments with risk ≥ 7/10 or High complexity, run an incremental segment verification per `deep-verify.mdc` before proceeding.

10. **Adapt if needed.** If a builder's implementation changes assumptions for a later segment, update that segment's brief before launching it.

11. **Move to next segment.** Repeat until all segments complete.

---

## If Builder Reports PARTIAL or BLOCKED:

1. **Launch an `iterative-debugger` subagent** with:
   - Full contents of `.cursor/rules/iterative-debugger-prompt.mdc`
   - Full contents of `.cursor/rules/devcontainer-exec.mdc`
   - The builder's structured final report
   - The segment brief
   - Specific failure details (failing test names, error output)

2. If debugger resolves it, re-verify exit gates and commit per standard procedure.

3. If debugger identifies a fundamental design flaw (not a bug), stop and return to the plan. Update affected issue briefs and re-slice if needed.

---

## Resuming After Debugger Resolution

If debugger reports RESOLVED but segment exit gates are not fully satisfied:

1. Re-launch an `iterative-builder` subagent with:
   - Standard preamble injection (fresh, from rule files)
   - Prepend to segment brief: "RESUME MODE: A previous builder session made partial progress and a debugger resolved a blocking issue. WIP commits exist from prior work. Start by running the targeted tests to assess current state, then continue from where the previous builder left off."
   - Fresh cycle budget (not the remainder from the original session)

2. On completion, verify exit gates and commit per standard procedure.

---

## Post-Execution Verification (Deep-Verify Loop)

After all segments are built and committed, **must** run a verification pass:

1. Switch to Plan Mode and invoke the `deep-verify` rule against the materialized plan file.

2. Review the verification report (criterion-by-criterion breakdown with PASS/PARTIAL/FAIL/UNVERIFIABLE verdicts).

3. **If FULLY VERIFIED:** Plan is complete. Update execution log with final verification result. Report to user.

4. **If PARTIALLY VERIFIED or NOT VERIFIED:** Collect all PARTIAL, FAIL, and HIGH-severity gaps. Feed into a follow-up deep-plan cycle:
   - Re-enter deep-plan at Entry Point B (Enrich Existing Plan), treating verification gaps as the existing plan.
   - Follow-up plan inherits context from original (build/test instructions, devcontainer setup, project conventions).
   - Materialize follow-up plan to a new file with `-followup` suffix.
   - Execute follow-up plan using this same orchestration protocol.
   - After follow-up segments complete, run deep-verify against the **combined** scope.
   - Repeat until FULLY VERIFIED or user decides to stop.

5. **Loop budget:** Flag if more than 2 follow-up cycles have been needed. Remaining issues likely require human design decisions.
