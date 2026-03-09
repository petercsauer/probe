Execute a large, multi-subsection deep-plan with state persistence, wave-based parallel dispatch, subsection integration gates, and a progress dashboard. Use this skill instead of `/orchestrate` when the plan has more than 8 segments or a two-level hierarchy (master plan → subsection plans → segments).

**CRITICAL: Every segment MUST be executed by launching an `iterative-builder` subagent via the Agent tool. The orchestration agent does NOT implement segments directly.**

## Design Principles (Named Patterns)

This skill implements five established orchestration patterns:

1. **Kahn's Algorithm (parallel variant) / Supersteps** [Bazel, LangGraph] — Topological sort produces waves of independent segments. All segments in a wave launch simultaneously; the wave completes when all builders report. This is how Bazel schedules build actions and LangGraph schedules parallel agent branches.

2. **Durable Execution / Checkpointing** [Temporal, Prefect, Dagster] — Execution state is persisted to disk after every wave so the orchestrator can resume after context-window exhaustion or interruption without re-running completed work. Equivalent to Temporal's event-history checkpointing and Prefect's `persist_result=True`.

3. **`fail-fast: false`** [GitHub Actions, Codefresh, Jenkins] — When a segment fails, other independent segments continue rather than aborting the pipeline. Dependent segments are marked `waiting-on-debug`. The pipeline only fully halts when there is no independent work left to do.

4. **Quality Gate / Stage-Gate Model** [SonarQube, Tekton, Jenkins] — After all segments in a subsection pass, a mandatory integration gate runs the full workspace build, test suite, and smoke test before the next subsection can start. This is the software delivery equivalent of the Stage-Gate Model (Cooper, 1986).

5. **Orchestrator-Worker / Scatter-Gather** [LangGraph, CrewAI, AutoGen] — The orchestrating agent dispatches multiple builder subagents (workers), collects their structured reports, merges results into shared execution state, and coordinates the next wave. Deferred execution handles asymmetric parallel branches — a wave does not advance until every builder in it has reported, regardless of order of completion.

**Configuration:**
- `fail-fast: true` — abort the entire pipeline on first segment failure (use for high-risk plans where later segments assume all earlier ones succeeded)
- `fail-fast: false` (default) — continue independent segments while debugger works on blocked segment

---

## Phase 0: Startup — Resume or Fresh Start

Before doing anything else, check for an existing execution state file adjacent to the plan:

```bash
cat <plan-dir>/execution-state.json 2>/dev/null
```

**If state file exists:**
Present the current status dashboard (see Phase 4 format) and ask:
> "Existing execution state found. N segments complete, M segments pending, K blocked.
> Resume from wave [W]? Or restart from the beginning? (resume / restart)"

On **resume**: skip to Phase 3, starting at the first incomplete wave.
On **restart**: delete the state file and proceed to Phase 1.

**If no state file:** proceed to Phase 1.

---

## Phase 1: Plan Ingestion and Hierarchy Detection

### 1a. Detect plan format

Read the plan file or directory. Detect whether it is:

- **Master plan** (two-level): The plan's segments are subsections, each with a `Cursor plan:` or `plan file:` field pointing to a subsection plan. Navigate into each subsection plan to extract builder-facing segments.
- **Flat plan** (one-level): All segments are builder-facing directly. Use standard segment extraction.

For the PRB project, the master plan is `.claude/plans/prb-phase1-master-2026-03-09.md` and subsection plans are in `.cursor/plans/universal-message-debugger-phase1-2026-03-08/`.

### 1b. Build the full segment list

For each plan level, extract every segment with:
- Segment ID (e.g., `S1.1`, `S2.3`)
- Title
- `depends_on` list (segment IDs or subsection names)
- Risk, complexity, cycle budget
- Which plan file contains its brief (for preamble assembly)
- Subsection membership

**For master plans:** enumerate subsection plans and their segments. Produce a flat list of all segments tagged with their subsection.

### 1c. Initialize the state file

Write `<plan-dir>/execution-state.json`:

