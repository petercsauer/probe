---
segment: 15
title: TUI config/theme to 60%
depends_on: [10]
risk: 2
complexity: Low
cycle_budget: 8
estimated_lines: ~220 test lines
---

# Segment 15: TUI Config/Theme/Loader to 60%

## Context

**Target modules:**
- `config.rs` - 25.88% → 60% (247 lines uncovered)
- `theme.rs` - 29.89% → 60% (461 lines uncovered)
- `loader.rs` - 30.14% → 60% (206 lines uncovered)

## Goal

Test configuration parsing, theme loading, data loading logic.

## Implementation Plan

### Priority 1: Config Parsing (~100 lines)

```rust
#[test]
fn test_config_from_toml() {
    let toml = r#"
        [ui]
        theme = "dark"
        [keys]
        quit = "q"
    "#;
    let config = Config::from_str(toml);
    assert!(config.is_ok());
}

#[test]
fn test_config_validation() {
    let invalid = Config { max_events: 0, .. };
    assert!(invalid.validate().is_err());
}
```

### Priority 2: Theme Parsing (~70 lines)

Test theme color parsing, fallbacks, validation.

### Priority 3: Data Loader (~50 lines)

Test MCAP file loading, format detection.

## Success Metrics

- config: 25.88% → 60%+
- theme: 29.89% → 60%+
- loader: 30.14% → 60%+
- ~30 new tests
