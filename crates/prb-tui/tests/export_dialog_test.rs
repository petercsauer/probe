//! Tests for export dialog overlay functionality.

use prb_tui::overlays::ExportDialogOverlay;

#[test]
fn test_export_dialog_creation() {
    let dialog = ExportDialogOverlay::new(10);

    assert_eq!(dialog.filtered_count, 10);
    assert_eq!(dialog.selected, 0);
    assert!(!dialog.editing_path);

    // Should have at least 6 formats (json, json-all, csv, har, otlp, html)
    assert!(dialog.formats.len() >= 6);
}

#[test]
fn test_export_dialog_format_selection() {
    let mut dialog = ExportDialogOverlay::new(5);

    // Initial selection
    assert_eq!(dialog.selected, 0);
    let format = dialog.selected_format().unwrap();
    assert_eq!(format.format, "json");

    // Move down
    dialog.move_selection(1);
    assert_eq!(dialog.selected, 1);
    let format = dialog.selected_format().unwrap();
    assert_eq!(format.format, "json-all");

    // Move down multiple times
    dialog.move_selection(2);
    assert_eq!(dialog.selected, 3);
    let format = dialog.selected_format().unwrap();
    assert_eq!(format.format, "har");

    // Move up
    dialog.move_selection(-1);
    assert_eq!(dialog.selected, 2);
    let format = dialog.selected_format().unwrap();
    assert_eq!(format.format, "csv");

    // Wrap around (move beyond end)
    let last_idx = dialog.formats.len() - 1;
    dialog.selected = last_idx;
    dialog.move_selection(1);
    assert_eq!(dialog.selected, 0, "Should wrap around to start");

    // Wrap around (move before start)
    dialog.selected = 0;
    dialog.move_selection(-1);
    assert_eq!(dialog.selected, last_idx, "Should wrap around to end");
}

#[test]
fn test_export_dialog_path_editing() {
    let mut dialog = ExportDialogOverlay::new(5);

    // Initially not editing
    assert!(!dialog.editing_path);

    // Toggle editing
    dialog.toggle_path_editing();
    assert!(dialog.editing_path);

    // Toggle back
    dialog.toggle_path_editing();
    assert!(!dialog.editing_path);
}

#[test]
fn test_export_dialog_path_updates_with_format() {
    let mut dialog = ExportDialogOverlay::new(5);

    // Initial path for json
    assert_eq!(dialog.output_path_input.value(), "./export.json");

    // Change to CSV (index 2)
    dialog.move_selection(2);
    assert_eq!(dialog.output_path_input.value(), "./export.csv");

    // Change to HAR (index 3)
    dialog.move_selection(1);
    assert_eq!(dialog.output_path_input.value(), "./export.har");

    // Change to HTML (index 5)
    dialog.move_selection(2);
    assert_eq!(dialog.output_path_input.value(), "./export.html");
}

#[test]
fn test_export_dialog_formats_include_all_supported() {
    let dialog = ExportDialogOverlay::new(100);

    let format_names: Vec<String> = dialog.formats.iter().map(|f| f.format.clone()).collect();

    // Check that all expected formats are present
    assert!(format_names.contains(&"json".to_string()));
    assert!(format_names.contains(&"json-all".to_string()));
    assert!(format_names.contains(&"csv".to_string()));
    assert!(format_names.contains(&"har".to_string()));
    assert!(format_names.contains(&"otlp".to_string()));
    assert!(format_names.contains(&"html".to_string()));

    // Parquet is feature-gated
    #[cfg(feature = "parquet")]
    assert!(format_names.contains(&"parquet".to_string()));
}

#[test]
fn test_export_dialog_format_descriptions() {
    let dialog = ExportDialogOverlay::new(42);

    // Verify format descriptions contain expected text
    let json_all_format = dialog
        .formats
        .iter()
        .find(|f| f.format == "json-all")
        .expect("Should have json-all format");

    assert!(
        json_all_format.description.contains("42"),
        "Description should include filtered count"
    );

    let csv_format = dialog
        .formats
        .iter()
        .find(|f| f.format == "csv")
        .expect("Should have CSV format");

    assert!(
        csv_format.description.contains("42"),
        "CSV description should include filtered count"
    );
}

#[test]
fn test_export_dialog_format_extensions() {
    let dialog = ExportDialogOverlay::new(5);

    // Verify correct file extensions
    let json_format = dialog.formats.iter().find(|f| f.format == "json").unwrap();
    assert_eq!(json_format.extension, "json");

    let csv_format = dialog.formats.iter().find(|f| f.format == "csv").unwrap();
    assert_eq!(csv_format.extension, "csv");

    let har_format = dialog.formats.iter().find(|f| f.format == "har").unwrap();
    assert_eq!(har_format.extension, "har");

    let otlp_format = dialog.formats.iter().find(|f| f.format == "otlp").unwrap();
    assert_eq!(otlp_format.extension, "json");

    let html_format = dialog.formats.iter().find(|f| f.format == "html").unwrap();
    assert_eq!(html_format.extension, "html");
}

