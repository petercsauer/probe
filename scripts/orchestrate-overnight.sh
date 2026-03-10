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
PLAN_DIR=".claude/plans/phase2-coverage-hardening"
STATE_FILE="$PLAN_DIR/execution-state.json"
SEGMENTS_DIR="$PLAN_DIR/segments"
BUILDER_SKILL=".claude/commands/iterative-builder.md"
LOG_DIR="logs/phase2-orchestrate"
NOTIFY_SCRIPT="./scripts/notify-imessage.sh"

MAX_RETRIES_PER_SEGMENT=2
MAX_PARALLEL=4
SESSION_TIMEOUT=3600     # 60 min per segment (fresh CARGO_TARGET_DIR needs compile time)
RETRY_DELAY=10
NETWORK_RETRY_MAX=600    # 10 min max waiting for network

# Enable/disable notifications (set to 0 to disable)
ENABLE_NOTIFICATIONS="${PRB_NOTIFY:-1}"
export PRB_NOTIFY_CONTACT="${PRB_NOTIFY_CONTACT:-+12036446182}"

# Wave definitions: each wave is a space-separated list of segment numbers
# Derived from the manifest dependency graph
# Wave 1: All 10 unit test segments (no deps, all parallel)
WAVE_1="01 02 03 04 05 06 07 08 09 10"
# Wave 2: Cross-crate integration (depends on 1,2,4,5,6,9,10)
WAVE_2="11"
# Wave 3: CLI e2e + real-data segments that only need S11 (depends on wave 1-2)
WAVE_3="12 13 14 15 16 17 18"
# Wave 4: Real-data segments that depend on wave 3 outputs
WAVE_4="19 20 21 22"
# Wave 5: Depends on S13 + S22
WAVE_5="23"
# Wave 6: Final regression suite (depends on all 13-23)
WAVE_6="24"
ALL_WAVES=("$WAVE_1" "$WAVE_2" "$WAVE_3" "$WAVE_4" "$WAVE_5" "$WAVE_6")

