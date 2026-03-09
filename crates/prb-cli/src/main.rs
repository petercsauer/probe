//! PRB CLI entry point.

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "prb")]
#[command(about = "Universal message debugger for gRPC, ZMTP, and DDS-RTPS")]
struct Cli {
    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let _cli = Cli::parse();
    println!("prb: universal message debugger");
    Ok(())
}
