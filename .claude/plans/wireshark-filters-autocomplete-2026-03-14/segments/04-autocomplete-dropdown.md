---
segment: 4
title: "Add Autocomplete Dropdown"
depends_on: [2]
risk: 5/10
complexity: High
cycle_budget: 20
status: pending
commit_message: "feat(tui): Add autocomplete dropdown with nucleo-matcher fuzzy search"
---

# Segment 4: Add Autocomplete Dropdown

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement autocomplete dropdown with nucleo-matcher fuzzy search, showing field names, operators, and values as user types in filter input.

**Depends on:** Segment 2 (parser must support all operators for suggestion context)

## Context: Issue 3 - No Autocomplete UI

**Core Problem:**
- Filter input has no autocomplete suggestions
- Users must memorize 60+ filterable fields (from DebugEvent)
- No guidance on operator syntax (matches, in, functions)
- Wireshark users expect Tab-completion with dropdown

**Current state:**
- Filter input exists in `prb-tui/src/filter_state.rs` (182 lines)
- No suggestion dropdown
- No fuzzy matching
- Reference: `command_palette.rs` has dropdown pattern with substring matching

**Required features:**
1. Dropdown UI showing suggestions below filter input
2. Fuzzy matching with nucleo-matcher (from Helix editor)
3. Context-aware suggestions:
   - Start of filter: suggest field names
   - After field name: suggest operators (==, !=, matches, in, etc.)
   - After operator: suggest values (for enums like transport)
   - After operator: suggest functions (len, lower, upper)
4. Tab/Down/Up navigation
5. Enter to accept suggestion
6. Escape to dismiss

**Proposed Fix:**
Add autocomplete module with nucleo-matcher integration:

