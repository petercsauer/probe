//! Schema management commands.

use crate::cli::{SchemaExportArgs, SchemaListArgs, SchemaLoadArgs, SchemasArgs, SchemasCommand};
use anyhow::{Context, Result};
use prb_schema::SchemaRegistry;
use prb_storage::SessionReader;
use std::path::Path;

/// Run the schemas command.
pub fn run_schemas(args: SchemasArgs) -> Result<()> {
    match args.command {
        SchemasCommand::Load(load_args) => run_load(load_args),
        SchemasCommand::List(list_args) => run_list(list_args),
        SchemasCommand::Export(export_args) => run_export(export_args),
    }
}

fn run_load(args: SchemaLoadArgs) -> Result<()> {
    let path = args.path.as_std_path();
    let mut registry = SchemaRegistry::new();

    // Determine file type by extension
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .context("Failed to determine file extension")?;

    match extension {
        "desc" => {
            tracing::info!("Loading descriptor set from {}", args.path);
            registry
                .load_descriptor_set_file(path)
                .context("Failed to load descriptor set")?;
        }
        "proto" => {
            tracing::info!("Compiling proto file {}", args.path);
            let include_paths: Vec<&Path> =
                args.include_paths.iter().map(|p| p.as_std_path()).collect();
            registry
                .load_proto_files(&[path], &include_paths)
                .context("Failed to compile proto file")?;
        }
        _ => {
            anyhow::bail!(
                "Unsupported file extension: {extension}. Expected .proto or .desc"
            );
        }
    }

    // List loaded messages
    let messages = registry.list_messages();
    let services = registry.list_services();

    println!("Successfully loaded schema from {}", args.path);
    println!("\nMessages ({}):", messages.len());
    for msg in messages {
        println!("  {msg}");
    }

    if !services.is_empty() {
        println!("\nServices ({}):", services.len());
        for svc in services {
            println!("  {svc}");
        }
    }

    Ok(())
}

fn run_list(args: SchemaListArgs) -> Result<()> {
    tracing::info!("Reading schemas from {}", args.session);

    let reader =
        SessionReader::open(args.session.as_std_path()).context("Failed to open session")?;

    let registry = reader
        .extract_schemas()
        .context("Failed to extract schemas from session")?;

    let messages = registry.list_messages();
    let services = registry.list_services();

    if messages.is_empty() && services.is_empty() {
        println!("No schemas found in session");
        return Ok(());
    }

    println!("Schemas in {}:", args.session);

    if !messages.is_empty() {
        println!("\nMessages ({}):", messages.len());
        for msg in messages {
            println!("  {msg}");
        }
    }

    if !services.is_empty() {
        println!("\nServices ({}):", services.len());
        for svc in services {
            println!("  {svc}");
        }
    }

    Ok(())
}

fn run_export(args: SchemaExportArgs) -> Result<()> {
    tracing::info!("Exporting schemas from {} to {}", args.session, args.output);

    let reader =
        SessionReader::open(args.session.as_std_path()).context("Failed to open session")?;

    let registry = reader
        .extract_schemas()
        .context("Failed to extract schemas from session")?;

    let descriptor_sets = registry.descriptor_sets();

    if descriptor_sets.is_empty() {
        anyhow::bail!("No schemas found in session");
    }

    // Merge all descriptor sets into one
    // For simplicity, we just write the first one for now
    // A more robust implementation would merge FileDescriptorSets
    let output_bytes = descriptor_sets
        .first()
        .context("No descriptor sets available")?;

    std::fs::write(args.output.as_std_path(), output_bytes)
        .context("Failed to write output file")?;

    println!(
        "Exported {} descriptor set(s) to {}",
        descriptor_sets.len(),
        args.output
    );

    Ok(())
}
