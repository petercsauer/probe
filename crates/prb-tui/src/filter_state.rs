use crate::filter_persistence::{FilterFavorite, FilterPersistence};
use prb_query::Filter;
use std::time::Instant;

const MAX_HISTORY_SIZE: usize = 50;
const DEBOUNCE_MS: u64 = 100;

/// State management for the filter bar with incremental preview, history, and debouncing.
#[derive(Debug, Clone)]
pub struct FilterState {
    /// Current text being edited in the filter bar
    pub text: String,

    /// Timestamp of the last text change (for debouncing)
    pub last_change: Instant,

    /// Preview filter parsed from current text (if valid)
    pub preview_filter: Option<Filter>,

    /// Preview count of filtered events (if preview_filter is valid)
    pub preview_count: Option<usize>,

    /// Committed filter that's actually applied
    pub committed_filter: Option<Filter>,

    /// History of filter expressions
    pub history: Vec<String>,

    /// Current position in history (None means not browsing history)
    pub history_cursor: Option<usize>,

    /// Temporary text buffer when browsing history (to restore when exiting history mode)
    history_temp_text: Option<String>,

    /// Persistent storage for history and favorites
    persistence: FilterPersistence,
}

impl Default for FilterState {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterState {
    pub fn new() -> Self {
        Self::new_with_persistence(true)
    }

    /// Create a new FilterState, optionally loading persistence.
    /// Use `load_persistence=false` for testing.
    pub fn new_with_persistence(load_persistence: bool) -> Self {
        let persistence = if load_persistence {
            FilterPersistence::load().unwrap_or_default()
        } else {
            FilterPersistence::default()
        };

        // Initialize history from persistence
        let history = persistence.history.clone();

        FilterState {
            text: String::new(),
            last_change: Instant::now(),
            preview_filter: None,
            preview_count: None,
            committed_filter: None,
            history,
            history_cursor: None,
            history_temp_text: None,
            persistence,
        }
    }

    /// Update the filter text and reset debounce timer.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.last_change = Instant::now();
        // Clear preview until debounce completes
        self.preview_filter = None;
        self.preview_count = None;
        // Clear history browsing state
        self.history_cursor = None;
        self.history_temp_text = None;
    }

    /// Check if enough time has elapsed since last change for debouncing.
    pub fn should_update_preview(&self) -> bool {
        self.last_change.elapsed().as_millis() as u64 >= DEBOUNCE_MS
    }

    /// Update preview filter and count.
    pub fn update_preview(&mut self, filter: Option<Filter>, count: Option<usize>) {
        self.preview_filter = filter;
        self.preview_count = count;
    }

    /// Commit the current filter text to history and as the active filter.
    pub fn commit(&mut self, filter: Option<Filter>) -> String {
        let text = self.text.clone();

        // Add to history if non-empty and different from last entry
        if !text.trim().is_empty() && self.history.last().is_none_or(|last| last != &text) {
            self.history.push(text.clone());
            if self.history.len() > MAX_HISTORY_SIZE {
                self.history.remove(0);
            }

            // Persist to disk
            self.persistence.add_to_history(text.clone());
            let _ = self.persistence.save(); // Ignore errors, don't block UX
        }

        self.committed_filter = filter;
        self.history_cursor = None;
        self.history_temp_text = None;
        self.preview_filter = None;
        self.preview_count = None;

        text
    }

    /// Clear the committed filter and reset state.
    pub fn clear(&mut self) {
        self.text.clear();
        self.committed_filter = None;
        self.preview_filter = None;
        self.preview_count = None;
        self.history_cursor = None;
        self.history_temp_text = None;
    }

    /// Navigate up in history (older entries).
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        // Save current text if entering history mode
        if self.history_cursor.is_none() {
            self.history_temp_text = Some(self.text.clone());
            self.history_cursor = Some(self.history.len() - 1);
        } else if let Some(cursor) = self.history_cursor
            && cursor > 0
        {
            self.history_cursor = Some(cursor - 1);
        }

