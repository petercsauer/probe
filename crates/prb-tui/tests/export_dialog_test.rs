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

    let format_names: Vec<String> = dialog.formats.iter()
        .map(|f| f.format.clone())
        .collect();

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
    let json_all_format = dialog.formats.iter()
        .find(|f| f.format == "json-all")
        .expect("Should have json-all format");

    assert!(json_all_format.description.contains("42"),
        "Description should include filtered count");

    let csv_format = dialog.formats.iter()
        .find(|f| f.format == "csv")
        .expect("Should have CSV format");

    assert!(csv_format.description.contains("42"),
        "CSV description should include filtered count");
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