```json
{
  "plan": "<master plan path>",
  "started": "<ISO 8601 timestamp>",
  "current_wave": 1,
  "segments": {
    "S1.1": { "status": "pending", "subsection": "Sub1", "title": "Workspace + Core Types" },
    "S1.2": { "status": "pending", "subsection": "Sub1", "title": "Traits + Fixture Adapter" },
    ...
  },
  "subsection_gates": {
    "Sub1": "pending",
    "Sub2": "pending",
    "Sub3": "pending",
    "Sub4": "pending"
  }
}
```

---

## Phase 2: Wave Computation and Pre-Flight Display

### 2a. Compute waves from the DAG

Topological sort: assign each segment to a wave number.

```
Wave N = all segments whose every dependency is in a wave < N
```

Example for PRB:
```
Wave 1:  S1.1
Wave 2:  S1.2
Wave 3:  S1.3                          ← Sub1 integration gate after
Wave 4:  S2.1
Wave 5:  S2.2
Wave 6:  S2.3 ║ S2.4                  ← parallel  ← Sub2 integration gate after
Wave 7:  S3.1
Wave 8:  S3.2
Wave 9:  S3.3
Wave 10: S3.4
Wave 11: S3.5                          ← Sub3 integration gate after
Wave 12: S4.1
Wave 13: S4.2 ║ S4.3                  ← parallel  ← Sub4 integration gate after
```

Mark each wave as:
- **Serial** (1 segment)
- **Parallel** (2+ independent segments — all launch simultaneously)
- **Gate** (subsection integration check runs after this wave before advancing)

### 2b. Display pre-flight wave plan

Print the full wave plan and total segment count. Ask for user confirmation before proceeding.

```
══════════════════════════════════════════════════════════
PRB Phase 1 — Execution Wave Plan
══════════════════════════════════════════════════════════
Total: 13 segments across 4 subsections, 13 waves

  Wave 1   │ S1.1 Workspace + Core Types         │ Low  │ 10cy
  Wave 2   │ S1.2 Traits + Fixture Adapter        │ Med  │ 15cy
  Wave 3   │ S1.3 CLI + Walking Skeleton          │ Med  │ 15cy  ← SUB1 GATE
  Wave 4   │ S2.1 MCAP Session Storage            │ Med  │ 15cy
  Wave 5   │ S2.2 Protobuf Schema Registry        │ Med  │ 15cy
  Wave 6   │ S2.3 Schema-backed Decode            │ High │ 20cy
           │ S2.4 Wire-format Decode    (║ PARALLEL) │ Med │ 15cy  ← SUB2 GATE
  Wave 7   │ S3.1 PCAP/pcapng Reader              │ Low  │ 10cy
  Wave 8   │ S3.2 Packet Normalization            │ Med  │ 15cy
  Wave 9   │ S3.3 TCP Reassembly                  │ Med  │ 15cy
  Wave 10  │ S3.4 TLS Decryption        ⚠️ Risk 8 │ High │ 20cy
  Wave 11  │ S3.5 Pipeline Integration + CLI      │ Med  │ 15cy  ← SUB3 GATE
  Wave 12  │ S4.1 gRPC/HTTP2 Decoder              │ High │ 20cy
  Wave 13  │ S4.2 ZMTP Decoder                    │ Med  │ 15cy
           │ S4.3 DDS/RTPS Decoder     (║ PARALLEL) │ Med │ 15cy  ← SUB4 GATE

Estimated builder-hours: ~215 cycles total
Parallelization saves: ~35 cycles vs serial execution
══════════════════════════════════════════════════════════
Proceed? (yes / adjust-order / abort)
```

Do NOT launch any builders until the user confirms.

---

## Phase 3: Wave Execution Loop

For each wave in order:

### 3a. Read segment briefs for this wave

For each segment in the wave, assemble the builder prompt:
1. Read `.claude/commands/iterative-builder.md` — full contents
2. Read `.claude/commands/devcontainer-exec.md` — full contents
3. Read the segment brief from its subsection plan file (or from the master plan if flat)

