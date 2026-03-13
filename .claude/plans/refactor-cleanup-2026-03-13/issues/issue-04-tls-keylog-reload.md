---
id: "04"
title: "TLS Keylog Reload Unimplemented"
risk: 6/10
addressed_by_segments: [4]
---

# Issue 04: TLS Keylog Reload Unimplemented

## Core Problem

TUI displays "TLS keylog reloaded successfully" but no actual reload occurs. The feature is a stub that returns success without performing any action. Users changing keylog files mid-session receive false feedback, leading to trust issues and incorrect analysis when they believe TLS decryption is using updated keys.

**Evidence:**
```rust
// crates/prb-tui/src/app.rs:2595
fn reload_tls_keylog(&mut self) -> Result<(), String> {
    // TODO: Implement actual keylog reloading
    Ok(())
}
```

Called from:
- `crates/prb-tui/src/app.rs:1247` (Shift-K keybinding handler)
- UI shows success message: `"TLS keylog reloaded successfully"`

## Root Cause

1. **Architectural gap**: Keylog loading happens in `prb-pcap::tls::KeylogFile` but reload requires:
   - Re-parsing keylog file
   - Updating existing TLS session state in `TlsDecryptor`
   - Invalidating cached decryption contexts
   - Re-attempting decryption of buffered TLS records

2. **State management complexity**: `PcapCaptureAdapter` owns `TlsDecryptor`, but TUI's `App` doesn't have reference to it. Would need message-passing or shared state.

3. **Deferred implementation**: TODO comment suggests this was intentionally left unimplemented during Phase 2 TUI development.

## Proposed Fix

### Phase 1: Implement keylog reload in prb-pcap
Add `reload_keylog()` method to `TlsDecryptor`:
```rust
// crates/prb-pcap/src/tls/decrypt.rs
impl TlsDecryptor {
    pub fn reload_keylog(&mut self, path: &Path) -> Result<(), TlsError> {
        let new_keylog = KeylogFile::from_path(path)?;

        // Clear cached session keys
        self.session_cache.clear();

        // Update keylog file
        self.keylog = Some(new_keylog);

        // Mark all sessions as pending retry
        self.pending_retry = true;

        Ok(())
    }
}
```

### Phase 2: Wire through TUI app
Update `App::reload_tls_keylog()` to:
1. Get keylog path from capture config
2. Send reload message to capture adapter
3. Return actual result (not fake success)

**Implementation:**
```rust
// crates/prb-tui/src/app.rs:2595
fn reload_tls_keylog(&mut self) -> Result<(), String> {
    let keylog_path = self.capture_config
        .as_ref()
        .and_then(|cfg| cfg.tls_keylog_path.as_ref())
        .ok_or("No TLS keylog configured")?;

    // Send reload message to live capture adapter
    if let Some(capture_handle) = &mut self.capture_handle {
        capture_handle.send_reload_keylog(keylog_path)
            .map_err(|e| format!("Failed to reload keylog: {}", e))?;
    }

    Ok(())
}
```

### Phase 3: Update live capture adapter
Add channel-based command handling in `prb-tui/src/live.rs`:
```rust
enum CaptureCommand {
    Stop,
    ReloadKeylog(PathBuf),
}

// In capture thread loop:
match cmd_rx.try_recv() {
    Ok(CaptureCommand::ReloadKeylog(path)) => {
        adapter.reload_tls_keylog(&path)?;
    }
    // ...
}
```

## Existing Solutions Evaluated

### Wireshark TLS keylog reload
- **Pattern**: Watches keylog file for changes, reloads automatically
- **Applicability**: probe could adopt file watching, but manual reload is simpler first step
- **Recommendation**: **ADAPT** - Implement manual reload first, defer auto-reload to future enhancement

### rustls session cache
- **URL**: https://docs.rs/rustls/latest/rustls/trait.StoresServerSessions.html
- **Pattern**: Pluggable session storage
- **Applicability**: probe uses custom TLS decryption (not rustls client/server)
- **Recommendation**: **REJECT** - Different use case (we decrypt captured TLS, not terminate TLS connections)

### pcap-analyzer TLS handling
- **URL**: https://github.com/rusticata/pcap-analyzer
- **Pattern**: Stateless TLS parsing, no session management
- **Recommendation**: **REJECT** - probe's stateful TLS decryption is more advanced

## Alternatives Considered

### Alternative 1: Automatic keylog file watching
- **Description**: Use `notify` crate to watch keylog file, reload on change
- **Why rejected**: Adds complexity (file watcher thread, debouncing), manual reload sufficient for now
- **Future consideration**: Phase 3 enhancement if users request it

### Alternative 2: Restart capture to reload keylog
- **Description**: Document workaround - stop capture, edit keylog, restart capture
- **Why rejected**: Poor UX, loses in-flight state, doesn't match user expectation from UI button