#[test]
fn test_export_dialog_path_not_updated_when_editing() {
    let mut dialog = ExportDialogOverlay::new(5);

    // Start editing path
    dialog.toggle_path_editing();
    assert!(dialog.editing_path);

    // Modify the path manually
    dialog.output_path_input =
        tui_input::Input::default().with_value("/custom/path.json".to_string());

    // Move selection - path should NOT update because we're editing
    let original_path = dialog.output_path_input.value().to_string();
    dialog.move_selection(1);
    assert_eq!(
        dialog.output_path_input.value(),
        original_path,
        "Path should not change when editing"
    );

    // Move to CSV format
    dialog.move_selection(1);
    assert_eq!(
        dialog.output_path_input.value(),
        original_path,
        "Path should still not change when editing"
    );

    // Stop editing
    dialog.toggle_path_editing();
    assert!(!dialog.editing_path);

    // Now path should update on format change
    dialog.move_selection(1);
    assert_ne!(
        dialog.output_path_input.value(),
        original_path,
        "Path should change after editing stops"
    );
}

#[test]
fn test_export_dialog_default() {
    let dialog = ExportDialogOverlay::default();

    assert_eq!(dialog.filtered_count, 0);
    assert_eq!(dialog.selected, 0);
    assert!(!dialog.editing_path);
    assert!(dialog.formats.len() >= 6);
}

#[test]
fn test_export_dialog_selected_format_out_of_bounds() {
    let mut dialog = ExportDialogOverlay::new(5);

    // Manually set selected to out of bounds
    dialog.selected = 999;

    // selected_format should return None
    assert!(dialog.selected_format().is_none());
}

#[test]
fn test_export_dialog_move_selection_with_empty_formats() {
    let mut dialog = ExportDialogOverlay::new(5);

    // Manually clear formats to test edge case
    dialog.formats.clear();

    // Should not panic or change selection
    let original_selected = dialog.selected;
    dialog.move_selection(1);
    assert_eq!(dialog.selected, original_selected);

    dialog.move_selection(-1);
    assert_eq!(dialog.selected, original_selected);
}

#[test]
fn test_export_dialog_wrap_around_boundary_conditions() {
    let mut dialog = ExportDialogOverlay::new(5);
    let num_formats = dialog.formats.len();

    // Start at 0, move backwards - should wrap to end
    dialog.selected = 0;
    dialog.move_selection(-1);
    assert_eq!(dialog.selected, num_formats - 1);

    // Move forward from end - should wrap to start
    dialog.selected = num_formats - 1;
    dialog.move_selection(1);
    assert_eq!(dialog.selected, 0);

    // Large positive delta
    dialog.selected = 0;
    dialog.move_selection(num_formats as isize + 2);
    assert_eq!(dialog.selected, 2);

    // Large negative delta
    dialog.selected = 2;
    dialog.move_selection(-(num_formats as isize + 1));
    assert_eq!(dialog.selected, 1);
}

#[test]
fn test_export_dialog_multiple_filtered_counts() {
    let dialog_zero = ExportDialogOverlay::new(0);
    let dialog_one = ExportDialogOverlay::new(1);
    let dialog_large = ExportDialogOverlay::new(9999);

    // Check that filtered count is reflected in descriptions
    let json_all_zero = dialog_zero
        .formats
        .iter()
        .find(|f| f.format == "json-all")
        .unwrap();
    assert!(json_all_zero.description.contains("0"));

    let json_all_one = dialog_one
        .formats
        .iter()
        .find(|f| f.format == "json-all")
        .unwrap();
    assert!(json_all_one.description.contains("1"));

    let json_all_large = dialog_large
        .formats
        .iter()
        .find(|f| f.format == "json-all")
        .unwrap();
    assert!(json_all_large.description.contains("9999"));
}

#[test]
fn test_export_dialog_input_value_persistence() {
    let mut dialog = ExportDialogOverlay::new(5);

    // Initially should have default path
    assert_eq!(dialog.output_path_input.value(), "./export.json");

    // Manually set custom value
    dialog.output_path_input =
        tui_input::Input::default().with_value("/tmp/custom.json".to_string());
    assert_eq!(dialog.output_path_input.value(), "/tmp/custom.json");

    // Without editing mode, format change should reset it
    dialog.move_selection(2); // to CSV
    assert_eq!(dialog.output_path_input.value(), "./export.csv");
}

#[test]
fn test_export_dialog_consecutive_toggles() {
    let mut dialog = ExportDialogOverlay::new(5);

    assert!(!dialog.editing_path);

    // Multiple toggles
    for i in 0..10 {
        dialog.toggle_path_editing();
        assert_eq!(dialog.editing_path, i % 2 == 0);
    }
}
