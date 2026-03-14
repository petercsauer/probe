//! Autocomplete functionality for Wireshark-style display filters.
//!
//! Provides context-aware suggestions with fuzzy matching using nucleo-matcher.

use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Widget};

/// Autocomplete suggestion with description and category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Suggestion text to insert.
    pub text: String,
    /// Human-readable description.
    pub description: String,
    /// Suggestion category for context filtering.
    pub category: SuggestionCategory,
}

/// Category of suggestion for context-aware filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionCategory {
    /// Field name (e.g., tcp.port, ip.src).
    Field,
    /// Comparison or logical operator (==, !=, &&, ||).
    Operator,
    /// Enum value (tcp, udp, grpc, inbound, outbound).
    Value,
}

/// Autocomplete state with fuzzy matcher and filtered suggestions.
pub struct AutocompleteState {
    /// All available suggestions.
    suggestions: Vec<Suggestion>,
    /// Filtered suggestions based on current input.
    filtered: Vec<Suggestion>,
    /// Selected suggestion index.
    selected: usize,
    /// Fuzzy matcher for scoring.
    matcher: Matcher,
    /// Whether dropdown is visible.
    visible: bool,
}

impl AutocompleteState {
    /// Create new autocomplete state with full suggestion catalog.
    #[must_use]
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

    /// Build comprehensive suggestion catalog from DebugEvent fields and operators.
    fn build_suggestions() -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Core event fields
        suggestions.push(Suggestion {
            text: "id".to_string(),
            description: "Event identifier (numeric)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "timestamp".to_string(),
            description: "Event timestamp in nanoseconds".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "transport".to_string(),
            description: "Transport protocol (gRPC, ZMQ, TCP, UDP, etc.)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "direction".to_string(),
            description: "Message direction (inbound, outbound, unknown)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "sequence".to_string(),
            description: "Sequence number within stream".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "warnings".to_string(),
            description: "Parse warnings (use with 'exists')".to_string(),
            category: SuggestionCategory::Field,
        });

        // Source fields
        suggestions.push(Suggestion {
            text: "adapter".to_string(),
            description: "Capture adapter (pcap, json-fixture, etc.)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "source.adapter".to_string(),
            description: "Capture adapter (same as 'adapter')".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "origin".to_string(),
            description: "Origin identifier (file path, device name)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "source.origin".to_string(),
            description: "Origin identifier (same as 'origin')".to_string(),
            category: SuggestionCategory::Field,
        });

        // Network address fields
        suggestions.push(Suggestion {
            text: "src".to_string(),
            description: "Source IP:port".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "source.src".to_string(),
            description: "Source IP:port (same as 'src')".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "dst".to_string(),
            description: "Destination IP:port".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "source.dst".to_string(),
            description: "Destination IP:port (same as 'dst')".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "network".to_string(),
            description: "Network information (use with 'exists')".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "source.network".to_string(),
            description: "Network information (same as 'network')".to_string(),
            category: SuggestionCategory::Field,
        });

        // Protocol-specific metadata fields (gRPC)
        suggestions.push(Suggestion {
            text: "grpc.method".to_string(),
            description: "gRPC method name (e.g., /api.v1.Users/Get)".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "grpc.status".to_string(),
            description: "gRPC status code".to_string(),
            category: SuggestionCategory::Field,
        });

        // HTTP/2 metadata fields
        suggestions.push(Suggestion {
            text: "h2.stream_id".to_string(),
            description: "HTTP/2 stream identifier".to_string(),
            category: SuggestionCategory::Field,
        });

        // ZeroMQ metadata fields
        suggestions.push(Suggestion {
            text: "zmq.topic".to_string(),
            description: "ZeroMQ topic name".to_string(),
            category: SuggestionCategory::Field,
        });

        // DDS metadata fields
        suggestions.push(Suggestion {
            text: "dds.domain_id".to_string(),
            description: "DDS domain ID".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "dds.topic_name".to_string(),
            description: "DDS topic name".to_string(),
            category: SuggestionCategory::Field,
        });

        // OpenTelemetry metadata fields
        suggestions.push(Suggestion {
            text: "otel.trace_id".to_string(),
            description: "OpenTelemetry trace ID".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "otel.span_id".to_string(),
            description: "OpenTelemetry span ID".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "otel.trace_flags".to_string(),
            description: "OpenTelemetry trace flags".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "otel.parent_span_id".to_string(),
            description: "OpenTelemetry parent span ID".to_string(),
            category: SuggestionCategory::Field,
        });
        suggestions.push(Suggestion {
            text: "otel.trace_sampled".to_string(),
            description: "OpenTelemetry trace sampled flag".to_string(),
            category: SuggestionCategory::Field,
        });