```rust
// New file: crates/prb-tui/src/autocomplete.rs

use nucleo_matcher::{Matcher, Config};
use ratatui::widgets::{List, ListItem, Block, Borders};
use crossterm::event::{KeyCode, KeyEvent};

/// Autocomplete suggestion
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub description: String,
    pub category: SuggestionCategory,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionCategory {
    Field,       // tcp.port, udp.port, ip.src, etc.
    Operator,    // ==, !=, matches, in, etc.
    Value,       // "tcp", "udp", "grpc", etc.
    Function,    // len(), lower(), upper()
}

/// Autocomplete state
pub struct AutocompleteState {
    /// All available suggestions
    suggestions: Vec<Suggestion>,

    /// Filtered suggestions based on input
    filtered: Vec<Suggestion>,

    /// Selected suggestion index
    selected: usize,

    /// Fuzzy matcher
    matcher: Matcher,

    /// Is dropdown visible
    visible: bool,
}

impl AutocompleteState {
    pub fn new() -> Self {
        let suggestions = Self::build_suggestions();
        let matcher = Matcher::new(Config::DEFAULT);

        Self {
            suggestions,
            filtered: Vec::new(),
            selected: 0,
            matcher,
            visible: false,
        }
    }

    /// Build suggestion list from field catalog
    fn build_suggestions() -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Field suggestions (from DebugEvent structure)
        suggestions.push(Suggestion {
            text: "tcp.port".to_string(),
            description: "TCP source or destination port".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "tcp.srcport".to_string(),
            description: "TCP source port".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "tcp.dstport".to_string(),
            description: "TCP destination port".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "udp.port".to_string(),
            description: "UDP source or destination port".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "ip.src".to_string(),
            description: "Source IP address (without port)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "ip.dst".to_string(),
            description: "Destination IP address (without port)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "frame.len".to_string(),
            description: "Frame length in bytes".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "transport".to_string(),
            description: "Transport protocol (tcp, udp, icmp)".to_string(),
            category: SuggestionCategory::Field,
        });
        // ... add all 60+ fields from DebugEvent

        // Operator suggestions
        suggestions.push(Suggestion {
            text: "==".to_string(),
            description: "Equal to".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "!=".to_string(),
            description: "Not equal to".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "matches".to_string(),
            description: "Regex match".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "in".to_string(),
            description: "Set membership".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "contains".to_string(),
            description: "String contains".to_string(),
            category: SuggestionCategory::Operator,
        });

        // Function suggestions
        suggestions.push(Suggestion {
            text: "len(".to_string(),
            description: "Length of field".to_string(),
            category: SuggestionCategory::Function,
        });
        suggestions.push(Suggestion {
            text: "lower(".to_string(),
            description: "Convert to lowercase".to_string(),
            category: SuggestionCategory::Function,
        });
        suggestions.push(Suggestion {
            text: "upper(".to_string(),
            description: "Convert to uppercase".to_string(),
            category: SuggestionCategory::Function,
        });

        // Value suggestions (for enums)
        suggestions.push(Suggestion {
            text: "tcp".to_string(),
            description: "TCP transport".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "udp".to_string(),
            description: "UDP transport".to_string(),
            category: SuggestionCategory::Value,
        });

        suggestions
    }

    /// Update filtered suggestions based on current input
    pub fn update(&mut self, input: &str, cursor_pos: usize) {
        // Determine context: are we at start, after field, after operator?
        let context = Self::determine_context(input, cursor_pos);

        // Filter suggestions by category and fuzzy match
        let prefix = Self::extract_current_word(input, cursor_pos);

        self.filtered = self.suggestions.iter()
            .filter(|s| Self::matches_context(s, &context))
            .filter(|s| {
                let score = self.matcher.fuzzy_match(&s.text, prefix);
                score.is_some()
            })
            .cloned()
            .collect();

        // Sort by match score
        self.filtered.sort_by_key(|s| {
            std::cmp::Reverse(self.matcher.fuzzy_match(&s.text, prefix).unwrap_or(0))
        });

        // Limit to 10 suggestions
        self.filtered.truncate(10);

        self.visible = !self.filtered.is_empty();
        self.selected = 0;
    }

    fn determine_context(input: &str, cursor_pos: usize) -> SuggestionCategory {
        let before_cursor = &input[..cursor_pos];

        // Simple heuristic: if last non-whitespace char is operator, suggest values
        if before_cursor.trim_end().ends_with("==") || before_cursor.trim_end().ends_with("!=") {
            SuggestionCategory::Value
        } else if before_cursor.contains('.') && !before_cursor.contains(' ') {
            // After field name, before operator
            SuggestionCategory::Operator
        } else {
            // Start of filter or after logical operator
            SuggestionCategory::Field
        }
    }

    fn extract_current_word(input: &str, cursor_pos: usize) -> &str {
        let before_cursor = &input[..cursor_pos];
        before_cursor.split_whitespace().last().unwrap_or("")
    }

    fn matches_context(suggestion: &Suggestion, context: &SuggestionCategory) -> bool {
        &suggestion.category == context
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match key.code {
            KeyCode::Down => {
                if !self.filtered.is_empty() {
                    self.selected = (self.selected + 1) % self.filtered.len();
                }
                None
            }
            KeyCode::Up => {
                if !self.filtered.is_empty() {
                    self.selected = if self.selected == 0 {
                        self.filtered.len() - 1
                    } else {
                        self.selected - 1
                    };
                }
                None
            }
            KeyCode::Tab | KeyCode::Enter => {
                if let Some(suggestion) = self.filtered.get(self.selected) {
                    Some(suggestion.text.clone())
                } else {
                    None
                }
            }
            KeyCode::Esc => {
                self.visible = false;
                None
            }
            _ => None
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if !self.visible || self.filtered.is_empty() {
            return;
        }

        let items: Vec<ListItem> = self.filtered.iter()
            .enumerate()
            .map(|(i, s)| {
                let content = format!("{} - {}", s.text, s.description);
                if i == self.selected {
                    ListItem::new(content).style(Style::default().bg(Color::Blue))
                } else {
                    ListItem::new(content)
                }
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Suggestions"));

        Widget::render(list, area, buf);
    }
}

// Update FilterState to use AutocompleteState
impl FilterState {
    pub fn new() -> Self {
        Self {
            // ... existing fields
            autocomplete: AutocompleteState::new(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Try autocomplete first
        if let Some(text) = self.autocomplete.handle_key(key) {
            // Insert suggestion at cursor
            self.insert_text(&text);
            return;
        }

        // ... existing key handling
    }

    pub fn update_input(&mut self, input: String) {
        self.input = input;
        // Update autocomplete suggestions
        self.autocomplete.update(&self.input, self.cursor_pos);
    }
}
```

**Pre-Mortem Risks:**
1. **Context detection heuristic**: Simple regex-based context may fail for complex nested filters
2. **nucleo-matcher performance**: Fuzzy matching on 100+ suggestions on every keystroke (acceptable, Helix does this)
3. **Dropdown positioning**: May overflow screen on small terminals (clamp to visible area)
4. **Tab vs Enter**: Tab traditionally cycles suggestions, Enter accepts (verify user expectations)
5. **Field catalog maintenance**: Hardcoded field list needs sync with DebugEvent fields (consider codegen)

**Alternatives Ruled Out:**
- **Substring matching only**: Less powerful than fuzzy search, harder to find fields
- **Generate suggestions from parser**: Parser doesn't know all valid field names
- **skim/fzf integration**: Too heavyweight, nucleo-matcher is purpose-built for this
- **Dynamic field discovery**: Would require scanning events, too slow for real-time

