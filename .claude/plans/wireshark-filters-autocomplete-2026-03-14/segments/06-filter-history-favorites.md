---
segment: 6
title: "Add Filter History and Favorites"
depends_on: [3]
risk: 3/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(tui): Add filter history and favorites with TOML persistence"
---

# Segment 6: Add Filter History and Favorites

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add filter history (max 50 entries) with Up/Down navigation and favorites list with star/unstar, persisted to `~/.config/prb/filters.toml`.

**Depends on:** Segment 3 (query planner ensures performant history replay)

## Context: Issue 6 - No Filter Persistence

**Core Problem:**
- Filter history exists in `filter_state.rs` (max 50) but not persisted across sessions
- No favorites/bookmarks for common filters
- Users must retype complex filters like `tcp.port in {80,443,8080} && tcp.payload matches "^(GET|POST)"`
- TOML config already exists at `~/.config/prb/config.toml` for other settings

**Current FilterState:**
```rust
// filter_state.rs
pub struct FilterState {
    history: VecDeque<String>,  // Max 50, in-memory only
    history_index: Option<usize>,
    // ... other fields
}

impl FilterState {
    pub fn add_to_history(&mut self, filter: String) {
        if self.history.len() >= 50 {
            self.history.pop_front();
        }
        self.history.push_back(filter);
    }

    pub fn history_up(&mut self) {
        // Navigate backwards through history
    }

    pub fn history_down(&mut self) {
        // Navigate forwards through history
    }
}
```

**Root Cause:**
History is runtime-only. No persistence layer. No favorites/bookmarks UI.

**Proposed Fix:**
Add persistence layer with TOML serialization and favorites management:

```rust
// New file: crates/prb-tui/src/filter_persistence.rs

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterPersistence {
    /// Recent filter history (max 50)
    pub history: Vec<String>,

    /// Favorited filters with optional names
    pub favorites: Vec<FilterFavorite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterFavorite {
    pub name: String,         // User-provided name (e.g., "DNS Traffic")
    pub filter: String,       // The filter expression
    pub description: String,  // Optional description
    pub created_at: String,   // ISO 8601 timestamp
}

impl FilterPersistence {
    pub fn load() -> Result<Self, String> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read filters.toml: {}", e))?;

        toml::from_str(&content)
            .map_err(|e| format!("Failed to parse filters.toml: {}", e))
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize filters: {}", e))?;

        fs::write(&path, content)
            .map_err(|e| format!("Failed to write filters.toml: {}", e))
    }

    fn config_path() -> Result<PathBuf, String> {
        let home = std::env::var("HOME")
            .map_err(|_| "HOME environment variable not set".to_string())?;

        Ok(PathBuf::from(home)
            .join(".config")
            .join("prb")
            .join("filters.toml"))
    }

    pub fn add_to_history(&mut self, filter: String) {
        // Remove duplicates
        self.history.retain(|f| f != &filter);

        // Add to front
        self.history.insert(0, filter);

        // Truncate to 50
        self.history.truncate(50);
    }

    pub fn add_favorite(&mut self, name: String, filter: String, description: String) {
        let favorite = FilterFavorite {
            name,
            filter,
            description,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        self.favorites.push(favorite);
    }

    pub fn remove_favorite(&mut self, index: usize) {
        if index < self.favorites.len() {
            self.favorites.remove(index);
        }
    }

    pub fn is_favorited(&self, filter: &str) -> bool {
        self.favorites.iter().any(|f| f.filter == filter)
    }
}

impl Default for FilterPersistence {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            favorites: Vec::new(),
        }
    }
}

// Update FilterState to use FilterPersistence
impl FilterState {
    pub fn new() -> Self {
        let persistence = FilterPersistence::load().unwrap_or_default();

        Self {
            // Convert Vec to VecDeque for existing logic
            history: persistence.history.iter().cloned().collect(),
            persistence,
            // ... other fields
        }
    }

    pub fn add_to_history(&mut self, filter: String) {
        // Update in-memory history
        if self.history.len() >= 50 {
            self.history.pop_front();
        }
        self.history.push_back(filter.clone());

        // Update persistence
        self.persistence.add_to_history(filter);
        let _ = self.persistence.save(); // Ignore errors, don't block UX
    }

    pub fn toggle_favorite(&mut self) {
        let current_filter = self.input.clone();

        if self.persistence.is_favorited(&current_filter) {
            // Remove from favorites
            if let Some(index) = self.persistence.favorites.iter()
                .position(|f| f.filter == current_filter) {
                self.persistence.remove_favorite(index);
            }
        } else {
            // Add to favorites (prompt for name)
            // For MVP: use first 30 chars of filter as name
            let name = if current_filter.len() > 30 {
                format!("{}...", &current_filter[..27])
            } else {
                current_filter.clone()
            };

            self.persistence.add_favorite(name, current_filter, String::new());
        }

        let _ = self.persistence.save();
    }

    pub fn is_current_favorited(&self) -> bool {
        self.persistence.is_favorited(&self.input)
    }

    pub fn get_favorites(&self) -> &[FilterFavorite] {
        &self.persistence.favorites
    }
}

// Add keyboard shortcuts in app.rs
// Ctrl+F: Toggle favorite for current filter
// F2: Show favorites dialog (list widget with selection)
```

**TOML format:**
```toml
# ~/.config/prb/filters.toml

history = [
    "tcp.port == 443",
    "udp.port == 53",
    "transport == \"grpc\"",
]

[[favorites]]
name = "HTTPS Traffic"
filter = "tcp.port in {443, 8443}"
description = "All HTTPS connections"
created_at = "2026-03-14T10:30:00Z"

[[favorites]]
name = "DNS Queries"
filter = "udp.port == 53"
description = "Standard DNS traffic"
created_at = "2026-03-14T10:31:00Z"
```

