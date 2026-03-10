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

STATE_FILE="${PRB_STATE_FILE:-.claude/plans/phase2-orchestrated/execution-state.json}"

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

notify_segment() {
    local seg_id="$1"
    local status="$2"
    local title="$3"

    local emoji
    case "$status" in
        pass)     emoji="✅" ;;
        partial)  emoji="⚠️" ;;
        blocked)  emoji="❌" ;;
        failed)   emoji="💥" ;;
        *)        emoji="❓" ;;
    esac

    local stats
    stats=$(get_stats)

    local message="PRB Build $emoji

Segment: $seg_id
Status: ${status^^}
Title: $title

Progress: $stats segments"

    send_imessage "$message"
}

notify_wave() {
    local wave_num="$1"
    local status="$2"    # "complete" or "partial"
    local summary="$3"   # e.g., "8/10 segments passed"

    local emoji="🌊"
    [[ "$status" == "complete" ]] && emoji="✅" || emoji="⚠️"

    local stats
    stats=$(get_stats)

    local message="PRB Build $emoji

Wave $wave_num Complete
$summary

Total Progress: $stats segments"

    send_imessage "$message"
}

notify_final() {
    local status="$1"      # "complete", "partial", or "stopped"
    local elapsed="$2"     # e.g., "2h 15m"
    local summary="$3"     # e.g., "25/29 passed, 2 blocked, 2 failed"

    local emoji
    case "$status" in
        complete) emoji="🎉" ;;
        partial)  emoji="⚠️" ;;
        stopped)  emoji="⏸️" ;;
        *)        emoji="🏁" ;;
    esac

    local stats
    stats=$(get_stats)

    local message="PRB Build $emoji ${status^^}

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
    segment)
        notify_segment "$2" "$3" "$4"
        ;;
    wave)
        notify_wave "$2" "$3" "$4"
        ;;
    final)
        notify_final "$2" "$3" "$4"
        ;;
    test)
        send_imessage "PRB orchestration notifications are working! 🚀"
        ;;
    *)
        echo "Usage: $0 {segment|wave|final|test} [args...]" >&2
        echo "" >&2
        echo "Examples:" >&2
        echo "  $0 segment S01 pass \"Query Language\"" >&2
        echo "  $0 wave 1 complete \"10/10 passed\"" >&2
        echo "  $0 final complete \"2h 15m\" \"25/29 passed\"" >&2
        echo "  $0 test" >&2
        exit 1
        ;;
esac
