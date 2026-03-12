//! Tests for command palette overlay functionality.

use prb_tui::overlays::CommandPaletteOverlay;

#[test]
fn test_command_palette_creation() {
    let palette = CommandPaletteOverlay::new();

    assert_eq!(palette.input, "");
    assert_eq!(palette.selected, 0);
    assert!(!palette.commands.is_empty());

    // Should have at least the core commands
    assert!(palette.commands.len() >= 9);
}

#[test]
fn test_command_palette_default() {
    let palette = CommandPaletteOverlay::default();

    assert_eq!(palette.input, "");
    assert_eq!(palette.selected, 0);
    assert!(!palette.commands.is_empty());
}

#[test]
fn test_command_palette_update_input() {
    let mut palette = CommandPaletteOverlay::new();

    // Set selection to non-zero
    palette.selected = 3;

    // Update input should reset selection to 0
    palette.update_input("filter".to_string());
    assert_eq!(palette.input, "filter");
    assert_eq!(
        palette.selected, 0,
        "Selection should reset on input change"
    );

    // Change input again
    palette.selected = 2;
    palette.update_input("help".to_string());
    assert_eq!(palette.input, "help");
    assert_eq!(palette.selected, 0, "Selection should reset again");
}

#[test]
fn test_command_palette_filtered_commands_empty_input() {
    let palette = CommandPaletteOverlay::new();

    let filtered = palette.filtered_commands();

    // Empty input should return all commands
    assert_eq!(filtered.len(), palette.commands.len());
}

#[test]
fn test_command_palette_filtered_commands_partial_match() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("filter".to_string());
    let filtered = palette.filtered_commands();

    // Should match "Filter events" and "Clear filter"
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().any(|c| c.name == "Filter events"));
    assert!(filtered.iter().any(|c| c.name == "Clear filter"));
}

#[test]
fn test_command_palette_filtered_commands_case_insensitive() {
    let mut palette = CommandPaletteOverlay::new();

    // Test with lowercase
    palette.update_input("help".to_string());
    let filtered_lower_len = palette.filtered_commands().len();
    assert!(filtered_lower_len > 0);
    assert!(palette.filtered_commands().iter().any(|c| c.name == "Help"));

    // Test with uppercase
    palette.update_input("HELP".to_string());
    let filtered_upper_len = palette.filtered_commands().len();
    assert_eq!(filtered_lower_len, filtered_upper_len);

    // Test with mixed case
    palette.update_input("HeLp".to_string());
    let filtered_mixed_len = palette.filtered_commands().len();
    assert_eq!(filtered_lower_len, filtered_mixed_len);
}

#[test]
fn test_command_palette_filtered_commands_no_match() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("xyznonexistent".to_string());
    let filtered = palette.filtered_commands();

    assert_eq!(filtered.len(), 0, "Should have no matches");
}

#[test]
fn test_command_palette_filtered_commands_match_description() {
    let mut palette = CommandPaletteOverlay::new();

    // Search for "keybinding" which appears in description
    palette.update_input("keybinding".to_string());
    let filtered = palette.filtered_commands();

    assert!(!filtered.is_empty());
    assert!(filtered.iter().any(|c| c.name == "Help"));
}

#[test]
fn test_command_palette_filtered_commands_match_name_and_description() {
    let mut palette = CommandPaletteOverlay::new();

    // "pane" appears in both name ("Next pane", "Previous pane") and description
    palette.update_input("pane".to_string());
    let filtered = palette.filtered_commands();

    assert!(filtered.len() >= 2);
    assert!(filtered.iter().any(|c| c.name == "Next pane"));
    assert!(filtered.iter().any(|c| c.name == "Previous pane"));
}

#[test]
fn test_command_palette_move_selection_within_filtered() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("event".to_string());
    let filtered = palette.filtered_commands();
    let filtered_count = filtered.len();

    assert!(filtered_count > 0, "Should have at least one match");

    // Move down
    palette.selected = 0;
    palette.move_selection(1);
    assert_eq!(palette.selected, 1);

    // Move up
    palette.move_selection(-1);
    assert_eq!(palette.selected, 0);
}

#[test]
fn test_command_palette_move_selection_wrap_around() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("filter".to_string());
    let filtered = palette.filtered_commands();
    let filtered_count = filtered.len();

    // Move backwards from 0 - should wrap to end
    palette.selected = 0;
    palette.move_selection(-1);
    assert_eq!(palette.selected, filtered_count - 1);

    // Move forward from end - should wrap to start
    palette.selected = filtered_count - 1;
    palette.move_selection(1);
    assert_eq!(palette.selected, 0);
}

#[test]
fn test_command_palette_move_selection_with_no_matches() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("xyznonexistent".to_string());
    let filtered = palette.filtered_commands();
    assert_eq!(filtered.len(), 0);

    // Should not panic or change selection
    let original_selected = palette.selected;
    palette.move_selection(1);
    assert_eq!(palette.selected, original_selected);

    palette.move_selection(-1);
    assert_eq!(palette.selected, original_selected);
}