        // Comparison operators
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
            text: ">".to_string(),
            description: "Greater than".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: ">=".to_string(),
            description: "Greater than or equal to".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "<".to_string(),
            description: "Less than".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "<=".to_string(),
            description: "Less than or equal to".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "contains".to_string(),
            description: "String contains (case-insensitive)".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "exists".to_string(),
            description: "Field exists check".to_string(),
            category: SuggestionCategory::Operator,
        });

        // Logical operators
        suggestions.push(Suggestion {
            text: "&&".to_string(),
            description: "Logical AND".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "||".to_string(),
            description: "Logical OR".to_string(),
            category: SuggestionCategory::Operator,
        });
        suggestions.push(Suggestion {
            text: "!".to_string(),
            description: "Logical NOT".to_string(),
            category: SuggestionCategory::Operator,
        });

        // Transport enum values
        suggestions.push(Suggestion {
            text: "\"gRPC\"".to_string(),
            description: "gRPC over HTTP/2 transport".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "\"ZMQ\"".to_string(),
            description: "ZeroMQ transport".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "\"DDS-RTPS\"".to_string(),
            description: "DDS RTPS transport".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "\"TCP\"".to_string(),
            description: "Raw TCP transport".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "\"UDP\"".to_string(),
            description: "Raw UDP transport".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "\"JSON-Fixture\"".to_string(),
            description: "JSON fixture input".to_string(),
            category: SuggestionCategory::Value,
        });

        // Direction enum values
        suggestions.push(Suggestion {
            text: "\"inbound\"".to_string(),
            description: "Inbound message direction".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "\"outbound\"".to_string(),
            description: "Outbound message direction".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "\"unknown\"".to_string(),
            description: "Unknown message direction".to_string(),
            category: SuggestionCategory::Value,
        });

        // Boolean values
        suggestions.push(Suggestion {
            text: "true".to_string(),
            description: "Boolean true".to_string(),
            category: SuggestionCategory::Value,
        });
        suggestions.push(Suggestion {
            text: "false".to_string(),
            description: "Boolean false".to_string(),
            category: SuggestionCategory::Value,
        });

        suggestions
    }

    /// Update filtered suggestions based on current input and cursor position.
    pub fn update(&mut self, input: &str, cursor_pos: usize) {
        // Determine context: are we at start, after field, after operator?
        let context = Self::determine_context(input, cursor_pos);

        // Extract the current word being typed
        let prefix = Self::extract_current_word(input, cursor_pos);

        // Don't show suggestions for empty prefix
        if prefix.is_empty() {
            self.filtered.clear();
            self.visible = false;
            return;
        }

        // Filter by category first
        let candidates: Vec<&Suggestion> = self
            .suggestions
            .iter()
            .filter(|s| Self::matches_context(s, context))
            .collect();

        // Apply fuzzy matching with nucleo-matcher
        // Convert strings to UTF-32 for nucleo-matcher
        let mut needle_buf = Vec::new();
        let needle = Utf32Str::new(&prefix, &mut needle_buf);
        let mut scored: Vec<(u16, &Suggestion)> = candidates
            .into_iter()
            .filter_map(|s| {
                let mut haystack_buf = Vec::new();
                let haystack = Utf32Str::new(&s.text, &mut haystack_buf);
                self.matcher
                    .fuzzy_match(haystack, needle)
                    .map(|score| (score, s))
            })
            .collect();

        // Sort by score descending
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));

        // Take top 10 suggestions
        self.filtered = scored
            .into_iter()
            .take(10)
            .map(|(_, s)| s.clone())
            .collect();

        self.visible = !self.filtered.is_empty();
        self.selected = 0;
    }

    /// Determine suggestion context based on input and cursor position.
    fn determine_context(input: &str, cursor_pos: usize) -> SuggestionCategory {
        let before_cursor = &input[..cursor_pos];
        let trimmed = before_cursor.trim_end();

        // After comparison operators, suggest values
        if trimmed.ends_with("==")
            || trimmed.ends_with("!=")
            || trimmed.ends_with('>')
            || trimmed.ends_with('<')
            || trimmed.ends_with(">=")
            || trimmed.ends_with("<=")
        {
            return SuggestionCategory::Value;
        }

        // After "contains", suggest values (string literals)
        if trimmed.ends_with("contains") {
            return SuggestionCategory::Value;
        }

        // Check if cursor is after a field name (with or without trailing space)
        // If we have whitespace before cursor but after the last word, suggest operators
        if before_cursor.ends_with(' ') && !trimmed.is_empty() {
            if let Some(last_word) = trimmed.split_whitespace().last() {
                // Check if it looks like a field (contains dot or is an identifier)
                if last_word.contains('.')
                    || last_word.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    return SuggestionCategory::Operator;
                }
            }
        }

        // If we have a partial field name being typed (no trailing space), suggest fields
        // Default: suggest fields
        SuggestionCategory::Field
    }

    /// Extract the word currently being typed at cursor position.
    fn extract_current_word(input: &str, cursor_pos: usize) -> &str {
        let before_cursor = &input[..cursor_pos];

        // Find the start of the current word (after whitespace or operators)
        let word_start = before_cursor
            .rfind(|c: char| c.is_whitespace() || "()&|!".contains(c))
            .map(|pos| pos + 1)
            .unwrap_or(0);

        &before_cursor[word_start..]
    }

    /// Check if suggestion category matches the expected context.
    fn matches_context(suggestion: &Suggestion, context: SuggestionCategory) -> bool {
        suggestion.category == context
    }

    /// Get currently selected suggestion if any.
    #[must_use]
    pub fn selected(&self) -> Option<&Suggestion> {
        if self.visible {
            self.filtered.get(self.selected)
        } else {
            None
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Accept currently selected suggestion.
    #[must_use]
    pub fn accept(&self) -> Option<String> {
        self.selected().map(|s| s.text.clone())
    }

    /// Dismiss the dropdown.
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.filtered.clear();
    }

    /// Check if dropdown is visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Render the autocomplete dropdown.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if !self.visible || self.filtered.is_empty() {
            return;
        }

        let items: Vec<ListItem> = self
            .filtered
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let content = format!("{} - {}", s.text, s.description);
                if i == self.selected {
                    ListItem::new(content).style(Style::default().bg(Color::Blue).fg(Color::White))
                } else {
                    ListItem::new(content)
                }
            })
            .collect();

        let list =
            List::new(items).block(Block::default().borders(Borders::ALL).title("Suggestions"));

        Widget::render(list, area, buf);
    }
}

