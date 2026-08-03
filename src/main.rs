mod app;
mod ledger;
mod manifest;
mod pattern;
mod process;
mod state;

use anyhow::{Context, Result, ensure};
use app::{App, NewPatchArgs};
use clap::{Args, Parser, Subcommand};
use manifest::PatchKind;
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
    /// Explain repository, stack, verification, and pending operation state.
    Status {
        /// Emit stable machine-readable JSON without ANSI color.
        #[arg(long)]
        json: bool,
    },
    /// Create and position a documented empty downstream patch.
    New(NewArgs),
    /// Verify stack identity, audit metadata, exports, and source contracts.
    Verify,
    /// Rebase onto an explicit upstream branch, tag, or commit.
    Rebase {
        /// Upstream ref or commit to fetch and use as the new stack base.
        #[arg(long)]
        onto: String,
    },
    /// Publish the verified branch and recovery tag with an exact lease.
    Publish,
    /// Print the repository and agent workflow contract.
    Instructions,
}

#[derive(Args)]
struct NewArgs {
    /// Finish the pending patch after implementation and refresh generated exports.
    #[arg(long)]
    finish: bool,
    /// Unique `StGit` patch name.
    name: Option<String>,
    /// Whether the patch changes source or downstream tooling.
    #[arg(long, value_enum)]
    kind: Option<PatchKind>,
    /// Why the downstream patch exists.
    #[arg(long)]
    purpose: Option<String>,
    /// Current upstream disposition.
    #[arg(long)]
    upstream_status: Option<String>,
    /// Objective condition under which the patch is removed.
    #[arg(long)]
    drop_when: Option<String>,
    /// Allowed repository path or non-directory-crossing glob.
    #[arg(long = "path")]
    paths: Vec<String>,
    /// Optional persisted `StGit` export path for source reconstruction.
    #[arg(long)]
    export: Option<String>,
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
        Operation::Status { json } => app.status(json),
        Operation::New(args) if args.finish => {
            ensure!(
                args.name.is_none()
                    && args.kind.is_none()
                    && args.purpose.is_none()
                    && args.upstream_status.is_none()
                    && args.drop_when.is_none()
                    && args.paths.is_empty()
                    && args.export.is_none(),
                "--finish does not accept patch creation arguments"
            );
            app.finish_new()
        }
        Operation::New(args) => app.new_patch(NewPatchArgs {
            name: args.name.context("patch name is required")?,
            kind: args.kind.context("--kind is required")?,
            purpose: args.purpose.context("--purpose is required")?,
            upstream_status: args
                .upstream_status
                .context("--upstream-status is required")?,
            drop_when: args.drop_when.context("--drop-when is required")?,
            paths: {
                ensure!(!args.paths.is_empty(), "at least one --path is required");
                args.paths
            },
            export: args.export,
        }),
        Operation::Verify => app.verify(),
        Operation::Rebase { onto } => app.rebase(&onto),
        Operation::Publish => app.publish(),
        Operation::Instructions => unreachable!("handled before loading repository state"),
    }
}
