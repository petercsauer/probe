//! Output formatting utilities.

use anyhow::Result;
use prb_core::DebugEvent;

/// Format events as a table to stdout.
pub fn format_table(events: &[DebugEvent]) {
    if events.is_empty() {
        println!("No events to display");
        return;
    }

    // Print header
    println!(
        "{:<20} {:<10} {:<5} {:<25} METADATA",
        "TIMESTAMP", "TRANSPORT", "DIR", "SOURCE"
    );
    println!("{}", "-".repeat(100));

    // Print rows
    for event in events {
        let timestamp = event.timestamp.to_string();
        let transport = format!("{:?}", event.transport).to_lowercase();
        let direction = match event.direction {
            prb_core::Direction::Inbound => "IN",
            prb_core::Direction::Outbound => "OUT",
            prb_core::Direction::Unknown => "?",
        };
        let source = format!("{}", event.source);

        // Format first metadata entry (if any) for compact display
        let metadata = if let Some((key, value)) = event.metadata.iter().next() {
            format!("{key}={value}")
        } else {
            String::from("-")
        };

        println!("{timestamp:<20} {transport:<10} {direction:<5} {source:<25} {metadata}");
    }
}

/// Format events as pretty-printed JSON to stdout.
pub fn format_json(events: &[DebugEvent]) -> Result<()> {
    let json = serde_json::to_string_pretty(events)?;
    println!("{json}");
    Ok(())
}