**Assembled prompt = `[iterative-builder.md]` + `[devcontainer-exec.md]` + `[segment brief]`**

### 3b. Update state → mark segments as `running`

Write to `execution-state.json`: set each segment in this wave to `"status": "running"`.

### 3c. Launch builders

**Serial wave (1 segment):** Launch one `iterative-builder` subagent. Wait for completion.

**Parallel wave (2+ segments):** Launch ALL segments in the wave simultaneously as separate `iterative-builder` subagents. Wait for ALL to complete before advancing. Do not advance to the next wave until every builder in this wave has reported.

Maximum concurrent builders: **4** (up from implicit in orchestrate.md — for PRB this plan never exceeds 2 concurrent, so this limit is never hit).

### 3d. Process builder reports

For each completed builder:

**If PASS:**
1. Verify exit gates independently:
   ```bash
   cargo build --workspace
   cargo nextest run -p <crate>
   ```
2. Squash WIP commits:
   ```bash
   N=$(git log --oneline | grep -c "WIP: <segment title>")
   git reset --soft HEAD~$N
   git commit -m "<pre-written commit message from segment brief>"
   ```
3. Update state: `"status": "pass", "cycles": N, "commit": "<hash>", "completed": "<timestamp>"`

**If PARTIAL or BLOCKED:**
1. Update state: `"status": "blocked", "stuck_on": "<description>"`
2. Launch `iterative-debugger` subagent (see Debugger Protocol below)
3. **Do NOT advance the wave.** Mark downstream segments as `"status": "waiting-on-debug"`.
4. **`fail-fast` check:**
   - If `fail-fast: true` → halt all remaining waves, surface the failure, wait for user direction.
   - If `fail-fast: false` (default) → identify segments in later waves that do NOT transitively depend on the blocked segment. Note them to the user as launchable in a parallel session while the debugger works. This is the **deferred execution** pattern — the blocked branch does not prevent progress on unblocked branches.

### 3e. Subsection integration gate (after final wave of each subsection)

After all segments of a subsection complete with PASS, run the integration gate before advancing to the next subsection:

```bash
# All subsections
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace -- -D warnings

# Sub1 only — walking skeleton smoke test
./target/debug/prb ingest fixtures/sample.json | ./target/debug/prb inspect --format table

# Sub2 only — schema round-trip smoke test
./target/debug/prb schemas load <test.proto> && echo "schema load OK"

# Sub3 only — pcap ingest smoke test
./target/debug/prb ingest tests/fixtures/sample.pcapng --output /tmp/test.mcap && echo "pcap ingest OK"

# Sub4 only — full decode smoke test
./target/debug/prb ingest tests/fixtures/grpc.pcapng --output /tmp/test.mcap \
  && ./target/debug/prb inspect /tmp/test.mcap | grep "grpc" && echo "grpc decode OK"
```

**If gate PASSES:** Update `subsection_gates.<SubN>: "pass"` in state file. Print dashboard. Advance to next wave.

**If gate FAILS:** Set `subsection_gates.<SubN>: "blocked"`. Do NOT advance. Launch a debugger subagent against the gate failure. This is a HIGH severity event — surface it clearly to the user before proceeding.

### 3f. Print progress dashboard

After every wave completes (pass or fail), print:

```
══════════════════════════════════════════════════════════
PRB Phase 1 — Progress  [Wave 6/13 complete]
══════════════════════════════════════════════════════════
Sub1: Foundation & Core Model       ✅ GATE PASSED
  ✅ S1.1 Workspace + Core Types        8cy  abc1234
  ✅ S1.2 Traits + Fixture Adapter     11cy  def5678
  ✅ S1.3 CLI + Walking Skeleton       14cy  ghi9012

Sub2: Storage & Schema Engine        🔄 IN PROGRESS
  ✅ S2.1 MCAP Session Storage         13cy  jkl3456
  ✅ S2.2 Protobuf Schema Registry     15cy  mno7890
  ✅ S2.3 Schema-backed Decode         18cy  pqr1234   (was parallel)
  ✅ S2.4 Wire-format Decode           12cy  stu5678   (was parallel)
  ─ Subsection gate: running...

Sub3: Network Capture Pipeline       ⬜ PENDING
  ⬜ S3.1 – S3.5 (5 segments)

Sub4: Protocol Decoders              ⬜ PENDING
  ⬜ S4.1 – S4.3 (3 segments)
══════════════════════════════════════════════════════════
7/13 segments complete │ 2 parallel waves used │ ~108 cycles saved
```

