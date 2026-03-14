---
segment: 5
title: "Enhance Syntax Highlighting"
depends_on: [2]
risk: 2/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(tui): Enhance filter syntax highlighting with error states and new operators"
---

# Segment 5: Enhance Syntax Highlighting

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Enhance existing syntax highlighting in `app.rs:3559-3642` to support new operators from S2 (matches, in, functions) and add error state coloring.

**Depends on:** Segment 2 (parser must support new operators for highlighting)

## Context: Issue 5 - Basic Syntax Highlighting Exists

**Core Problem:**
- Syntax highlighting already implemented in `prb-tui/src/app.rs:3559-3642` with hand-written lexer
- Current colors: Green (strings), Magenta (numbers), Yellow (operators), Cyan (fields)
- Missing: matches, in, functions, slice syntax, error states (invalid regex, unmatched parens)

**Current implementation:**
```rust
// app.rs:3559-3642
fn highlight_filter_syntax(filter: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut chars = filter.chars().peekable();
    let mut current = String::new();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                // String literal (green)
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }
                current.push(ch);
                while let Some(c) = chars.next() {
                    current.push(c);
                    if c == '"' { break; }
                }
                spans.push(Span::styled(current.clone(), Style::default().fg(Color::Green)));
                current.clear();
            }
            '0'..='9' => {
                // Number (magenta)
                // ... number parsing logic
                spans.push(Span::styled(current.clone(), Style::default().fg(Color::Magenta)));
            }
            // ... operators, fields, etc.
        }
    }

    spans
}
```

**Root Cause:**
Highlighting was built for basic syntax only. New operators from S2 need highlighting rules:
- `matches` keyword → Yellow (like other operators)
- `in` keyword → Yellow
- `{1, 2, 3}` set syntax → Color::Cyan for braces, Magenta for values
- `field[0:4]` slice syntax → Cyan for field, Yellow for brackets/colon, Magenta for indices
- `len(field)` function syntax → Color::Blue for function name, Cyan for field

Also need error highlighting:
- Invalid regex in `matches "(?invalid"` → Red
- Unmatched parentheses → Red
- Unknown field names → DarkGray (optional, requires field catalog)

**Proposed Fix:**
Extend lexer in `highlight_filter_syntax()`:

```rust
fn highlight_filter_syntax(filter: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut chars = filter.chars().peekable();
    let mut current = String::new();

    // Try to parse filter for error detection
    let parse_result = prb_query::parse_filter(filter);
    let has_parse_error = parse_result.is_err();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                // String literal (green), check if valid
                if !current.is_empty() {
                    spans.extend(highlight_token(&current));
                    current.clear();
                }

                current.push(ch);
                let mut escaped = false;
                while let Some(c) = chars.next() {
                    current.push(c);
                    if c == '\\' && !escaped {
                        escaped = true;
                    } else if c == '"' && !escaped {
                        break;
                    } else {
                        escaped = false;
                    }
                }

                // Check if this is a regex pattern (after "matches")
                let is_regex = spans.iter().rev()
                    .find(|s| !s.content.trim().is_empty())
                    .map(|s| s.content.trim() == "matches")
                    .unwrap_or(false);

                let color = if is_regex {
                    // Validate regex
                    let pattern = &current[1..current.len()-1]; // Strip quotes
                    if regex::Regex::new(pattern).is_ok() {
                        Color::Green
                    } else {
                        Color::Red // Invalid regex
                    }
                } else {
                    Color::Green
                };

                spans.push(Span::styled(current.clone(), Style::default().fg(color)));
                current.clear();
            }

            '{' | '}' => {
                // Set syntax for "in" operator
                if !current.is_empty() {
                    spans.extend(highlight_token(&current));
                    current.clear();
                }
                spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Cyan)));
            }

            '[' | ']' => {
                // Slice syntax
                if !current.is_empty() {
                    spans.extend(highlight_token(&current));
                    current.clear();
                }
                spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Yellow)));
            }

            '(' | ')' => {
                // Function call or grouping
                if !current.is_empty() {
                    // Check if previous token is a function name
                    let is_function = current.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && matches!(current.as_str(), "len" | "lower" | "upper");

                    if is_function {
                        spans.push(Span::styled(current.clone(), Style::default().fg(Color::Blue)));
                    } else {
                        spans.extend(highlight_token(&current));
                    }
                    current.clear();
                }
                spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Yellow)));
            }

            // ... rest of lexer
        }
    }

    // Highlight unmatched parentheses
    if has_parse_error {
        // Re-color entire filter as error if parse fails
        // Or more sophisticated: highlight specific error location
        if let Err(err) = parse_result {
            if err.contains("unmatched") || err.contains("expected") {
                // Add error indicator
                spans.push(Span::styled(" ⚠", Style::default().fg(Color::Red)));
            }
        }
    }

    spans
}

fn highlight_token(token: &str) -> Vec<Span> {
    let color = match token {
        // Logical operators
        "&&" | "||" | "!" | "and" | "or" | "not" => Color::Yellow,

        // Comparison operators
        "==" | "!=" | "<" | ">" | "<=" | ">=" => Color::Yellow,

        // New operators from S2
        "matches" | "in" | "contains" | "exists" => Color::Yellow,

        // Functions from S2
        "len" | "lower" | "upper" => Color::Blue,

        // Numbers
        s if s.chars().all(|c| c.is_numeric()) => Color::Magenta,

        // Fields (anything with a dot)
        s if s.contains('.') => Color::Cyan,

        // Default
        _ => Color::White,
    };

    vec![Span::styled(token.to_string(), Style::default().fg(color))]
}
```