### Alternative 3: Remove reload UI element
- **Description**: Hide Shift-K keybinding and UI hint until feature implemented
- **Why rejected**: Removes useful feature, users want this functionality (reason it was stubbed)

## Pre-Mortem — What Could Go Wrong

1. **Session state corruption**: Reloading keylog mid-capture could corrupt TLS session state
   - **Mitigation**: Lock session cache during reload, use atomic swap pattern
   - **Test**: Property test with random reload timing during active TLS traffic

2. **File I/O errors**: Keylog file might be locked, moved, or malformed
   - **Mitigation**: Return `Result<(), TlsError>` with descriptive errors, show in TUI
   - **Test**: Unit tests with invalid/missing keylog files

3. **Message channel deadlock**: UI thread sends reload command, capture thread blocks on full channel
   - **Mitigation**: Use `try_send()` with timeout, bounded channel
   - **Test**: Integration test with slow capture adapter, verify no UI freeze

4. **Keylog path not configured**: User presses Shift-K without TLS capture active
   - **Mitigation**: Check `capture_config.tls_keylog_path`, show error "No TLS keylog configured"
   - **Test**: Unit test `reload_tls_keylog()` with no capture config

5. **Performance impact**: Reloading keylog on large file (10k+ sessions) causes UI freeze
   - **Mitigation**: Reload happens in capture thread (async), show "Reloading..." status
   - **Test**: Benchmark with 100MB keylog file, verify UI remains responsive

## Risk Factor

**6/10** - Moderate risk
- Changes stateful TLS decryption logic (session management)
- Requires cross-thread message passing (deadlock risk)
- Affects live capture (production code path, not just tests)
- High user visibility (UI feedback must be accurate)

**Mitigating factors:**
- Feature is currently non-functional (can't make worse)
- Clear error boundary (reload is atomic operation)
- Testable in isolation (unit tests for TlsDecryptor, integration tests for TUI)

## Evidence for Optimality

### Source 1 (Codebase Evidence)
- `crates/prb-tui/src/app.rs:2595`: Stub implementation with TODO comment
- `crates/prb-pcap/src/tls/decrypt.rs:55-88`: Existing `TlsDecryptor` with session cache
- `crates/prb-pcap/src/tls/keylog.rs:1-45`: KeylogFile parsing already implemented
- `crates/prb-tui/src/live.rs:127`: Live capture already uses channel for stop command (pattern to follow)

### Source 2 (Project Conventions)
- ADR 0002: "Never panic on malformed input" - return Result for reload failures
- CONTRIBUTING.md line 157: "No .unwrap() in library code" - handle file I/O errors gracefully
- architecture.md line 198: TLS decryption happens in pcap layer, TUI is consumer

### Source 3 (Existing Solutions)
- **Wireshark**: Reloads keylog manually via "Reload" button in TLS preferences
- **tshark**: Accepts keylog via `-o tls.keylog_file:path`, no reload (restart required)
- **mitmproxy**: Watches keylog file, reloads automatically (more complex than probe needs)

### Source 4 (External Best Practices)
- Rust concurrency patterns: Use message-passing (mpsc) for cross-thread commands (The Rust Book Ch. 16)
- TLS session management: Clear session cache on keylog update (RFC 5246 - sessions are keylog-derived)
- UX principle: Never show fake success (Jakob Nielsen's Heuristic #1: Visibility of system status)

## Blast Radius

### Direct Changes
- **Modified files**:
  - `crates/prb-pcap/src/tls/decrypt.rs` (~30 lines: `reload_keylog()` method)
  - `crates/prb-tui/src/app.rs:2595` (~15 lines: replace stub with actual implementation)
  - `crates/prb-tui/src/live.rs` (~40 lines: add command channel handling)

### Potential Ripple
- `CaptureHandle` struct in `live.rs` needs new field: `cmd_tx: Sender<CaptureCommand>`
- `App` struct needs reference to `CaptureHandle` (already has it via `self.capture_handle`)
- Error types: `TlsError` needs new variant `KeylogReloadFailed`

### Test Impact
- **New tests**:
  - `prb-pcap/src/tls/decrypt.rs`: Unit test `reload_keylog_success()`, `reload_keylog_missing_file()`
  - `prb-tui/tests/`: Integration test for reload command flow
  - Property test: reload timing during active TLS capture (proptest)
- **Regression risk**: Existing TLS decryption tests must pass (no behavior change for non-reload case)

### Documentation Impact
- Update `docs/user-guide.md`: Document Shift-K keybinding (currently documented but non-functional)
- Update `crates/prb-pcap/README.md`: Document `reload_keylog()` public API
- Remove TODO comment from `app.rs:2595`