#[test]
fn test_command_palette_selected_command_with_filter() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("filter".to_string());
    palette.selected = 0;

    let selected = palette.selected_command();
    assert!(selected.is_some());

    let cmd = selected.unwrap();
    assert!(cmd.name.to_lowercase().contains("filter"));
}

#[test]
fn test_command_palette_selected_command_no_filter() {
    let palette = CommandPaletteOverlay::new();

    let selected = palette.selected_command();
    assert!(selected.is_some());

    // Should return first command
    assert_eq!(selected.unwrap().name, palette.commands[0].name);
}

#[test]
fn test_command_palette_selected_command_out_of_bounds() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("help".to_string());
    palette.selected = 999;

    let selected = palette.selected_command();
    assert!(selected.is_none());
}

#[test]
fn test_command_palette_selected_command_with_empty_results() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("xyznonexistent".to_string());

    let selected = palette.selected_command();
    assert!(selected.is_none());
}

#[test]
fn test_command_palette_all_commands_have_required_fields() {
    let palette = CommandPaletteOverlay::new();

    for cmd in &palette.commands {
        assert!(!cmd.name.is_empty(), "Command name should not be empty");
        assert!(
            !cmd.description.is_empty(),
            "Command description should not be empty"
        );
        assert!(
            !cmd.key_hint.is_empty(),
            "Command key_hint should not be empty"
        );
    }
}

#[test]
fn test_command_palette_expected_commands_present() {
    let palette = CommandPaletteOverlay::new();

    let command_names: Vec<String> = palette.commands.iter().map(|c| c.name.clone()).collect();

    // Verify expected commands are present
    assert!(command_names.contains(&"Filter events".to_string()));
    assert!(command_names.contains(&"Clear filter".to_string()));
    assert!(command_names.contains(&"Help".to_string()));
    assert!(command_names.contains(&"Next pane".to_string()));
    assert!(command_names.contains(&"Previous pane".to_string()));
    assert!(command_names.contains(&"First event".to_string()));
    assert!(command_names.contains(&"Last event".to_string()));
    assert!(command_names.contains(&"Reload config".to_string()));
    assert!(command_names.contains(&"Quit".to_string()));
}

#[test]
fn test_command_palette_selection_after_multiple_input_changes() {
    let mut palette = CommandPaletteOverlay::new();

    // Change input multiple times
    palette.update_input("help".to_string());
    assert_eq!(palette.selected, 0);

    palette.selected = 5;
    palette.update_input("filter".to_string());
    assert_eq!(palette.selected, 0);

    palette.selected = 1;
    palette.update_input("quit".to_string());
    assert_eq!(palette.selected, 0);
}

#[test]
fn test_command_palette_large_delta_movement() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("event".to_string());
    let filtered = palette.filtered_commands();
    let filtered_count = filtered.len();

    // Large positive delta
    palette.selected = 0;
    palette.move_selection(filtered_count as isize + 2);
    assert_eq!(palette.selected, 2);

    // Large negative delta
    palette.selected = 2;
    palette.move_selection(-(filtered_count as isize + 1));
    assert_eq!(palette.selected, 1);
}

#[test]
fn test_command_palette_filter_single_character() {
    let mut palette = CommandPaletteOverlay::new();

    // Single character should still filter
    palette.update_input("q".to_string());
    let filtered = palette.filtered_commands();

    // Should match "Quit"
    assert!(!filtered.is_empty());
    assert!(filtered.iter().any(|c| c.name == "Quit"));
}

#[test]
fn test_command_palette_filter_exact_match() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("Help".to_string());
    let filtered = palette.filtered_commands();

    assert!(!filtered.is_empty());
    assert!(filtered.iter().any(|c| c.name == "Help"));
}

#[test]
fn test_command_palette_filter_substring_at_word_boundary() {
    let mut palette = CommandPaletteOverlay::new();

    // "event" appears in "Filter events", "First event", "Last event"
    palette.update_input("event".to_string());
    let filtered = palette.filtered_commands();

    assert!(filtered.len() >= 3);
    assert!(filtered.iter().any(|c| c.name == "Filter events"));
    assert!(filtered.iter().any(|c| c.name == "First event"));
    assert!(filtered.iter().any(|c| c.name == "Last event"));
}

#[test]
fn test_command_palette_consecutive_same_input() {
    let mut palette = CommandPaletteOverlay::new();

    palette.update_input("help".to_string());
    let filtered1_len = palette.filtered_commands().len();

    palette.update_input("help".to_string());
    let filtered2_len = palette.filtered_commands().len();

    // Should return same results
    assert_eq!(filtered1_len, filtered2_len);
}
