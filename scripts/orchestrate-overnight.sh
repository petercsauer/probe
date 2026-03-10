#!/usr/bin/env bash
#
# Phase 2 Overnight Orchestration Runner
#
# The SCRIPT is the orchestrator. Each segment gets its own `claude -p` call
# with a focused builder prompt. No nested subagents, no complex protocol —
# just: read segment brief → build → test → commit → next.
#
# Usage:
#   ./scripts/orchestrate-overnight.sh              # run
#   ./scripts/orchestrate-overnight.sh --status      # print progress
#   ./scripts/orchestrate-overnight.sh --dry-run     # show wave plan
#
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────
# Configuration
# ──────────────────────────────────────────────────────────────────────
PLAN_DIR=".claude/plans/phase2-orchestrated"
STATE_FILE="$PLAN_DIR/execution-state.json"
SEGMENTS_DIR="$PLAN_DIR/segments"
BUILDER_SKILL=".claude/commands/iterative-builder.md"
LOG_DIR="logs/phase2-orchestrate"
NOTIFY_SCRIPT="./scripts/notify-imessage.sh"

MAX_RETRIES_PER_SEGMENT=2
MAX_PARALLEL=4
SESSION_TIMEOUT=1800     # 30 min per segment
RETRY_DELAY=10
NETWORK_RETRY_MAX=600    # 10 min max waiting for network

# Enable/disable notifications (set to 0 to disable)
ENABLE_NOTIFICATIONS="${PRB_NOTIFY:-1}"

# Wave definitions: each wave is a space-separated list of segment numbers
# Derived from the manifest dependency graph
WAVE_1="01 02 03 04 05 06 07 12 16 24"
WAVE_2="08 09 10 11 13 17 25"
WAVE_3="14 18 26"
WAVE_4="15 19 27 28"
WAVE_5="20"
WAVE_6="21 29"
WAVE_7="22 23"
ALL_WAVES=("$WAVE_1" "$WAVE_2" "$WAVE_3" "$WAVE_4" "$WAVE_5" "$WAVE_6" "$WAVE_7")