        // Load history text
        if let Some(cursor) = self.history_cursor
            && let Some(entry) = self.history.get(cursor)
        {
            self.text = entry.clone();
            self.last_change = Instant::now();
        }
    }

    /// Navigate down in history (newer entries).
    pub fn history_down(&mut self) {
        if self.history_cursor.is_none() {
            return;
        }

        if let Some(cursor) = self.history_cursor {
            if cursor + 1 < self.history.len() {
                self.history_cursor = Some(cursor + 1);
                if let Some(entry) = self.history.get(cursor + 1) {
                    self.text = entry.clone();
                    self.last_change = Instant::now();
                }
            } else {
                // Reached end of history, restore temp text
                if let Some(temp) = self.history_temp_text.take() {
                    self.text = temp;
                }
                self.history_cursor = None;
                self.last_change = Instant::now();
            }
        }
    }

    /// Get the committed filter text (for display when not editing).
    pub fn committed_text(&self) -> String {
        self.committed_filter
            .as_ref()
            .map(|f| f.source().to_string())
            .unwrap_or_default()
    }

    /// Set an initial filter (for app startup with a filter specified).
    pub fn set_initial_filter(&mut self, text: String, filter: Filter) {
        self.text = text;
        self.committed_filter = Some(filter);
        self.last_change = Instant::now();
    }

    /// Toggle favorite status for the current filter.
    pub fn toggle_favorite(&mut self) {
        let current_filter = self.text.clone();

        if current_filter.trim().is_empty() {
            return;
        }

        if self.persistence.is_favorited(&current_filter) {
            // Remove from favorites
            if let Some(index) = self
                .persistence
                .favorites
                .iter()
                .position(|f| f.filter == current_filter)
            {
                self.persistence.remove_favorite(index);
            }
        } else {
            // Add to favorites (use first 30 chars of filter as name for MVP)
            let name = if current_filter.len() > 30 {
                format!("{}...", &current_filter[..27])
            } else {
                current_filter.clone()
            };

            self.persistence
                .add_favorite(name, current_filter, String::new());
        }

        let _ = self.persistence.save(); // Ignore errors, don't block UX
    }

    /// Check if the current filter is favorited.
    pub fn is_current_favorited(&self) -> bool {
        self.persistence.is_favorited(&self.text)
    }

    /// Get all favorites.
    pub fn get_favorites(&self) -> &[FilterFavorite] {
        &self.persistence.favorites
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_debounce_timing() {
        let mut state = FilterState::new_with_persistence(false);
        state.set_text("test".to_string());

        // Should not update immediately
        assert!(!state.should_update_preview());

        // Wait for debounce
        thread::sleep(Duration::from_millis(110));
        assert!(state.should_update_preview());
    }

    #[test]
    fn test_history_management() {
        let mut state = FilterState::new_with_persistence(false);

        // Commit a few filters
        state.set_text("filter1".to_string());
        state.commit(None);

        state.set_text("filter2".to_string());
        state.commit(None);

        state.set_text("filter3".to_string());
        state.commit(None);

        assert_eq!(state.history.len(), 3);
        assert_eq!(state.history[0], "filter1");
        assert_eq!(state.history[2], "filter3");
    }

    #[test]
    fn test_history_navigation() {
        let mut state = FilterState::new_with_persistence(false);

        // Build history
        state.set_text("first".to_string());
        state.commit(None);
        state.set_text("second".to_string());
        state.commit(None);
        state.set_text("third".to_string());
        state.commit(None);

        // Start with new text
        state.set_text("current".to_string());

        // Navigate up
        state.history_up();
        assert_eq!(state.text, "third");

        state.history_up();
        assert_eq!(state.text, "second");

        state.history_up();
        assert_eq!(state.text, "first");

        // Can't go further back
        state.history_up();
        assert_eq!(state.text, "first");

        // Navigate down
        state.history_down();
        assert_eq!(state.text, "second");

        state.history_down();
        assert_eq!(state.text, "third");

        // Go past end, should restore temp text
        state.history_down();
        assert_eq!(state.text, "current");
        assert!(state.history_cursor.is_none());
    }

    #[test]
    fn test_history_deduplication() {
        let mut state = FilterState::new_with_persistence(false);

        // Commit same filter twice
        state.set_text("filter".to_string());
        state.commit(None);

        state.set_text("filter".to_string());
        state.commit(None);

        // Should only have one entry
        assert_eq!(state.history.len(), 1);
    }

    #[test]
    fn test_history_max_size() {
        let mut state = FilterState::new_with_persistence(false);

        // Add more than MAX_HISTORY_SIZE entries
        for i in 0..60 {
            state.set_text(format!("filter{}", i));
            state.commit(None);
        }

        assert_eq!(state.history.len(), MAX_HISTORY_SIZE);
        // Should have kept the most recent ones
        assert_eq!(state.history.last().unwrap(), "filter59");
    }

    #[test]
    fn test_clear() {
        let mut state = FilterState::new_with_persistence(false);
        state.set_text("test".to_string());
        state.commit(Some(Filter::parse("test == 1").unwrap()));

        state.clear();

        assert!(state.text.is_empty());
        assert!(state.committed_filter.is_none());
        assert!(state.preview_filter.is_none());
    }

    #[test]
    fn test_set_text_clears_history_browsing() {
        let mut state = FilterState::new_with_persistence(false);
        state.set_text("old".to_string());
        state.commit(None);

        // Start browsing history
        state.history_up();
        assert!(state.history_cursor.is_some());

        // Set new text should exit history mode
        state.set_text("new".to_string());
        assert!(state.history_cursor.is_none());
        assert!(state.history_temp_text.is_none());
    }
}
