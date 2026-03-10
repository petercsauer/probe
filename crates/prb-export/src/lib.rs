mod error;
mod csv_export;
mod har_export;
mod html_export;
mod otlp_export;
mod otlp_import;
mod merge;
#[cfg(feature = "parquet")]
mod parquet_export;

pub use error::ExportError;
pub use csv_export::CsvExporter;
pub use har_export::HarExporter;
pub use html_export::HtmlExporter;
pub use otlp_export::OtlpExporter;
pub use otlp_import::{parse_otlp_json, otlp_to_events, ExportTraceServiceRequest};
pub use merge::{merge_traces_with_packets, MergedEvent, SpanSummary};
#[cfg(feature = "parquet")]
pub use parquet_export::ParquetExporter;

use prb_core::DebugEvent;
use std::io::Write;

pub trait Exporter {
    fn format_name(&self) -> &'static str;
    fn file_extension(&self) -> &'static str;
    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError>;
}

pub fn create_exporter(format: &str) -> Result<Box<dyn Exporter>, ExportError> {
    match format {
        "csv" => Ok(Box::new(CsvExporter)),
        "har" => Ok(Box::new(HarExporter)),
        "otlp" => Ok(Box::new(OtlpExporter)),
        "html" => Ok(Box::new(HtmlExporter)),
        #[cfg(feature = "parquet")]
        "parquet" => Ok(Box::new(ParquetExporter)),
        other => Err(ExportError::UnsupportedFormat(other.to_string())),
    }
}

pub fn supported_formats() -> Vec<&'static str> {
    #[allow(unused_mut)]
    let mut formats = vec!["csv", "har", "otlp", "html"];
    #[cfg(feature = "parquet")]
    formats.push("parquet");
    formats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_registry() {
        // Test that all formats create correct exporters
        let csv = create_exporter("csv").unwrap();
        assert_eq!(csv.format_name(), "csv");
        assert_eq!(csv.file_extension(), "csv");

        let har = create_exporter("har").unwrap();
        assert_eq!(har.format_name(), "har");
        assert_eq!(har.file_extension(), "har");

        let otlp = create_exporter("otlp").unwrap();
        assert_eq!(otlp.format_name(), "otlp");
        assert_eq!(otlp.file_extension(), "json");

        let html = create_exporter("html").unwrap();
        assert_eq!(html.format_name(), "html");
        assert_eq!(html.file_extension(), "html");

        #[cfg(feature = "parquet")]
        {
            let parquet = create_exporter("parquet").unwrap();
            assert_eq!(parquet.format_name(), "parquet");
            assert_eq!(parquet.file_extension(), "parquet");
        }

        // Test unsupported format
        assert!(create_exporter("unknown").is_err());
    }

    #[test]
    fn supported_formats_list() {
        let formats = supported_formats();
        assert!(formats.contains(&"csv"));
        assert!(formats.contains(&"har"));
        assert!(formats.contains(&"otlp"));
        assert!(formats.contains(&"html"));

        #[cfg(feature = "parquet")]
        assert!(formats.contains(&"parquet"));

        #[cfg(not(feature = "parquet"))]
        assert_eq!(formats.len(), 4);

        #[cfg(feature = "parquet")]
        assert_eq!(formats.len(), 5);
    }
}
