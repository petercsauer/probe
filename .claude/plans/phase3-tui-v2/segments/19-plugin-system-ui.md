---
segment: 19
title: Plugin System UI
depends: [05]
risk: 5
complexity: Medium
cycle_budget: 7
estimated_lines: 350
---

# Segment 19: Plugin System UI

## Context

Plugin system exists but has no TUI management. Need UI for installing, enabling/disabling, and configuring plugins.

## Goal

Add plugin manager UI for discovery, installation, and configuration of decoder plugins.

## Exit Criteria

1. [ ] Plugin manager overlay with `P` key
2. [ ] List installed plugins with status
3. [ ] Install plugin from path or URL
4. [ ] Enable/disable plugins
5. [ ] Configure plugin settings
6. [ ] Show plugin metadata (name, version, description)
7. [ ] Reload plugins without restart
8. [ ] Manual test: install and use plugin

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/overlays/plugin_manager.rs` (~250 lines)
  - Plugin list UI
  - Install/uninstall
  - Configuration
- `crates/prb-tui/src/app.rs` (~100 lines)
  - Wire plugin manager
  - Handle plugin reload

### Plugin Manager UI

```
╭─ Plugin Manager ────────────────────────────╮
│ Installed Plugins:                          │
│                                              │
│ ✓ mqtt-decoder     v1.0.0  [ENABLED]        │
│ ✓ custom-grpc      v0.1.0  [ENABLED]        │
│ ✗ test-plugin      v2.0.0  [DISABLED]       │
│                                              │
│ [i]nstall [e]nable [d]isable [c]onfigure    │
╰──────────────────────────────────────────────╯
```

### Plugin Operations

```rust
fn install_plugin(&mut self, path: &Path) -> Result<()> {
    let plugin = self.plugin_manager.load_plugin(path)?;
    self.plugins.push(plugin);
    self.save_plugin_config()?;
    Ok(())
}
```

## Test Plan

1. Press `P` to open manager
2. List plugins
3. Install test plugin
4. Enable/disable plugins
5. Configure plugin
6. Reload and verify
7. Run test suite

## Blocked By

- S05 (Schema Decode) - plugins may provide schemas

## Blocks

None - plugin UI is additive.

## Rollback Plan

Remove plugin manager UI, use CLI only.

## Success Metrics

- Plugin operations work
- UI is clear and intuitive
- Reload without restart works
- Zero regressions
