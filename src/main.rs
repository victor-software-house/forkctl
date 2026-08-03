mod app;
mod manifest;
mod pattern;
mod process;

use anyhow::Result;
use app::App;
use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;

const DEFAULT_MANIFEST: &str = "patches/fork.json";
const INSTRUCTIONS: &str = include_str!("instructions.md");

#[derive(Parser)]
#[command(version, about = "Maintain a declared StGit downstream patch stack")]
struct Cli {
    #[arg(long, global = true)]
    manifest: Option<PathBuf>,

    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    /// Reconstruct `StGit` metadata after cloning a fork.
    Init,
    /// Verify stack identity, allowed drift, exports, and source contracts.
    Verify,
    /// Rebase onto upstream and refresh exports and base pins.
    Rebase,
    /// Print the repository and agent workflow contract.
    Instructions,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if matches!(&cli.operation, Operation::Instructions) {
        print!("{INSTRUCTIONS}");
        return Ok(());
    }

    let manifest = cli
        .manifest
        .or_else(|| env::var_os("FORK_MANIFEST").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MANIFEST));
    let mut app = App::load(&manifest)?;
    match cli.operation {
        Operation::Init => app.init(),
        Operation::Verify => app.verify(),
        Operation::Rebase => app.rebase(),
        Operation::Instructions => unreachable!("handled before loading repository state"),
    }
}
