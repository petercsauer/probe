---
segment: 4
title: "Implement TLS Keylog Reload"
depends_on: [1]
risk: 6/10
complexity: Medium
cycle_budget: 18
status: pending
commit_message: "feat(tui): Implement TLS keylog reload functionality"
---

# Segment 4: Implement TLS Keylog Reload

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Replace stub TLS keylog reload with actual implementation, enabling users to reload keylog files mid-session without restarting capture.

**Depends on:** Segment 1 (test utilities for comprehensive testing)

## Context: Issue 04 - TLS Keylog Reload Unimplemented

**Core Problem:** `app.rs:2595` shows success message but performs no action. Feature is a stub: `fn reload_tls_keylog(&mut self) -> Result<(), String> { Ok(()) }`. Users receive false feedback when changing keylog files.

**Proposed Fix:**
1. Add `reload_keylog()` to `TlsDecryptor` in prb-pcap
2. Wire through TUI with command channel to capture thread
3. Return actual result (not fake success)

## Scope
- **Crates:** prb-pcap (TLS decryption), prb-tui (UI + live capture)
- **Files:**
  - `crates/prb-pcap/src/tls/decrypt.rs` (~30 lines: reload method)
  - `crates/prb-tui/src/app.rs:2595` (~15 lines: replace stub)
  - `crates/prb-tui/src/live.rs` (~40 lines: command channel)

## Implementation Approach

### Step 1: Add reload_keylog to TlsDecryptor
```rust
// crates/prb-pcap/src/tls/decrypt.rs
impl TlsDecryptor {
    pub fn reload_keylog(&mut self, path: &Path) -> Result<(), TlsError> {
        let new_keylog = KeylogFile::from_path(path)?;
        self.session_cache.clear();
        self.keylog = Some(new_keylog);
        self.pending_retry = true;
        Ok(())
    }
}
```

### Step 2: Add command enum and channel
```rust
// crates/prb-tui/src/live.rs
enum CaptureCommand {
    Stop,
    ReloadKeylog(PathBuf),
}

// In capture thread:
match cmd_rx.try_recv() {
    Ok(CaptureCommand::ReloadKeylog(path)) => {
        adapter.reload_tls_keylog(&path)?;
    }
    // ...
}
```

### Step 3: Update App::reload_tls_keylog
```rust
// crates/prb-tui/src/app.rs:2595
fn reload_tls_keylog(&mut self) -> Result<(), String> {
    let keylog_path = self.capture_config
        .as_ref()
        .and_then(|cfg| cfg.tls_keylog_path.as_ref())
        .ok_or("No TLS keylog configured")?;

    if let Some(handle) = &mut self.capture_handle {
        handle.send_reload_keylog(keylog_path)
            .map_err(|e| format!("Failed to reload: {}", e))?;
    }

    Ok(())
}
```

## Build and Test Commands

**Build:** `cargo build --package prb-pcap --package prb-tui`

**Test (targeted):**
```bash
cargo test --package prb-pcap --lib tls::decrypt::tests
cargo test --package prb-tui --lib test_reload_tls_keylog
```

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** New unit tests verify reload clears cache and updates keylog
2. **Regression tests:** All TLS decryption tests pass (no behavior change for non-reload)
3. **Full build gate:** `cargo build --workspace` succeeds
4. **Full test suite:** `cargo test --workspace --all-targets` passes
5. **Self-review:** No TODO at line 2595, no fake success messages
6. **Scope verification:** Only files listed in Scope section modified