# Segment number → filename (function avoids bash octal issue with 08/09)
seg_file() {
    local files
    files=(
        "01-query-language.md"
        "02-conversation-reconstruction.md"
        "03-export-formats.md"
        "04-otel-trace-correlation.md"
        "05-ai-explanation.md"
        "06-tui-core.md"
        "07-data-integration.md"
        "08-event-list-pane.md"
        "09-decode-tree-pane.md"
        "10-hex-dump-pane.md"
        "11-timeline-pane.md"
        "12-capture-engine.md"
        "13-live-pipeline.md"
        "14-capture-cli.md"
        "15-tui-live-mode.md"
        "16-pipeline-traits.md"
        "17-mmap-reader.md"
        "18-parallel-normalization.md"
        "19-flow-partitioned-reassembly.md"
        "20-parallel-tls-decode.md"
        "21-streaming-pipeline.md"
        "22-benchmarks.md"
        "23-parallel-cli.md"
        "24-protocol-detector.md"
        "25-decoder-registry.md"
        "26-pipeline-integration.md"
        "27-native-plugins.md"
        "28-wasm-plugins.md"
        "29-plugin-cli.md"
    )
    local idx=$((10#$1 - 1))
    echo "${files[$idx]}"
}

# ──────────────────────────────────────────────────────────────────────
# Helpers
# ──────────────────────────────────────────────────────────────────────
ts() { date "+%Y-%m-%d %H:%M:%S"; }
ts_file() { date "+%Y%m%d-%H%M%S"; }

red()    { printf '\033[0;31m%s\033[0m' "$*"; }
green()  { printf '\033[0;32m%s\033[0m' "$*"; }
yellow() { printf '\033[0;33m%s\033[0m' "$*"; }
cyan()   { printf '\033[0;36m%s\033[0m' "$*"; }
bold()   { printf '\033[1m%s\033[0m' "$*"; }

log() { echo "[$(ts)] $*"; }

# ──────────────────────────────────────────────────────────────────────
# Notifications
# ──────────────────────────────────────────────────────────────────────
notify() {
    [[ "$ENABLE_NOTIFICATIONS" != "1" ]] && return 0
    [[ ! -x "$NOTIFY_SCRIPT" ]] && return 0
    "$NOTIFY_SCRIPT" "$@" &> /dev/null || true
}

# ──────────────────────────────────────────────────────────────────────
# Bedrock env setup
# ──────────────────────────────────────────────────────────────────────
setup_bedrock_env() {
    export ANTHROPIC_MODEL="${CUI_DEFAULT_MODEL:-us-gov.anthropic.claude-sonnet-4-5-20250929-v1:0}"
    export ANTHROPIC_HAIKU_MODEL="${CUI_DEFAULT_HAIKU:-anthropic.claude-3-haiku-20240307-v1:0}"
    export AWS_REGION="${CUI_AWS_REGION:-us-gov-west-1}"
    export AWS_BEARER_TOKEN_BEDROCK="$CUI_BEDROCK_API_KEY"
    export CLAUDE_CODE_USE_BEDROCK=1
}

# ──────────────────────────────────────────────────────────────────────
# State management
# ──────────────────────────────────────────────────────────────────────
init_state() {
    if [[ -f "$STATE_FILE" ]]; then
        return
    fi
    python3 -c "
import json
segments = {}
files = {
    '01': ('TUI', 'Query Language Engine'),
    '02': ('Core', 'Conversation Reconstruction'),
    '03': ('Export', 'Export Formats'),
    '04': ('OTel', 'OTel Trace Correlation'),
    '05': ('AI', 'AI-Powered Explanation'),
    '06': ('TUI', 'TUI Core & App Shell'),
    '07': ('TUI', 'Data Layer & CLI Integration'),
    '08': ('TUI', 'Event List Pane'),
    '09': ('TUI', 'Decode Tree Pane'),
    '10': ('TUI', 'Hex Dump Pane'),
    '11': ('TUI', 'Timeline Pane'),
    '12': ('Capture', 'Capture Engine'),
    '13': ('Capture', 'Live Pipeline Integration'),
    '14': ('Capture', 'Capture CLI'),
    '15': ('Capture', 'TUI Live Mode'),
    '16': ('Parallel', 'Pipeline Trait Refactoring'),
    '17': ('Parallel', 'Mmap Reader'),
    '18': ('Parallel', 'Parallel Normalization'),
    '19': ('Parallel', 'Flow-Partitioned Reassembly'),
    '20': ('Parallel', 'Parallel TLS + Decode'),
    '21': ('Parallel', 'Streaming Pipeline'),
    '22': ('Parallel', 'Benchmarks'),
    '23': ('Parallel', 'Parallel CLI Integration'),
    '24': ('Detect', 'Protocol Detector Trait + Built-ins'),
    '25': ('Detect', 'Decoder Registry + Dispatch'),
    '26': ('Detect', 'Pipeline Integration'),
    '27': ('Detect', 'Native Plugin System'),
    '28': ('Detect', 'WASM Plugin System'),
    '29': ('Detect', 'Plugin Management CLI'),
}
for num, (track, title) in files.items():
    segments[f'S{num}'] = {'status': 'pending', 'track': track, 'title': title, 'attempts': 0}
state = {
    'plan': '$PLAN_DIR/manifest.md',
    'started': '$(date -u +%Y-%m-%dT%H:%M:%SZ)',
    'segments': segments,
}
with open('$STATE_FILE', 'w') as f:
    json.dump(state, f, indent=2)
print('State file created')
"
}

get_segment_status() {
    local seg_num="$1"
    python3 -c "
import json
with open('$STATE_FILE') as f:
    state = json.load(f)
print(state['segments'].get('S$seg_num', {}).get('status', 'unknown'))
" 2>/dev/null || echo "unknown"
}

set_segment_status() {
    local seg_num="$1"
    local status="$2"
    local extra="${3:-}"
    python3 -c "
import json
with open('$STATE_FILE') as f:
    state = json.load(f)
seg = state['segments'].setdefault('S$seg_num', {})
seg['status'] = '$status'
if '$extra':
    for kv in '$extra'.split(','):
        if '=' in kv:
            k, v = kv.split('=', 1)
            seg[k.strip()] = v.strip()
with open('$STATE_FILE', 'w') as f:
    json.dump(state, f, indent=2)
"
}

increment_attempts() {
    local seg_num="$1"
    python3 -c "
import json
with open('$STATE_FILE') as f:
    state = json.load(f)
seg = state['segments'].get('S$seg_num', {})
seg['attempts'] = seg.get('attempts', 0) + 1
with open('$STATE_FILE', 'w') as f:
    json.dump(state, f, indent=2)
print(seg['attempts'])
"
}

get_attempts() {
    local seg_num="$1"
    python3 -c "
import json
with open('$STATE_FILE') as f:
    state = json.load(f)
print(state['segments'].get('S$seg_num', {}).get('attempts', 0))
" 2>/dev/null || echo 0
}

progress_summary() {
    python3 -c "
import json, os
if not os.path.exists('$STATE_FILE'):
    print('  No state file — fresh start')
    return
with open('$STATE_FILE') as f:
    state = json.load(f)
segs = state.get('segments', {})
total = len(segs)
done = sum(1 for v in segs.values() if v.get('status') == 'pass')
blocked = sum(1 for v in segs.values() if 'block' in v.get('status', ''))
running = sum(1 for v in segs.values() if v.get('status') == 'running')
failed = sum(1 for v in segs.values() if v.get('status') == 'failed')
pending = total - done - blocked - running - failed
pct = int(done / max(total, 1) * 100)
bar_w = 30
bar_done = int(pct / 100 * bar_w)
bar_left = bar_w - bar_done
bar = '█' * bar_done + '░' * bar_left
print(f'  [{bar}] {pct}%  ({done}/{total} segments)')
print(f'  ✅ {done} pass | ⏳ {pending} pending | 🔧 {running} running | ❌ {blocked} blocked | 💥 {failed} failed')
passed = [k for k, v in segs.items() if v.get('status') == 'pass']
if passed:
    print(f'  Completed: {\" \".join(sorted(passed))}')
" 2>/dev/null || echo "  (unable to parse state)"
}

# ──────────────────────────────────────────────────────────────────────
# Network check
# ──────────────────────────────────────────────────────────────────────
wait_for_network() {
    local waited=0
    local delay=10
    while ! curl -s --max-time 5 https://api.anthropic.com >/dev/null 2>&1; do
        waited=$((waited + delay))
        if [[ $waited -ge $NETWORK_RETRY_MAX ]]; then
            log "$(red "Network unreachable for ${NETWORK_RETRY_MAX}s, continuing anyway...")"
            return
        fi
        log "$(yellow "  📡 Network down. Retry in ${delay}s... (${waited}/${NETWORK_RETRY_MAX}s)")"
        sleep "$delay"
        delay=$((delay < 60 ? delay * 2 : 60))
    done
}

# ──────────────────────────────────────────────────────────────────────
# Build a focused prompt for one segment
# ──────────────────────────────────────────────────────────────────────
build_segment_prompt() {
    local seg_num="$1"
    local seg_file="$(seg_file "$seg_num")"
    local prompt_file="/tmp/prb-segment-${seg_num}.md"

    cat > "$prompt_file" <<PROMPT_HEADER
You are an iterative-builder. Build ONE segment and report results.

## Your Operating Protocol

Read and follow \`.claude/commands/iterative-builder.md\` exactly. It defines your cycle budget, testing strategy, checkpoint strategy, and report format.

## Your Segment Brief

Read \`$SEGMENTS_DIR/$seg_file\` — this is what you must build.

## Rules

- This is UNATTENDED. Do NOT ask questions. Make reasonable decisions.
- Do NOT spawn subagents or nested tasks. Work directly.
- Read the iterative-builder.md file FIRST, then the segment brief, then start building.
- Follow the staged testing strategy: Build → Targeted tests → Regression → Full gate.
- Create WIP commits after each cycle with passing tests.
- When done, output your structured final report (PASS / PARTIAL / BLOCKED).
- Run from workspace root. Use \`cargo check -p <crate>\` before full builds.

## Begin now. Read the two files above and start building.
PROMPT_HEADER

    echo "$prompt_file"
}

# ──────────────────────────────────────────────────────────────────────
# Execute one segment
# ──────────────────────────────────────────────────────────────────────
run_segment() {
    local seg_num="$1"
    local seg_file="$(seg_file "$seg_num")"
    local seg_log="$LOG_DIR/segment-${seg_num}-$(ts_file).log"
    local prompt_file

    prompt_file=$(build_segment_prompt "$seg_num")

    log "  $(cyan "▶ S${seg_num}") ${seg_file%.md} → $seg_log"
    set_segment_status "$seg_num" "running"

    claude \
        -p "$(cat "$prompt_file")" \
        --dangerously-skip-permissions \
        --verbose \
        < /dev/null \
        > "$seg_log" 2>&1 &
    local pid=$!

    # Watchdog
    local elapsed=0
    while kill -0 "$pid" 2>/dev/null; do
        sleep 10
        elapsed=$((elapsed + 10))
        if [[ $elapsed -ge $SESSION_TIMEOUT ]]; then
            log "  $(yellow "⏱ S${seg_num} timed out after ${SESSION_TIMEOUT}s, killing")"
            kill "$pid" 2>/dev/null; sleep 2; kill -9 "$pid" 2>/dev/null
            break
        fi
    done

    wait "$pid" 2>/dev/null
    local exit_code=$?

    # Parse result from log
    local result="unknown"
    if [[ -f "$seg_log" ]] && [[ -s "$seg_log" ]]; then
        if grep -q "Status.*PASS" "$seg_log" 2>/dev/null; then
            result="pass"
        elif grep -q "Status.*PARTIAL" "$seg_log" 2>/dev/null; then
            result="partial"
        elif grep -q "Status.*BLOCKED" "$seg_log" 2>/dev/null; then
            result="blocked"
        elif [[ $exit_code -eq 0 ]]; then
            result="pass"
        else
            result="failed"
        fi
    else
        result="failed"
    fi

    local lines=0
    [[ -f "$seg_log" ]] && lines=$(wc -l < "$seg_log")

    # Get segment title for notification
    local seg_title=""
    if [[ -f "$SEGMENTS_DIR/$seg_file" ]]; then
        seg_title=$(grep -m 1 "^# " "$SEGMENTS_DIR/$seg_file" 2>/dev/null | sed 's/^# //' || echo "${seg_file%.md}")
    fi

    case "$result" in
        pass)
            set_segment_status "$seg_num" "pass" "completed=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
            log "  $(green "✅ S${seg_num} PASS") ($lines lines output)"
            notify segment "S${seg_num}" "pass" "$seg_title"
            ;;
        partial)
            set_segment_status "$seg_num" "partial"
            log "  $(yellow "⚠ S${seg_num} PARTIAL") ($lines lines output)"
            notify segment "S${seg_num}" "partial" "$seg_title"
            ;;
        blocked)
            set_segment_status "$seg_num" "blocked"
            log "  $(red "❌ S${seg_num} BLOCKED") ($lines lines output)"
            notify segment "S${seg_num}" "blocked" "$seg_title"
            ;;
        *)
            set_segment_status "$seg_num" "failed" "exit_code=$exit_code"
            log "  $(red "💥 S${seg_num} FAILED") (exit=$exit_code, $lines lines output)"
            notify segment "S${seg_num}" "failed" "$seg_title"
            ;;
    esac

    echo "$result"
}

# ──────────────────────────────────────────────────────────────────────
# Execute a wave (parallel segments)
# ──────────────────────────────────────────────────────────────────────
run_wave() {
    local wave_num="$1"
    shift
    local segments=("$@")
    local to_run=()

    # Filter: only run pending/failed segments (skip pass/blocked)
    for seg in "${segments[@]}"; do
        local status
        status=$(get_segment_status "$seg")
        if [[ "$status" == "pass" ]]; then
            log "  $(green "⏭ S${seg} already passed, skipping")"
            continue
        fi
        local attempts
        attempts=$(get_attempts "$seg")
        if [[ "$attempts" -ge "$MAX_RETRIES_PER_SEGMENT" ]] && [[ "$status" == "blocked" || "$status" == "failed" ]]; then
            log "  $(red "⏭ S${seg} exhausted $MAX_RETRIES_PER_SEGMENT attempts, skipping")"
            continue
        fi
        to_run+=("$seg")
    done

    if [[ ${#to_run[@]} -eq 0 ]]; then
        log "  $(green "Wave $wave_num: nothing to run (all done or skipped)")"
        return 0
    fi

    log "$(bold "━━━ Wave $wave_num: ${#to_run[@]} segments ━━━")"

    # Launch in batches of MAX_PARALLEL
    local pids=()
    local seg_for_pid=()
    local result_files=()

    for seg in "${to_run[@]}"; do
        increment_attempts "$seg" > /dev/null

        # If we're at max parallel, wait for one to finish
        while [[ ${#pids[@]} -ge $MAX_PARALLEL ]]; do
            wait_for_any_pid
        done

        # Launch segment in background subshell that writes result to a temp file
        local result_file="/tmp/prb-result-${seg}"
        (
            result=$(run_segment "$seg")
            echo "$result" > "$result_file"
        ) &
        pids+=($!)
        seg_for_pid+=("$seg")
        result_files+=("$result_file")

        wait_for_network
    done

    # Wait for all remaining
    for pid in "${pids[@]}"; do
        wait "$pid" 2>/dev/null || true
    done

    # Collect results
    local all_pass=true
    local pass_count=0
    for i in "${!seg_for_pid[@]}"; do
        local seg="${seg_for_pid[$i]}"
        local rf="${result_files[$i]}"
        local result="unknown"
        [[ -f "$rf" ]] && result=$(cat "$rf") && rm -f "$rf"
        if [[ "$result" == "pass" ]]; then
            pass_count=$((pass_count + 1))
        else
            all_pass=false
        fi
    done

    local total_in_wave=${#to_run[@]}
    local wave_summary="${pass_count}/${total_in_wave} segments passed"

    if $all_pass; then
        log "$(green "  ✅ Wave $wave_num: ALL PASSED")"
        notify wave "$wave_num" "complete" "$wave_summary"
    else
        log "$(yellow "  ⚠ Wave $wave_num: some segments did not pass (see above)")"
        notify wave "$wave_num" "partial" "$wave_summary"
    fi
}

wait_for_any_pid() {
    wait -n 2>/dev/null || sleep 5
}

# ──────────────────────────────────────────────────────────────────────
# Integration gate after each wave
# ──────────────────────────────────────────────────────────────────────
run_gate() {
    local wave_num="$1"
    log "  🔒 Running integration gate after wave $wave_num..."
    local gate_log="$LOG_DIR/gate-wave${wave_num}-$(ts_file).log"

    {
        cargo build --workspace 2>&1 || true
        cargo clippy --workspace -- -D warnings 2>&1 || true
    } > "$gate_log" 2>&1

    if grep -q "^error" "$gate_log" 2>/dev/null; then
        log "  $(yellow "⚠ Gate: build warnings/errors detected (see $gate_log)")"
    else
        log "  $(green "✅ Gate: workspace builds clean")"
    fi
}

# ──────────────────────────────────────────────────────────────────────
# CLI: --status
# ──────────────────────────────────────────────────────────────────────
if [[ "${1:-}" == "--status" ]]; then
    echo ""
    bold "Phase 2 Orchestration Status"
    echo ""
    progress_summary
    echo ""
    exit 0
fi

# ──────────────────────────────────────────────────────────────────────
# CLI: --dry-run
# ──────────────────────────────────────────────────────────────────────
if [[ "${1:-}" == "--dry-run" ]]; then
    echo ""
    bold "Phase 2 — Dry Run"
    echo ""
    for i in "${!ALL_WAVES[@]}"; do
        wave_num=$((i + 1))
        echo "  Wave $wave_num: ${ALL_WAVES[$i]}"
    done
    echo ""
    echo "  Max parallel: $MAX_PARALLEL"
    echo "  Timeout/segment: ${SESSION_TIMEOUT}s"
    echo "  Retries/segment: $MAX_RETRIES_PER_SEGMENT"
    echo ""
    progress_summary
    exit 0
fi

# ──────────────────────────────────────────────────────────────────────
# Pre-flight
# ──────────────────────────────────────────────────────────────────────
if ! command -v claude &>/dev/null; then
    log "$(red "❌ 'claude' not in PATH")"
    exit 1
fi
if [[ -z "${CUI_BEDROCK_API_KEY:-}" ]]; then
    log "$(red "❌ CUI_BEDROCK_API_KEY not set")"
    exit 1
fi
if ! command -v python3 &>/dev/null; then
    log "$(red "❌ python3 not in PATH")"
    exit 1
fi
if [[ ! -d "$SEGMENTS_DIR" ]]; then
    log "$(red "❌ Segments dir not found: $SEGMENTS_DIR")"
    exit 1
fi

setup_bedrock_env
mkdir -p "$LOG_DIR"
init_state

MASTER_LOG="$LOG_DIR/master-$(ts_file).log"

# ──────────────────────────────────────────────────────────────────────
# Banner
# ──────────────────────────────────────────────────────────────────────
{
echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║   $(bold 'Phase 2 — Overnight Orchestration')                            ║"
echo "║   Script-driven: 1 claude call per segment, ${MAX_PARALLEL} parallel max     ║"
echo "║   7 waves, 29 segments, ${SESSION_TIMEOUT}s timeout/segment                ║"
echo "║   Master log: $MASTER_LOG ║"
echo "║   $(cyan 'Ctrl+C to stop after current wave')                            ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""
progress_summary
echo ""
} 2>&1 | tee -a "$MASTER_LOG"

# ──────────────────────────────────────────────────────────────────────
# Graceful stop
# ──────────────────────────────────────────────────────────────────────
STOP_REQUESTED=false
trap 'STOP_REQUESTED=true; log "$(yellow "⚡ Stop requested — finishing current wave")"; ' INT

# ──────────────────────────────────────────────────────────────────────
# Main: execute waves sequentially, segments in parallel within wave
# ──────────────────────────────────────────────────────────────────────
start_time=$(date +%s)

for i in "${!ALL_WAVES[@]}"; do
    wave_num=$((i + 1))
    # shellcheck disable=SC2206
    wave_segments=(${ALL_WAVES[$i]})

    if [[ "$STOP_REQUESTED" == "true" ]]; then
        log "$(yellow "Stopping. Run again to resume.")"
        break
    fi

    echo "" | tee -a "$MASTER_LOG"
    log "$(bold "══════════════════════════════════════════════════")" | tee -a "$MASTER_LOG"
    log "$(bold "  WAVE $wave_num / 7  —  Segments: ${ALL_WAVES[$i]}")" | tee -a "$MASTER_LOG"
    log "$(bold "══════════════════════════════════════════════════")" | tee -a "$MASTER_LOG"

    wait_for_network
    run_wave "$wave_num" "${wave_segments[@]}" 2>&1 | tee -a "$MASTER_LOG"

    run_gate "$wave_num" 2>&1 | tee -a "$MASTER_LOG"

    echo "" | tee -a "$MASTER_LOG"
    progress_summary | tee -a "$MASTER_LOG"
    echo "" | tee -a "$MASTER_LOG"
done

# ──────────────────────────────────────────────────────────────────────
# Final summary
# ──────────────────────────────────────────────────────────────────────
elapsed=$(( $(date +%s) - start_time ))
hours=$((elapsed / 3600))
mins=$(( (elapsed % 3600) / 60 ))

# Calculate final stats for notification
final_stats=$(python3 -c "
import json
with open('$STATE_FILE') as f:
    state = json.load(f)
segs = state.get('segments', {})
total = len(segs)
passed = sum(1 for v in segs.values() if v.get('status') == 'pass')
blocked = sum(1 for v in segs.values() if 'block' in v.get('status', ''))
failed = sum(1 for v in segs.values() if v.get('status') == 'failed')
partial = sum(1 for v in segs.values() if v.get('status') == 'partial')
print(f'{passed}/{total} passed, {blocked} blocked, {failed} failed, {partial} partial')
" 2>/dev/null || echo "stats unavailable")

# Determine overall status
final_status="complete"
if [[ "$STOP_REQUESTED" == "true" ]]; then
    final_status="stopped"
elif echo "$final_stats" | grep -qE "(blocked|failed|partial)"; then
    final_status="partial"
fi

{
echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║   $(bold 'Orchestration Complete')                                        ║"
echo "║   Elapsed: ${hours}h ${mins}m                                              ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""
progress_summary
echo ""
} 2>&1 | tee -a "$MASTER_LOG"

# Send final notification
notify final "$final_status" "${hours}h ${mins}m" "$final_stats"