# Segment number → filename (function avoids bash octal issue with 08/09)
seg_file() {
    local files
    files=(
        "01-core-coverage.md"
        "02-tui-coverage.md"
        "03-cli-coverage.md"
        "04-pcap-coverage.md"
        "05-grpc-coverage.md"
        "06-capture-coverage.md"
        "07-ai-coverage.md"
        "08-plugin-coverage.md"
        "09-decode-coverage.md"
        "10-misc-coverage.md"
        "11-integration-tests.md"
        "12-cli-e2e-tests.md"
        "13-real-data-grpc-http2.md"
        "14-real-data-tls.md"
        "15-real-data-tcp-ip.md"
        "16-real-data-dns-dhcp.md"
        "17-real-data-http1-websocket.md"
        "18-real-data-smb-rdp.md"
        "19-real-data-rtps-dds-mqtt.md"
        "20-real-data-quic-ssh.md"
        "21-real-data-malicious.md"
        "22-real-data-conversation-export.md"
        "23-real-data-otel-correlation.md"
        "24-real-data-e2e-regression.md"
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

# Extract a brief summary from a segment log for notifications
extract_log_summary() {
    local log_file="$1"
    [[ ! -f "$log_file" ]] || [[ ! -s "$log_file" ]] && echo "(no output)" && return

    python3 -c "
import re, sys

log = open('$log_file', errors='replace').read()

parts = []

# Cycles used
m = re.search(r'Cycles used:\*?\*?\s*(\d+\s*/\s*\d+)', log)
if m: parts.append(f'Cycles: {m.group(1)}')

# Final phase
m = re.search(r'Final phase reached:\*?\*?\s*(.+)', log)
if m: parts.append(f'Phase: {m.group(1).strip()}')

# Test counts
m = re.search(r'Tests:\*?\*?\s*(.+)', log)
if m: parts.append(f'Tests: {m.group(1).strip()}')

# If BLOCKED/PARTIAL, grab the stuck-on line
m = re.search(r'Stuck on:\*?\*?\s*(.+)', log)
if m: parts.append(f'Stuck: {m.group(1).strip()[:80]}')

# WIP commits
m = re.search(r'WIP commits:\*?\*?\s*(\d+)', log)
if m and int(m.group(1)) > 0: parts.append(f'Commits: {m.group(1)}')

if parts:
    print(chr(10).join(parts))
else:
    # Fallback: grab last meaningful line
    lines = [l.strip() for l in log.splitlines() if l.strip() and not l.startswith('[')]
    if lines:
        print(lines[-1][:120])
    else:
        print('(no parseable summary)')
" 2>/dev/null || echo "(summary extraction failed)"
}

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
# State management (all operations go through locked Python helper)
# ──────────────────────────────────────────────────────────────────────
STATE_LOCK="${STATE_FILE}.lock"

# All state operations funnel through this single locked Python script.
# Usage: state_op <command> [args...]
#   init                          — create state file if missing
#   get_status <seg>              — print segment status
#   set_status <seg> <status> [extra_kv]  — set segment status
#   inc_attempts <seg>            — increment attempts, print new count
#   get_attempts <seg>            — print attempt count
#   summary                       — print progress summary
state_op() {
    local cmd="$1"; shift
    python3 -c "
import fcntl, json, os, sys

state_file = '$STATE_FILE'
lock_file  = '$STATE_LOCK'
cmd = '$cmd'
args = sys.argv[1:]

SEGMENTS_INIT = {
    '01': ('Core', 'prb-core Unit Tests'),
    '02': ('TUI', 'prb-tui Unit + Render Tests'),
    '03': ('CLI', 'prb-cli Command Tests'),
    '04': ('Pcap', 'prb-pcap Pipeline Tests'),
    '05': ('gRPC', 'prb-grpc H2 Parser Tests'),
    '06': ('Capture', 'prb-capture Mock Tests'),
    '07': ('AI', 'prb-ai Explain Tests'),
    '08': ('Plugin', 'prb-plugin Test Harness'),
    '09': ('Decode', 'prb-decode Codec Tests'),
    '10': ('Misc', 'prb-detect + export + misc'),
    '11': ('Integration', 'Cross-Crate Integration Tests'),
    '12': ('CLI', 'CLI End-to-End Tests'),
    '13': ('RealData', 'Real-Data: gRPC/HTTP2'),
    '14': ('RealData', 'Real-Data: TLS'),
    '15': ('RealData', 'Real-Data: TCP/IP'),
    '16': ('RealData', 'Real-Data: DNS/DHCP'),
    '17': ('RealData', 'Real-Data: HTTP1/WebSocket'),
    '18': ('RealData', 'Real-Data: SMB/RDP'),
    '19': ('RealData', 'Real-Data: RTPS/DDS/MQTT'),
    '20': ('RealData', 'Real-Data: QUIC/SSH'),
    '21': ('RealData', 'Real-Data: Malicious'),
    '22': ('RealData', 'Real-Data: Conversation Export'),
    '23': ('RealData', 'Real-Data: OTel Correlation'),
    '24': ('RealData', 'Real-Data: E2E Regression'),
}

os.makedirs(os.path.dirname(lock_file) or '.', exist_ok=True)
lf = open(lock_file, 'w')
fcntl.flock(lf, fcntl.LOCK_EX)

try:
    def read_state():
        if not os.path.exists(state_file) or os.path.getsize(state_file) == 0:
            return None
        with open(state_file) as f:
            return json.load(f)

    def write_state(state):
        tmp = state_file + '.tmp'
        with open(tmp, 'w') as f:
            json.dump(state, f, indent=2)
        os.replace(tmp, state_file)

    if cmd == 'init':
        if read_state() is not None:
            print('State file already exists')
        else:
            from datetime import datetime, timezone
            segments = {}
            for num, (track, title) in SEGMENTS_INIT.items():
                segments[f'S{num}'] = {'status': 'pending', 'track': track, 'title': title, 'attempts': 0}
            state = {
                'plan': '$PLAN_DIR/manifest.md',
                'started': datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
                'segments': segments,
            }
            write_state(state)
            print('State file created')

    elif cmd == 'get_status':
        seg_num = args[0]
        state = read_state()
        if state is None:
            print('unknown')
        else:
            print(state.get('segments', {}).get(f'S{seg_num}', {}).get('status', 'unknown'))

    elif cmd == 'set_status':
        seg_num, status = args[0], args[1]
        extra = args[2] if len(args) > 2 else ''
        state = read_state()
        if state is None:
            print('error: no state file', file=sys.stderr); sys.exit(1)
        seg = state['segments'].setdefault(f'S{seg_num}', {})
        seg['status'] = status
        if extra:
            for kv in extra.split(','):
                if '=' in kv:
                    k, v = kv.split('=', 1)
                    seg[k.strip()] = v.strip()
        write_state(state)

    elif cmd == 'inc_attempts':
        seg_num = args[0]
        state = read_state()
        if state is None:
            print('0'); sys.exit(0)
        seg = state['segments'].get(f'S{seg_num}', {})
        seg['attempts'] = seg.get('attempts', 0) + 1
        write_state(state)
        print(seg['attempts'])

    elif cmd == 'get_attempts':
        seg_num = args[0]
        state = read_state()
        if state is None:
            print('0')
        else:
            print(state.get('segments', {}).get(f'S{seg_num}', {}).get('attempts', 0))

    elif cmd == 'summary':
        state = read_state()
        if state is None:
            print('  No state file — fresh start'); sys.exit(0)
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
        bar = chr(9608) * bar_done + chr(9617) * bar_left
        print(f'  [{bar}] {pct}%  ({done}/{total} segments)')
        print(f'  pass:{done} | pending:{pending} | running:{running} | blocked:{blocked} | failed:{failed}')
        passed = [k for k, v in segs.items() if v.get('status') == 'pass']
        if passed:
            print(f'  Completed: {\" \".join(sorted(passed))}')
    else:
        print(f'Unknown command: {cmd}', file=sys.stderr)
        sys.exit(1)
finally:
    fcntl.flock(lf, fcntl.LOCK_UN)
    lf.close()
" "$@" 2>/dev/null
}

# Convenience wrappers
init_state()           { state_op init; }
get_segment_status()   { state_op get_status "$1" || echo "unknown"; }
set_segment_status()   { state_op set_status "$1" "$2" "${3:-}"; }
increment_attempts()   { state_op inc_attempts "$1"; }
get_attempts()         { state_op get_attempts "$1" || echo "0"; }
progress_summary()     { state_op summary || echo "  (unable to parse state)"; }

# ──────────────────────────────────────────────────────────────────────
# Network check
# ──────────────────────────────────────────────────────────────────────
wait_for_network() {
    local waited=0
    local delay=10
    local notified=false
    while ! curl -s --max-time 5 https://api.anthropic.com >/dev/null 2>&1; do
        waited=$((waited + delay))
        if [[ $waited -ge $NETWORK_RETRY_MAX ]]; then
            log "$(red "Network unreachable for ${NETWORK_RETRY_MAX}s, continuing anyway...")"
            return
        fi
        # Notify once after 60s of downtime
        if [[ "$notified" == "false" ]] && [[ $waited -ge 60 ]]; then
            notify network "$waited"
            notified=true
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
- CARGO_TARGET_DIR is set to \`/tmp/prb-target-S${seg_num}\` in your environment. All cargo commands will use this automatically — do NOT override it. This isolates your builds from other parallel segments.

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

    local seg_target_dir="/tmp/prb-target-S${seg_num}"
    mkdir -p "$seg_target_dir"

    log "  $(cyan "▶ S${seg_num}") ${seg_file%.md} → $seg_log  [target: $seg_target_dir]"
    set_segment_status "$seg_num" "running"

    local seg_log_raw="${seg_log%.log}.stream.jsonl"

    CARGO_TARGET_DIR="$seg_target_dir" claude \
        -p "$(cat "$prompt_file")" \
        --dangerously-skip-permissions \
        --verbose \
        --output-format stream-json \
        < /dev/null \
        > "$seg_log_raw" 2>&1 &
    local pid=$!

    # Watchdog
    local elapsed=0
    while kill -0 "$pid" 2>/dev/null; do
        sleep 10
        elapsed=$((elapsed + 10))
        if [[ $elapsed -ge $SESSION_TIMEOUT ]]; then
            log "  $(yellow "⏱ S${seg_num} timed out after ${SESSION_TIMEOUT}s, killing")"
            kill "$pid" 2>/dev/null; sleep 2; kill -9 "$pid" 2>/dev/null
            local timeout_summary
            timeout_summary=$(extract_log_summary "$seg_log")
            notify segment "S${seg_num}" "timeout" "${seg_file%.md}" "Killed after ${SESSION_TIMEOUT}s
$timeout_summary"
            break
        fi
    done

    wait "$pid" 2>/dev/null
    local exit_code=$?

    # Convert stream JSONL to readable log
    python3 -c "
import json, sys
for line in open('$seg_log_raw', errors='replace'):
    line = line.strip()
    if not line: continue
    try: obj = json.loads(line)
    except: continue
    t = obj.get('type', '')
    if t == 'assistant' and 'message' in obj:
        for block in obj['message'].get('content', []):
            if block.get('type') == 'text':
                print(block['text'])
            elif block.get('type') == 'tool_use':
                print(f'--- tool: {block.get(\"name\",\"\")} ---')
    elif t == 'result':
        txt = obj.get('result', '')
        if txt: print(txt)
" > "$seg_log" 2>/dev/null

    # Parse result from both readable log and raw stream
    local result="unknown"
    local check_files=("$seg_log" "$seg_log_raw")
    for cf in "${check_files[@]}"; do
        [[ -f "$cf" ]] && [[ -s "$cf" ]] || continue
        if grep -q "Status.*PASS" "$cf" 2>/dev/null; then
            result="pass"; break
        elif grep -q "Status.*PARTIAL" "$cf" 2>/dev/null; then
            result="partial"; break
        elif grep -q "Status.*BLOCKED" "$cf" 2>/dev/null; then
            result="blocked"; break
        fi
    done
    if [[ "$result" == "unknown" ]]; then
        if [[ $exit_code -eq 0 ]]; then
            result="pass"
        else
            result="failed"
        fi
    fi

    local lines=0
    [[ -f "$seg_log" ]] && lines=$(wc -l < "$seg_log")

    # Get segment title and build summary for notification
    local seg_title=""
    if [[ -f "$SEGMENTS_DIR/$seg_file" ]]; then
        seg_title=$(grep -m 1 "^# " "$SEGMENTS_DIR/$seg_file" 2>/dev/null | sed 's/^# //' || echo "${seg_file%.md}")
    fi
    local summary
    summary=$(extract_log_summary "$seg_log")

    case "$result" in
        pass)
            set_segment_status "$seg_num" "pass" "completed=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
            log "  $(green "✅ S${seg_num} PASS") ($lines lines output)"
            notify segment "S${seg_num}" "pass" "$seg_title" "$summary"
            ;;
        partial)
            set_segment_status "$seg_num" "partial"
            log "  $(yellow "⚠ S${seg_num} PARTIAL") ($lines lines output)"
            notify segment "S${seg_num}" "partial" "$seg_title" "$summary"
            ;;
        blocked)
            set_segment_status "$seg_num" "blocked"
            log "  $(red "❌ S${seg_num} BLOCKED") ($lines lines output)"
            notify segment "S${seg_num}" "blocked" "$seg_title" "$summary"
            ;;
        *)
            set_segment_status "$seg_num" "failed" "exit_code=$exit_code"
            log "  $(red "💥 S${seg_num} FAILED") (exit=$exit_code, $lines lines output)"
            notify segment "S${seg_num}" "failed" "$seg_title" "$summary"
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

    notify wave-start "$wave_num" "${#ALL_WAVES[@]}" "${to_run[*]}" "${#to_run[@]}"

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
        local error_count
        error_count=$(grep -c "^error" "$gate_log" 2>/dev/null || echo "?")
        local first_errors
        first_errors=$(grep "^error" "$gate_log" 2>/dev/null | head -3 | tr '\n' '; ')
        log "  $(yellow "⚠ Gate: build warnings/errors detected (see $gate_log)")"
        notify gate "$wave_num" "fail" "Build gate FAILED after wave $wave_num
${error_count} errors: ${first_errors}"
    else
        log "  $(green "✅ Gate: workspace builds clean")"
        notify gate "$wave_num" "pass" "Workspace builds clean after wave $wave_num"
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
echo "║   $(bold 'Phase 2 — Coverage Hardening Orchestration')                    ║"
echo "║   Script-driven: 1 claude call per segment, ${MAX_PARALLEL} parallel max     ║"
echo "║   6 waves, 24 segments, ${SESSION_TIMEOUT}s timeout/segment                ║"
echo "║   Master log: $MASTER_LOG ║"
echo "║   $(cyan 'Ctrl+C to stop after current wave')                            ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""
progress_summary
echo ""
} 2>&1 | tee -a "$MASTER_LOG"

notify started "24" "${#ALL_WAVES[@]}" "$MAX_PARALLEL"

# ──────────────────────────────────────────────────────────────────────
# Heartbeat: every 15 min, summarize running segments via claude and notify
# ──────────────────────────────────────────────────────────────────────
HEARTBEAT_INTERVAL=900  # 15 minutes

heartbeat_loop() {
    sleep "$HEARTBEAT_INTERVAL"
    while true; do
        # Collect the last ~50 events from each running segment's JSONL
        local snapshot=""
        local running_segs=0
        for jsonl in "$LOG_DIR"/segment-*-*.stream.jsonl; do
            [[ -f "$jsonl" ]] || continue
            local seg_id
            seg_id=$(basename "$jsonl" | grep -o 'segment-[0-9]*' | grep -o '[0-9]*')
            # Only include segments still actively writing (modified in last 5 min)
            if [[ $(find "$jsonl" -mmin -5 2>/dev/null) ]]; then
                running_segs=$((running_segs + 1))
                local recent
                recent=$(tail -30 "$jsonl" | python3 -c "
import sys, json
lines = []
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    try: obj = json.loads(line)
    except: continue
    t = obj.get('type', '')
    if t == 'assistant' and 'message' in obj:
        for b in obj['message'].get('content', []):
            if b.get('type') == 'text' and b['text'].strip():
                lines.append(b['text'].strip()[:200])
            elif b.get('type') == 'tool_use':
                lines.append(f'[tool: {b.get(\"name\",\"\")}]')
for l in lines[-8:]:
    print(l)
" 2>/dev/null)
                if [[ -n "$recent" ]]; then
                    snapshot+="
=== S${seg_id} (active) ===
${recent}
"
                fi
            fi
        done

        if [[ $running_segs -eq 0 ]]; then
            sleep "$HEARTBEAT_INTERVAL"
            continue
        fi

        # Get overall progress
        local progress
        progress=$(progress_summary 2>/dev/null)

        # Ask claude to summarize
        local summary
        summary=$(CARGO_TARGET_DIR="/tmp/prb-heartbeat" claude -p "You are a build monitor. Summarize the status of this overnight orchestration in 3-5 SHORT lines for an iMessage notification. Be specific about what each segment is doing and how far along it is. No markdown, no headers, just plain text lines.

Overall progress:
${progress}

Recent activity from ${running_segs} running segments:
${snapshot}" \
            --output-format text \
            --max-turns 1 \
            < /dev/null 2>/dev/null | head -8)

        if [[ -n "$summary" ]]; then
            notify heartbeat "$summary"
            log "  💓 Heartbeat sent"
        fi

        sleep "$HEARTBEAT_INTERVAL"
    done
}

heartbeat_loop &
HEARTBEAT_PID=$!

# ──────────────────────────────────────────────────────────────────────
# Graceful stop
# ──────────────────────────────────────────────────────────────────────
STOP_REQUESTED=false
cleanup_heartbeat() {
    kill "$HEARTBEAT_PID" 2>/dev/null
    wait "$HEARTBEAT_PID" 2>/dev/null
}
trap 'STOP_REQUESTED=true; log "$(yellow "⚡ Stop requested — finishing current wave")"; cleanup_heartbeat' INT
trap 'cleanup_heartbeat' EXIT

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
    log "$(bold "  WAVE $wave_num / ${#ALL_WAVES[@]}  —  Segments: ${ALL_WAVES[$i]}")" | tee -a "$MASTER_LOG"
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