### 3g. Save state after every wave

After processing all builder reports and the subsection gate (if applicable), write the complete updated state to `execution-state.json`. This is the resume checkpoint.

---

## Debugger Protocol

When a builder reports PARTIAL or BLOCKED:

1. Launch `iterative-debugger` subagent with:
   - Full contents of `.claude/commands/iterative-debugger.md`
   - Full contents of `.claude/commands/devcontainer-exec.md`
   - The builder's structured final report (exact text from builder output)
   - The segment brief
   - Specific failing test names and error output

2. Update state: `"status": "debugging"` for the affected segment.

3. **While the debugger runs:** check if any OTHER segments (from later waves, independent of the blocked segment) could be launched. If yes, surface this to the user:
   > "S3.3 is blocked and a debugger is running. S3.4 and S3.5 depend on S3.3 and cannot proceed. No other independent segments are available. Waiting for debugger..."

4. If debugger reports RESOLVED:
   - Re-launch `iterative-builder` in RESUME MODE with a fresh cycle budget
   - Prepend to brief: "RESUME MODE: A debugger resolved a blocking issue. WIP commits exist. Start by running targeted tests to assess current state."
   - On PASS: verify exit gates, squash commits, update state, advance.

5. If debugger reports BLOCKED:
   - Update state: `"status": "blocked-unresolved"`
   - Surface the debugger's report to the user with recommended next steps
   - Ask: "Re-plan this segment? Skip and continue with independent work? Abort?"
   - Do NOT auto-advance past a blocked segment with unresolved issues.

---

## Post-Execution: Deep-Verify Loop

After all waves complete:

1. Run `/deep-verify` against the master plan file.

2. Review the criterion-by-criterion report.

3. **If FULLY VERIFIED:**
   - Update `execution-state.json`: `"status": "complete", "verified": "<timestamp>"`
   - Update the plan file's execution log
   - Print final summary

4. **If PARTIALLY VERIFIED or NOT VERIFIED:**
   - Feed gaps into a follow-up `/deep-plan` (Entry Point B)
   - Save follow-up plan to `<plan-dir>-followup-YYYY-MM-DD.md`
   - Execute follow-up using this same skill
   - Re-verify after follow-up against combined scope
   - Flag to user if more than 2 follow-up cycles have been needed

---

## Resume Protocol (detailed)

When resuming from `execution-state.json`:

1. Read all segment statuses.
2. Identify the first incomplete wave (any segment with status != "pass").
3. For segments in "running" state (orphaned from previous context): treat as UNKNOWN. Run their targeted tests to determine actual status:
   ```bash
   cargo nextest run -p <crate>
   ```
   If tests pass → mark PASS. If tests fail → mark PARTIAL and escalate to debugger.
4. For segments in "waiting-on-debug": check git log for a commit matching the segment title. If found → mark PASS. If not → re-launch builder in RESUME MODE.
5. Resume from the first non-passing wave.
6. Print dashboard showing what's already done before continuing.

---

## Adaptation Protocol

When a builder's implementation changes assumptions for a later segment:

1. Note the change in `execution-state.json` under `"adaptations": [...]`.
2. Read the affected downstream segment brief.
3. Edit the segment file to reflect the updated assumptions before launching that wave.
4. If the change affects MULTIPLE downstream segments (e.g., a core interface changed), update ALL affected segment files before advancing. For PRB: a change to `DebugEvent` in Sub1 could require updating segment briefs in Sub2, Sub3, Sub4.
5. For fundamental design changes: stop, surface to user, re-enter `/deep-plan` on affected segments rather than forcing forward.
