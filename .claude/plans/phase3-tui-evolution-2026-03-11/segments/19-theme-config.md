---
segment: 19
title: "Theme System & Configuration"
depends_on: [1]
risk: 4
complexity: Medium
cycle_budget: 7
status: pending
commit_message: "feat(prb-tui): runtime theme switching, config file support, custom keybindings, profiles"
---

# Segment 19: Theme System & Configuration

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Replace the hardcoded `Theme` struct with a runtime-configurable theme system, and add a config file for persisting preferences, keybindings, and profiles.

**Depends on:** S01 (Visual Polish — theme styles to make configurable)

## Scope

- `crates/prb-tui/src/theme.rs` — Major rewrite: `ThemeConfig` struct, palette loading, runtime switching
- `crates/prb-tui/src/config.rs` — **New file.** Config file loading from `~/.config/prb/config.toml`
- `crates/prb-tui/src/app.rs` — `T` key for theme cycling, config integration

## Implementation

### 19.1 Theme Config Struct

Replace static `Theme` methods with a `ThemeConfig` instance:

```rust
pub struct ThemeConfig {
    pub name: String,
    pub selected_row_fg: Color,
    pub selected_row_bg: Color,
    pub zebra_bg: Color,
    pub focused_border: Color,
    pub unfocused_border: Color,
    pub transport_colors: HashMap<TransportKind, Color>,
    pub warning_bg: Color,
    // ... all semantic colors
}

impl ThemeConfig {
    pub fn dark() -> Self { /* default dark theme */ }
    pub fn light() -> Self { /* light theme */ }
    pub fn catppuccin_mocha() -> Self { /* catppuccin colors */ }
    pub fn dracula() -> Self { /* dracula colors */ }
}
```

All rendering code references `theme.selected_row()` etc. through the config.

### 19.2 Runtime Theme Switching

Press `T` to cycle through themes: Dark → Light → Catppuccin → Dracula → Dark...

Store active theme in `App`. Show current theme name briefly in status bar on switch.

### 19.3 Config File

Load from `~/.config/prb/config.toml`:

```toml
[tui]
theme = "catppuccin-mocha"
max_events = 100000
auto_follow = true
show_timeline = true

[tui.columns]
visible = ["#", "time", "source", "destination", "protocol", "direction", "summary"]

[tui.keybindings]
quit = "q"
filter = "/"
zoom = "z"
theme_cycle = "T"

[tui.profiles.grpc-debug]
theme = "dark"
default_filter = "transport == \"gRPC\""
columns = ["#", "time", "source", "protocol", "summary"]
```

Use `serde` + `toml` for parsing. Create directory and default config if not exists on first run.

### 19.4 Custom Keybindings

Allow rebinding all keys via config. Maintain a `KeyMap` struct:

```rust
pub struct KeyMap {
    bindings: HashMap<String, KeyCode>,
}
```

Support vim and emacs presets. Default to vim-style (current).

### 19.5 Profile Support

Named profiles for different analysis scenarios. Switch with `:profile <name>` via command palette.

## Key Files and Context

- `crates/prb-tui/src/theme.rs` — Current static Theme impl
- `crates/prb-tui/src/app.rs` — All rendering references Theme

## Exit Criteria

1. **Theme struct:** `ThemeConfig` replaces static `Theme` methods
2. **4 themes:** Dark, Light, Catppuccin Mocha, Dracula all render correctly
3. **Runtime switch:** `T` cycles themes, change is immediate
4. **Config file:** Reads from `~/.config/prb/config.toml` if present
5. **Keybindings:** At least quit, filter, zoom, help are rebindable via config
6. **Tests pass:** `cargo nextest run -p prb-tui`
7. **Full gate:** `cargo build --workspace && cargo clippy --workspace -- -D warnings`