**Pre-Mortem Risks:**
1. **Regex validation performance**: Compiling regex on every keystroke (acceptable, debouncing mitigates)
2. **Parse error position**: Parser doesn't report error location (can only show generic error indicator)
3. **Ambiguous tokens**: "in" could be field name or operator (context-sensitive lexing needed)
4. **Color contrast**: Some terminals have poor contrast for chosen colors (test on multiple terminals)

**Alternatives Ruled Out:**
- **Tree-sitter for syntax highlighting**: Overkill for simple filter syntax, adds complexity
- **No error highlighting**: Users want immediate feedback on invalid filters
- **Full semantic highlighting**: Would require type checking field references, too complex

## Scope

**Files to modify:**
- `crates/prb-tui/src/app.rs` - Enhance `highlight_filter_syntax()` function (lines 3559-3642)

**Unchanged files:**
- `crates/prb-query/src/parser.rs` - Parser used for validation, not modified
- `crates/prb-tui/src/filter_state.rs` - Highlighting is pure rendering, no state changes

## Implementation Approach

1. **Extract lexer logic**
   - Move `highlight_filter_syntax()` to separate function for clarity
   - Add helper `highlight_token()` for token classification

2. **Add new token types**
   - Add cases for `matches`, `in` keywords
   - Add cases for `{`, `}` (set syntax)
   - Add cases for `[`, `]` (slice syntax)
   - Add cases for `len`, `lower`, `upper` (functions)

3. **Add regex validation**
   - Detect strings after `matches` keyword
   - Attempt to compile regex
   - Color red if invalid, green if valid

4. **Add parse error indicator**
   - Call `prb_query::parse_filter()` once per highlight call
   - If parse fails, append error icon (⚠) at end
   - Or: color entire filter red (less precise but simpler)

5. **Test color contrast**
   - Test on light and dark terminal themes
   - Ensure error red is visible
   - Ensure cyan fields are distinct from blue functions

6. **Write tests** (if possible for rendering code)
   - Test token classification: "matches" → Yellow
   - Test regex validation: `matches "(?invalid"` → contains Red span
   - Test function highlighting: `len(field)` → "len" is Blue, "field" is Cyan
   - Visual test: screenshot of various filters

## Build and Test Commands

**Build:** `cargo build --package prb-tui`

**Test (targeted):** `cargo test --package prb-tui highlight` (if tests exist)

**Test (regression):** `cargo test --package prb-tui`

**Test (full gate):** `cargo test --workspace --all-targets`

**Visual test:** Run TUI and type various filters to verify colors

## Exit Criteria

1. **Targeted tests:**
   - `test_highlight_matches_keyword` - "matches" is Yellow
   - `test_highlight_in_keyword` - "in" is Yellow
   - `test_highlight_function` - "len" is Blue
   - `test_highlight_set_syntax` - `{1, 2}` has Cyan braces
   - `test_highlight_slice_syntax` - `[0:4]` has Yellow brackets
   - `test_highlight_invalid_regex` - `matches "(?invalid"` contains Red span
   - `test_highlight_parse_error` - Invalid filter shows error indicator

2. **Regression tests:** All existing prb-tui tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - All new operators from S2 highlighted
   - Error states visibly distinct
   - No performance regression (highlighting < 1ms)
   - No dead code or commented-out blocks

6. **Scope verification gate:**
   - Only app.rs modified (lines 3559-3642 and surrounding)
   - No changes to parser or filter_state

**Visual verification:**
- Type `tcp.port == 443` → "tcp.port" is Cyan, "==" is Yellow, "443" is Magenta
- Type `matches "^GET"` → "matches" is Yellow, "^GET" is Green
- Type `matches "(?invalid"` → pattern is Red
- Type `tcp.port in {80, 443}` → "in" is Yellow, braces are Cyan, numbers are Magenta
- Type `len(tcp.payload) > 100` → "len" is Blue, "tcp.payload" is Cyan

**Risk Factor:** 2/10 - Pure rendering enhancement, no logic changes, well-isolated

**Estimated Complexity:** Low - Extending existing lexer with new token types, straightforward

**Evidence for Optimality:**
1. **Codebase evidence**: Existing hand-written lexer works well, extension is natural fit
2. **User expectations**: Syntax highlighting matches Wireshark's color scheme (verified in screenshots)
3. **Performance**: Hand-written lexer faster than parser-based highlighting (Helix uses similar approach)
4. **Wireshark semantics**: Error highlighting helps users debug invalid filters immediately
