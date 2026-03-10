use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use prb_core::{CaptureAdapter, DebugEvent};
use prb_schema::SchemaRegistry;

use crate::EventStore;

#[derive(Debug, Clone, Copy)]
enum InputFormat {
    Json,
    Pcap,
    Pcapng,
    Mcap,
}

fn detect_format(path: &Path) -> Result<InputFormat> {
    let mut file =
        File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut magic = [0u8; 8];
    let bytes_read = file.read(&mut magic).unwrap_or(0);

    if bytes_read >= 4 {
        match &magic[..4] {
            [0x89, b'M', b'C', b'A'] => return Ok(InputFormat::Mcap),
            [0x0a, 0x0d, 0x0d, 0x0a] => return Ok(InputFormat::Pcapng),
            [0xa1, 0xb2, 0xc3, 0xd4] | [0xd4, 0xc3, 0xb2, 0xa1] => {
                return Ok(InputFormat::Pcap)
            }
            [b'{', ..] | [b'[', ..] => return Ok(InputFormat::Json),
            _ => {}
        }
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => Ok(InputFormat::Json),
        Some("pcap") => Ok(InputFormat::Pcap),
        Some("pcapng") => Ok(InputFormat::Pcapng),
        Some("mcap") => Ok(InputFormat::Mcap),
        _ => bail!("Cannot detect format for {}", path.display()),
    }
}

pub fn load_events(path: &Path) -> Result<EventStore> {
    let format = detect_format(path)?;
    let events = match format {
        InputFormat::Json => load_json(path)?,
        InputFormat::Pcap | InputFormat::Pcapng => load_pcap(path)?,
        InputFormat::Mcap => load_mcap(path)?,
    };
    Ok(EventStore::new(events))
}

fn load_json(path: &Path) -> Result<Vec<DebugEvent>> {
    use prb_fixture::JsonFixtureAdapter;
    let utf8_path = camino::Utf8PathBuf::try_from(path.to_path_buf())
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 path: {}", e))?;
    let mut adapter = JsonFixtureAdapter::new(utf8_path);
    let mut events = Vec::new();
    for result in adapter.ingest() {
        match result {
            Ok(event) => events.push(event),
            Err(e) => tracing::warn!("Skipping event with error: {}", e),
        }
    }
    Ok(events)
}

fn load_pcap(path: &Path) -> Result<Vec<DebugEvent>> {
    use prb_pcap::PcapCaptureAdapter;
    let mut adapter = PcapCaptureAdapter::new(PathBuf::from(path), None);
    let mut events = Vec::new();
    for result in adapter.ingest() {
        match result {
            Ok(event) => events.push(event),
            Err(e) => tracing::warn!("Skipping event with error: {}", e),
        }
    }
    Ok(events)
}

fn load_mcap(path: &Path) -> Result<Vec<DebugEvent>> {
    use prb_storage::SessionReader;
    let reader =
        SessionReader::open(path).with_context(|| format!("Failed to open MCAP {}", path.display()))?;
    let mut events = Vec::new();
    for result in reader.events() {
        match result {
            Ok(event) => events.push(event),
            Err(e) => tracing::warn!("Skipping MCAP event with error: {}", e),
        }
    }
    Ok(events)
}

/// Load schemas from proto files, descriptor sets, and MCAP auto-extraction.
pub fn load_schemas(
    proto_paths: &[PathBuf],
    descriptor_sets: &[PathBuf],
    mcap_path: Option<&Path>,
) -> Result<SchemaRegistry> {
    let mut registry = SchemaRegistry::new();

    // Load descriptor set files
    for path in descriptor_sets {
        tracing::debug!("Loading descriptor set from {}", path.display());
        registry
            .load_descriptor_set_file(path)
            .with_context(|| format!("Failed to load descriptor set {}", path.display()))?;
    }

    // Load and compile proto files
    if !proto_paths.is_empty() {
        tracing::debug!("Compiling {} proto files", proto_paths.len());
        // Use parent directory of first proto as include path
        let includes: Vec<PathBuf> = proto_paths
            .iter()
            .filter_map(|p| p.parent().map(|parent| parent.to_path_buf()))
            .collect();
        registry
            .load_proto_files(proto_paths, &includes)
            .context("Failed to compile proto files")?;
    }

    // Auto-extract schemas from MCAP if provided
    if let Some(path) = mcap_path
        && let Ok(format) = detect_format(path)
        && matches!(format, InputFormat::Mcap)
    {
        tracing::debug!("Attempting to extract schemas from MCAP file");
        if let Err(e) = extract_mcap_schemas(&mut registry, path) {
            tracing::warn!("Failed to extract schemas from MCAP: {}", e);
        }
    }

    Ok(registry)
}

fn extract_mcap_schemas(_registry: &mut SchemaRegistry, path: &Path) -> Result<()> {
    use prb_storage::SessionReader;
    let _reader = SessionReader::open(path)
        .with_context(|| format!("Failed to open MCAP for schema extraction: {}", path.display()))?;

    // Try to extract embedded schemas from the MCAP file
    // SessionReader would need to expose schema extraction functionality
    // For now, we'll just log that we attempted it
    tracing::debug!("MCAP schema extraction not yet implemented in SessionReader");

    Ok(())
}