**UI enhancements:**
- Show star icon (★/☆) next to filter input if current filter is favorited
- Ctrl+F to toggle favorite status
- F2 to open favorites dialog (scrollable list, Enter to apply)
- Up/Down in filter input navigates history (already implemented)

**Pre-Mortem Risks:**
1. **TOML parse errors**: Corrupted file breaks filter loading (catch and use default)
2. **Concurrent writes**: Multiple probe instances could clobber file (acceptable, rare case)
3. **File size growth**: 50 history + unlimited favorites could grow large (add max 100 favorites)
4. **Timestamp dependency**: chrono crate adds compile time (use std::time instead)
5. **Favorites dialog UI**: New dialog mode adds complexity (defer to S7 if time-constrained)

**Alternatives Ruled Out:**
- **JSON instead of TOML**: TOML is more human-readable, already used in probe config
- **SQLite database**: Overkill for simple list storage
- **History limit > 50**: More entries = slower navigation, 50 is Wireshark default
- **Auto-favorite frequently used filters**: Explicit user action is clearer

## Scope

**Files to create:**
- `crates/prb-tui/src/filter_persistence.rs` - New persistence module
- `crates/prb-tui/tests/filter_persistence_test.rs` - Test TOML serialization

**Files to modify:**
- `crates/prb-tui/src/filter_state.rs` - Add `persistence: FilterPersistence` field
- `crates/prb-tui/src/app.rs` - Add Ctrl+F keyboard shortcut, show star icon
- `crates/prb-tui/src/lib.rs` - Export filter_persistence module
- `crates/prb-tui/Cargo.toml` - Add `toml = "0.8"` and `chrono = "0.4"` dependencies

**Unchanged files:**
- `~/.config/prb/config.toml` - Existing config file unchanged, filters go to separate file
- `crates/prb-query/src/parser.rs` - No parser changes needed

## Implementation Approach

1. **Add dependencies**
   - Add `toml = "0.8"` to Cargo.toml
   - Add `chrono = "0.4"` to Cargo.toml (or use std::time)
   - Add `serde = { version = "1.0", features = ["derive"] }`

2. **Create filter_persistence.rs module**
   - Define `FilterPersistence` struct with history and favorites
   - Define `FilterFavorite` struct with name, filter, description, timestamp
   - Implement TOML serialization/deserialization
   - Implement `load()` and `save()` methods

3. **Update FilterState**
   - Add `persistence: FilterPersistence` field
   - Load persistence in `new()`
   - Update `add_to_history()` to persist
   - Add `toggle_favorite()` method
   - Add `is_current_favorited()` method

4. **Add keyboard shortcuts in app.rs**
   - Ctrl+F: Call `filter_state.toggle_favorite()`
   - Show star icon (★ or ☆) based on `is_current_favorited()`

5. **Handle file I/O errors gracefully**
   - If load fails, use default (empty history/favorites)
   - If save fails, log error but don't crash
   - Don't block UI on file operations

6. **Write comprehensive tests**
   - Test TOML serialization roundtrip
   - Test history truncation at 50 entries
   - Test duplicate removal in history
   - Test favorite add/remove
   - Test `is_favorited()` check
   - Integration test: save, restart, load

## Build and Test Commands

**Build:** `cargo build --package prb-tui`

**Test (targeted):** `cargo test --package prb-tui filter_persistence`

**Test (regression):** `cargo test --package prb-tui`

**Test (full gate):** `cargo test --workspace --all-targets`

**Manual test:** Run TUI, add filters to history, restart, verify history persists

## Exit Criteria

1. **Targeted tests:**
   - `test_toml_roundtrip` - Serialize and deserialize FilterPersistence
   - `test_history_truncate` - Adding 51st entry removes oldest
   - `test_history_dedup` - Adding duplicate moves to front
   - `test_add_favorite` - Favorite is added to list
   - `test_remove_favorite` - Favorite is removed by index
   - `test_is_favorited` - Check if filter is in favorites
   - `test_load_missing_file` - Returns default if file doesn't exist
   - `test_load_corrupted_file` - Returns default if parse fails
   - Integration: Save, load, verify history and favorites

2. **Regression tests:** All existing prb-tui tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - File I/O errors handled gracefully
   - No blocking operations in UI thread
   - History limited to 50 entries
   - Favorites limited to 100 entries (or reasonable limit)
   - No dead code or commented-out blocks

6. **Scope verification gate:**
   - Only filter_state.rs, app.rs, lib.rs, Cargo.toml, and new files modified
   - TOML file written to `~/.config/prb/filters.toml`
   - Existing config.toml unchanged

**Manual verification:**
- Type `tcp.port == 443`, press Enter
- Type Up arrow → filter appears again (history works)
- Press Ctrl+F → star icon appears (favorited)
- Restart probe → Up arrow shows history, star shows favorites
- Create 51 filters → oldest is removed from history

**Risk Factor:** 3/10 - File I/O introduces failure modes, but well-tested and non-blocking

**Estimated Complexity:** Medium - TOML serialization is straightforward, integration with FilterState requires care

**Evidence for Optimality:**
1. **Codebase evidence**: TOML already used for config.toml, proven pattern
2. **User expectations**: History with Up/Down is standard in shells (bash, zsh)
3. **Wireshark semantics**: Wireshark has filter history and favorites (Display Filter Macros)
4. **Existing solutions**: Most packet capture tools (tshark, tcpdump) persist filters in shell history
