# PRB Orchestration Notifications

Automatic iMessage notifications for overnight orchestration runs.

## Setup

The notification system is **already integrated** into `orchestrate-overnight.sh`.

### Requirements

- macOS with Messages app
- Your "Me" contact card configured in Contacts.app (with phone number or email)

### Configuration

**No configuration needed!** The script auto-detects your contact info.

**Optional:** Set a custom recipient:
```bash
export PRB_NOTIFY_CONTACT="+1234567890"
# or
export PRB_NOTIFY_CONTACT="your@email.com"
```

**Disable notifications:**
```bash
export PRB_NOTIFY=0
./scripts/orchestrate-overnight.sh
```

## What You'll Receive

### 1. **Segment Notifications** (after each segment)
```
PRB Build ✅

Segment: S01
Status: PASS
Title: Query Language Engine

Progress: 1/29 segments
```

Emojis by status:
- ✅ PASS - segment completed successfully
- ⚠️ PARTIAL - segment partially completed
- ❌ BLOCKED - segment blocked
- 💥 FAILED - segment failed

### 2. **Wave Notifications** (after each wave)
```
PRB Build ✅

Wave 1 Complete
10/10 segments passed

Total Progress: 10/29 segments
```

### 3. **Final Notification** (when orchestration completes)
```
PRB Build 🎉 COMPLETE

Elapsed: 2h 15m
Result: 25/29 passed, 2 blocked, 2 failed

Final: 25/29 segments complete
```

## Testing

Test the notification system:
```bash
./scripts/notify-imessage.sh test
```

You should receive:
```
PRB orchestration notifications are working! 🚀
```

## Manual Usage

You can also send notifications manually:

```bash
# Segment notification
./scripts/notify-imessage.sh segment S01 pass "Query Language Engine"

# Wave notification
./scripts/notify-imessage.sh wave 1 complete "10/10 passed"

# Final notification
./scripts/notify-imessage.sh final complete "2h 15m" "25/29 passed"
```

## Troubleshooting

**"ERROR: Cannot determine iMessage recipient"**
- Open Contacts.app → find your "Me" card → add phone number or email
- Or set `PRB_NOTIFY_CONTACT` environment variable

**Messages app not sending**
- Ensure Messages.app is running and signed in
- Check System Settings → Messages → iMessage is enabled
- Try sending a manual message first

**Notifications not appearing**
- Check `PRB_NOTIFY` is not set to `0`
- Verify script is executable: `chmod +x scripts/notify-imessage.sh`
- Check logs in `logs/phase2-orchestrate/` for errors

**Too many notifications?**
- Set `PRB_NOTIFY=0` to disable
- Or modify `ENABLE_NOTIFICATIONS` in `orchestrate-overnight.sh`

## Implementation Details

- **Notification script:** `scripts/notify-imessage.sh`
- **Integration points:**
  - After each segment completes (`run_segment` function)
  - After each wave completes (`run_wave` function)
  - After final orchestration completes
- **AppleScript backend:** Uses macOS Messages.app via `osascript`
- **Non-blocking:** Notifications run in background, won't block orchestration
- **Fail-safe:** If notifications fail, orchestration continues normally
