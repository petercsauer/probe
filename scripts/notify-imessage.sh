#!/usr/bin/env bash
#
# iMessage Notification Helper for PRB Orchestration
#
# Usage:
#   ./notify-imessage.sh segment S01 pass "Query Language Engine"
#   ./notify-imessage.sh wave 1 complete "10/10 segments passed"
#   ./notify-imessage.sh final complete "2h 15m" "25/29 passed"
#
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────
# Configuration
# ──────────────────────────────────────────────────────────────────────

# Get the user's own phone number/iMessage handle from Contacts
# Falls back to environment variable if set
IMESSAGE_RECIPIENT="${PRB_NOTIFY_CONTACT:-$(osascript -e 'tell application "Contacts" to get value of phone 1 of person 1 whose it is me' 2>/dev/null || echo "")}"

# If still empty, try getting email instead
if [[ -z "$IMESSAGE_RECIPIENT" ]]; then
    IMESSAGE_RECIPIENT=$(osascript -e 'tell application "Contacts" to get value of email 1 of person 1 whose it is me' 2>/dev/null || echo "")
fi

# If STILL empty, error out
if [[ -z "$IMESSAGE_RECIPIENT" ]]; then
    echo "ERROR: Cannot determine iMessage recipient. Set PRB_NOTIFY_CONTACT environment variable." >&2
    exit 1
fi

STATE_FILE="${PRB_STATE_FILE:-.claude/plans/phase2-coverage-hardening/execution-state.json}"

# ──────────────────────────────────────────────────────────────────────
# Helper: Send iMessage
# ──────────────────────────────────────────────────────────────────────
send_imessage() {
    local message="$1"

    # Try to send via Messages app
    if ! osascript -e "tell application \"Messages\" to send \"$message\" to buddy \"$IMESSAGE_RECIPIENT\"" 2>/dev/null; then
        echo "WARNING: Failed to send iMessage to $IMESSAGE_RECIPIENT" >&2
        echo "Message was: $message" >&2
        return 1
    fi

    echo "✓ Sent iMessage to $IMESSAGE_RECIPIENT"
    return 0
}

# ──────────────────────────────────────────────────────────────────────
# Get current progress stats
# ──────────────────────────────────────────────────────────────────────
get_stats() {
    python3 -c "
import json, os
if not os.path.exists('$STATE_FILE'):
    print('0/0')
    exit(0)
with open('$STATE_FILE') as f:
    state = json.load(f)
segs = state.get('segments', {})
total = len(segs)
done = sum(1 for v in segs.values() if v.get('status') == 'pass')
print(f'{done}/{total}')
" 2>/dev/null || echo "?/?"
}

# ──────────────────────────────────────────────────────────────────────
# Notification types
# ──────────────────────────────────────────────────────────────────────

notify_started() {
    local total_segments="$1"
    local total_waves="$2"
    local max_parallel="$3"

    local message="PRB 🚀 Orchestration Started

$total_segments segments across $total_waves waves
Max parallel: $max_parallel
Timeout: 30min/segment

Let's go."

    send_imessage "$message"
}

notify_segment() {
    local seg_id="$1"
    local status="$2"
    local title="$3"
    local log_summary="${4:-}"

    local emoji
    case "$status" in
        pass)     emoji="✅" ;;
        partial)  emoji="⚠️" ;;
        blocked)  emoji="❌" ;;
        failed)   emoji="💥" ;;
        timeout)  emoji="⏱️" ;;
        *)        emoji="❓" ;;
    esac

    local stats
    stats=$(get_stats)

    local status_upper
    status_upper=$(echo "$status" | tr '[:lower:]' '[:upper:]')

    local message="PRB $emoji $seg_id $status_upper
$title

$log_summary

Progress: $stats segments"

    send_imessage "$message"
}

notify_wave_start() {
    local wave_num="$1"
    local total_waves="$2"
    local segment_list="$3"
    local segment_count="$4"

    local stats
    stats=$(get_stats)

    local message="PRB 🌊 Wave $wave_num/$total_waves Starting

Segments: $segment_list
($segment_count running in parallel)

Progress so far: $stats"

    send_imessage "$message"
}

notify_wave() {
    local wave_num="$1"
    local status="$2"
    local summary="$3"

    local emoji
    [[ "$status" == "complete" ]] && emoji="✅" || emoji="⚠️"

    local stats
    stats=$(get_stats)

    local message="PRB $emoji Wave $wave_num Complete
$summary

Total Progress: $stats segments"

    send_imessage "$message"
}

notify_gate() {
    local wave_num="$1"
    local status="$2"
    local details="${3:-}"

    local emoji
    [[ "$status" == "pass" ]] && emoji="✅" || emoji="🚨"

    local message="PRB $emoji Build Gate (Wave $wave_num)
$details"

    send_imessage "$message"
}

notify_network() {
    local waited="$1"

    local message="PRB 📡 Network Stall

API unreachable for ${waited}s and counting.
Orchestration paused until connectivity returns."

    send_imessage "$message"
}

notify_heartbeat() {
    local summary="$1"

    local stats
    stats=$(get_stats)

    local message="PRB 💓 Status Update

$summary

Progress: $stats segments"

    send_imessage "$message"
}

notify_final() {
    local status="$1"
    local elapsed="$2"
    local summary="$3"

    local emoji
    case "$status" in
        complete) emoji="🎉" ;;
        partial)  emoji="⚠️" ;;
        stopped)  emoji="⏸️" ;;
        *)        emoji="🏁" ;;
    esac

    local stats
    stats=$(get_stats)

    local final_status_upper
    final_status_upper=$(echo "$status" | tr '[:lower:]' '[:upper:]')

    local message="PRB $emoji ALL DONE — $final_status_upper

Elapsed: $elapsed
Result: $summary

Final: $stats segments complete"

    send_imessage "$message"
}

# ──────────────────────────────────────────────────────────────────────
# CLI
# ──────────────────────────────────────────────────────────────────────
cmd="${1:-}"

case "$cmd" in
    started)
        notify_started "$2" "$3" "$4"
        ;;
    segment)
        notify_segment "$2" "$3" "${4:-}" "${5:-}"
        ;;
    wave-start)
        notify_wave_start "$2" "$3" "$4" "$5"
        ;;
    wave)
        notify_wave "$2" "$3" "$4"
        ;;
    heartbeat)
        notify_heartbeat "$2"
        ;;
    gate)
        notify_gate "$2" "$3" "${4:-}"
        ;;
    network)
        notify_network "$2"
        ;;
    final)
        notify_final "$2" "$3" "$4"
        ;;
    test)
        send_imessage "PRB orchestration notifications are working! 🚀"
        ;;
    *)
        echo "Usage: $0 {started|segment|wave-start|wave|gate|network|final|test} [args...]" >&2
        exit 1
        ;;
esac