## Scope

**Files to create:**
- `crates/prb-tui/src/autocomplete.rs` - New autocomplete module
- `crates/prb-tui/tests/autocomplete_test.rs` - Test autocomplete logic

**Files to modify:**
- `crates/prb-tui/Cargo.toml` - Add `nucleo-matcher = "0.3"` dependency
- `crates/prb-tui/src/filter_state.rs` - Add `autocomplete: AutocompleteState` field
- `crates/prb-tui/src/lib.rs` - Export autocomplete module
- `crates/prb-tui/src/app.rs` - Render autocomplete dropdown in filter area

**Unchanged files:**
- `crates/prb-query/src/parser.rs` - Autocomplete uses parser indirectly via suggestions
- `crates/prb-core/src/event.rs` - No changes to event structure

## Implementation Approach

1. **Add nucleo-matcher dependency**
   - Add `nucleo-matcher = "0.3"` to Cargo.toml
   - Import matcher in autocomplete.rs

2. **Create autocomplete.rs module**
   - Define `Suggestion` struct with text, description, category
   - Define `SuggestionCategory` enum
   - Define `AutocompleteState` with fuzzy matcher

3. **Build suggestion catalog**
   - Extract all field names from DebugEvent (60+ fields)
   - Add all operators from parser (==, !=, matches, in, contains, etc.)
   - Add all functions (len, lower, upper)
   - Add enum values (tcp, udp, grpc, zmq, etc.)

4. **Implement context detection**
   - Parse input to determine cursor position context
   - Start of filter → suggest fields
   - After field → suggest operators
   - After operator → suggest values/functions

5. **Implement fuzzy matching**
   - Use nucleo-matcher to score suggestions
   - Sort by score, take top 10
   - Update on every keystroke

6. **Integrate with FilterState**
   - Add `autocomplete: AutocompleteState` field
   - Forward key events (Tab, Enter, Up, Down, Esc)
   - Insert selected suggestion at cursor

7. **Render dropdown UI**
   - Position below filter input (or above if near bottom)
   - Highlight selected suggestion
   - Show description for each suggestion

8. **Write comprehensive tests**
   - Test context detection (field, operator, value contexts)
   - Test fuzzy matching (typos, abbreviations)
   - Test key navigation (Up/Down, Tab, Enter, Esc)
   - Test suggestion insertion
   - Integration test: type "tcp.p" → suggests "tcp.port"

## Build and Test Commands

**Build:** `cargo build --package prb-tui`

**Test (targeted):** `cargo test --package prb-tui autocomplete`

**Test (regression):** `cargo test --package prb-tui`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:**
   - `test_context_detection_field` - At start, suggests fields
   - `test_context_detection_operator` - After field, suggests operators
   - `test_context_detection_value` - After operator, suggests values
   - `test_fuzzy_match_typo` - "tpc" matches "tcp.port"
   - `test_fuzzy_match_abbreviation` - "tport" matches "tcp.port"
   - `test_key_navigation_down` - Down arrow moves selection
   - `test_key_navigation_up` - Up arrow moves selection
   - `test_tab_accepts_suggestion` - Tab inserts selected suggestion
   - `test_enter_accepts_suggestion` - Enter inserts selected suggestion
   - `test_escape_dismisses` - Escape hides dropdown
   - Integration: Type "udp.por" → dropdown shows "udp.port", "udp.srcport", "udp.dstport"

2. **Regression tests:** All existing prb-tui tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - All 60+ DebugEvent fields in suggestion catalog
   - All operators from S2 in suggestion catalog
   - Context detection handles nested parentheses
   - No dead code or commented-out blocks

6. **Scope verification gate:**
   - Only filter_state.rs, app.rs, lib.rs, Cargo.toml, and new files modified
   - nucleo-matcher dependency added with exact version
   - Dropdown rendering doesn't break existing TUI layout

**Risk Factor:** 5/10 - Context detection heuristics could be brittle, but fuzzy matching is well-tested in Helix

**Estimated Complexity:** High - Significant new UI component with context-aware logic and fuzzy matching

**Evidence for Optimality:**
1. **Existing solutions**: nucleo-matcher is from Helix editor, proven for autocomplete (100k+ downloads)
2. **Codebase evidence**: command_palette.rs shows dropdown pattern works well in probe TUI
3. **Wireshark semantics**: Context-aware suggestions match Wireshark's autocomplete behavior
4. **User expectations**: Fuzzy matching is standard in modern editors (VSCode, Helix, Neovim)
