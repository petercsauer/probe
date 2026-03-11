---
segment: 14
title: Theme System & Configuration
depends: []
risk: 4
complexity: Medium
cycle_budget: 7
estimated_lines: 650
---

# Segment 14: Theme System & Configuration

## Context

Theme system exists but can't be changed at runtime. Need theme switching, TOML config, and theme editor.

## Goal

Add runtime theme switching, TOML config file, and theme customization UI.

## Exit Criteria

1. [ ] Config file at ~/.prb/config.toml
2. [ ] Runtime theme switching with `:theme <name>`
3. [ ] Built-in themes: default, dark, light, solarized, monokai
4. [ ] Theme editor overlay for live customization
5. [ ] Save custom themes to config
6. [ ] Hot reload config file
7. [ ] Color preview in theme editor
8. [ ] Manual test: switch themes, customize, save

## Implementation Notes

### Files to Modify

- `crates/prb-tui/src/config.rs` (~200 lines)
  - TOML config parsing
  - Theme loading
  - Config hot reload
- `crates/prb-tui/src/theme.rs` (~250 lines)
  - More built-in themes
  - Custom theme support
- `crates/prb-tui/src/overlays/theme_editor.rs` (~200 lines NEW)
  - Theme customization UI

### Config File

```toml
[tui]
theme = "dark"
auto_follow = true

[tui.colors]
background = "#1a1a2e"
foreground = "#eaeaea"
accent = "#66ccff"
```

### Theme Switching

```rust
fn switch_theme(&mut self, name: &str) {
    if let Some(theme) = ThemeConfig::load(name) {
        self.theme = theme;
        self.config.tui.theme = name.to_string();
        self.save_config();
    }
}
```

## Test Plan

1. Create config file
2. Launch TUI
3. Switch themes with `:theme dark`
4. Customize colors in editor
5. Save and reload
6. Run test suite

## Blocked By

None - theme system is independent.

## Blocks

- S16 (Accessibility) - needs theme system for high contrast

## Rollback Plan

Remove runtime switching, keep static themes.

## Success Metrics

- Theme switching works instantly
- Config persists across sessions
- Theme editor is intuitive
- Zero regressions