impl Default for AutocompleteState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_at_start_suggests_fields() {
        let context = AutocompleteState::determine_context("", 0);
        assert_eq!(context, SuggestionCategory::Field);

        let context = AutocompleteState::determine_context("trans", 5);
        assert_eq!(context, SuggestionCategory::Field);
    }

    #[test]
    fn context_after_field_suggests_operators() {
        let context = AutocompleteState::determine_context("transport ", 10);
        assert_eq!(context, SuggestionCategory::Operator);

        let context = AutocompleteState::determine_context("grpc.method ", 12);
        assert_eq!(context, SuggestionCategory::Operator);
    }

    #[test]
    fn context_after_operator_suggests_values() {
        let context = AutocompleteState::determine_context("transport == ", 13);
        assert_eq!(context, SuggestionCategory::Value);

        let context = AutocompleteState::determine_context("id > ", 5);
        assert_eq!(context, SuggestionCategory::Value);

        let context = AutocompleteState::determine_context("direction != ", 13);
        assert_eq!(context, SuggestionCategory::Value);
    }

    #[test]
    fn context_after_contains_suggests_values() {
        let context = AutocompleteState::determine_context("grpc.method contains ", 21);
        assert_eq!(context, SuggestionCategory::Value);
    }

    #[test]
    fn extract_current_word_at_start() {
        let word = AutocompleteState::extract_current_word("trans", 5);
        assert_eq!(word, "trans");
    }

    #[test]
    fn extract_current_word_after_space() {
        let word = AutocompleteState::extract_current_word("transport == gR", 15);
        assert_eq!(word, "gR");
    }

    #[test]
    fn extract_current_word_after_operator() {
        let word = AutocompleteState::extract_current_word("transport&&grpc", 15);
        assert_eq!(word, "grpc");
    }

    #[test]
    fn fuzzy_match_typo() {
        let mut state = AutocompleteState::new();
        state.update("tansport", 8);
        assert!(state.is_visible());
        assert!(!state.filtered.is_empty());
        assert!(state.filtered.iter().any(|s| s.text == "transport"));
    }

    #[test]
    fn fuzzy_match_abbreviation() {
        let mut state = AutocompleteState::new();
        state.update("grpcmthd", 8);
        assert!(state.is_visible());
        assert!(state.filtered.iter().any(|s| s.text == "grpc.method"));
    }

    #[test]
    fn select_navigation() {
        let mut state = AutocompleteState::new();
        state.update("t", 1);
        assert!(state.is_visible());
        let initial = state.selected;

        state.select_next();
        assert_ne!(state.selected, initial);

        state.select_prev();
        assert_eq!(state.selected, initial);
    }

    #[test]
    fn accept_returns_selected_text() {
        let mut state = AutocompleteState::new();
        state.update("transport", 9);
        assert!(state.is_visible());

        let accepted = state.accept();
        assert!(accepted.is_some());
        assert_eq!(accepted.unwrap(), "transport");
    }

    #[test]
    fn dismiss_hides_dropdown() {
        let mut state = AutocompleteState::new();
        state.update("transport", 9);
        assert!(state.is_visible());

        state.dismiss();
        assert!(!state.is_visible());
    }

    #[test]
    fn empty_input_shows_all_fields() {
        let mut state = AutocompleteState::new();
        state.update("", 0);
        // Empty input should match nothing with fuzzy matcher
        assert!(!state.is_visible());
    }

    #[test]
    fn limits_to_10_suggestions() {
        let mut state = AutocompleteState::new();
        // Single letter should match many fields
        state.update("t", 1);
        assert!(state.filtered.len() <= 10);
    }
}
