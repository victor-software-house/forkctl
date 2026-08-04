use crate::manifest::{PatchKind, RequiredText};
use crate::protocol::{
    ApiRequest, CaptureSource, CheckArgs, CheckScope, ColorMode, ContractEditArgs, EmptyArgs,
    ExecutionMode, InitArgs, OperationAbortArgs, OutputFormat, PatchCreateArgs, PatchEditArgs,
    PatchName, PatchRefreshArgs, PatchTarget, RebaseArgs, SchemaKind, ScopeEdit,
};
use anyhow::{Context, Result, ensure};
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Maintain an explicit audited StGit downstream patch stack",
    arg_required_else_help = false
)]
pub struct Cli {
    /// Manifest path; defaults to `FORK_MANIFEST` then `patches/fork.json`.
    #[arg(short = 'm', long, global = true, value_hint = clap::ValueHint::FilePath, help_heading = "Output")]
    pub manifest: Option<PathBuf>,

    /// Output representation.
    #[arg(
        short = 'f',
        long = "format",
        global = true,
        value_enum,
        default_value = "pretty",
        help_heading = "Output"
    )]
    pub output: OutputFormat,

    /// Pretty-output color policy.
    #[arg(
        short = 'c',
        long,
        global = true,
        value_enum,
        default_value = "auto",
        help_heading = "Output"
    )]
    pub color: ColorMode,

    /// Suppress successful pretty output.
    #[arg(short = 'q', long, global = true, help_heading = "Output")]
    pub quiet: bool,

    #[arg(
        long,
        global = true,
        hide = true,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "forkctl"
    )]
    pub usage_spec: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Bootstrap a new contract or hydrate a fresh clone.
    Init(InitCliArgs),
    /// Inspect repository, patch, worktree, check, and operation state.
    Status,
    /// Check the complete repository or the staged index.
    Check(CheckCliArgs),
    /// Create, inspect, select, edit, refresh, or finish patches.
    Patch {
        #[command(subcommand)]
        command: PatchCommand,
    },
    /// Replay the declared stack onto an exact upstream target.
    Rebase(RebaseCliArgs),
    /// Edit declarative repository contracts.
    Contract {
        #[command(subcommand)]
        command: ContractCommand,
    },
    /// Atomically publish the branch and recovery tag under an exact lease.
    Publish(DryRunArgs),
    /// Inspect, continue, or abort the current operation.
    Operation {
        #[command(subcommand)]
        command: OperationCommand,
    },
    /// Print the generated operator and repository contract.
    Instructions,
    /// Generate shell completion registration.
    Completion {
        /// Shell whose registration script should be emitted.
        shell: CompletionShell,
    },
    /// Use the versioned local JSON API.
    Api {
        #[command(subcommand)]
        command: ApiCommand,
    },
    #[command(hide = true, name = "__candidates")]
    Candidates {
        #[arg(value_enum)]
        kind: crate::completion::CandidateKind,
    },
}

#[derive(Args, Default)]
pub struct DryRunArgs {
    /// Validate and show the mutation plan without writes, hooks, or ref updates.
    #[arg(short = 'n', long, help_heading = "Execution")]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct InitCliArgs {
    /// Fetch-only upstream remote name.
    #[arg(long, help_heading = "Repository", add = crate::completion::remote_completer())]
    pub upstream_remote: Option<String>,
    /// Exact upstream repository URL.
    #[arg(short = 'u', long, help_heading = "Repository")]
    pub upstream_url: Option<String>,
    /// Full upstream branch ref, such as refs/heads/main.
    #[arg(long, help_heading = "Repository")]
    pub upstream_ref: Option<String>,
    /// Downstream publication remote name.
    #[arg(long, help_heading = "Repository", add = crate::completion::remote_completer())]
    pub downstream_remote: Option<String>,
    /// Managed downstream branch name.
    #[arg(long, help_heading = "Repository")]
    pub downstream_branch: Option<String>,
    /// Initial full branch/tag ref or commit SHA.
    #[arg(short = 'b', long, help_heading = "Repository", add = crate::completion::ref_completer())]
    pub base: Option<String>,
    /// Generated patch ledger path.
    #[arg(short = 'l', long, help_heading = "Documents", value_hint = clap::ValueHint::FilePath)]
    pub ledger: Option<String>,
    /// Generated source-patch export directory.
    #[arg(short = 'e', long, help_heading = "Documents", value_hint = clap::ValueHint::DirPath)]
    pub exports: Option<String>,
    /// Final tooling patch name.
    #[arg(short = 'k', long, help_heading = "Bookkeeping")]
    pub bookkeeping_patch: Option<String>,
    /// Additional bookkeeping ownership scope; repeatable.
    #[arg(short = 'p', long = "bookkeeping-path", help_heading = "Bookkeeping", value_hint = clap::ValueHint::AnyPath)]
    pub bookkeeping_scope: Vec<String>,
    /// Base-drift ownership glob allowed below the patch stack; repeatable.
    #[arg(short = 'a', long, help_heading = "Contracts")]
    pub allow_base: Vec<String>,
    /// Required repository text as PATH=TEXT; repeatable.
    #[arg(short = 'r', long, help_heading = "Contracts", value_parser = parse_required_text)]
    pub required_text: Vec<RequiredText>,
    #[command(flatten)]
    pub execution: DryRunArgs,
}

#[derive(Args)]
pub struct CheckCliArgs {
    /// Check the staged index against an explicit or active patch.
    #[arg(short = 's', long, help_heading = "Check scope")]
    pub staged: bool,
    /// Patch used by staged check; defaults to the active patch.
    #[arg(short = 'p', long, requires = "staged", help_heading = "Check scope", add = crate::completion::patch_completer())]
    pub patch: Option<String>,
}

#[derive(Subcommand)]
pub enum PatchCommand {
    /// List the declared series and active patch.
    List,
    /// Show one patch; defaults to the active patch.
    Show {
        /// Patch name; defaults to the active patch.
        #[arg(add = crate::completion::patch_completer())]
        name: Option<String>,
    },
    /// Record a metadata-only active patch draft.
    Create(PatchCreateCliArgs),
    /// Select an existing patch locally.
    Select {
        /// Existing patch to select.
        #[arg(add = crate::completion::patch_completer())]
        name: String,
        #[command(flatten)]
        execution: DryRunArgs,
    },
    /// Edit metadata or ownership scope.
    Edit(PatchEditCliArgs),
    /// Capture staged, owned, or path-limited changes.
    Refresh(PatchRefreshCliArgs),
    /// Run the full check and clear active state.
    Finish {
        /// Patch name; defaults to the active patch.
        #[arg(add = crate::completion::patch_completer())]
        name: Option<String>,
        #[command(flatten)]
        execution: DryRunArgs,
    },
}

#[derive(Args)]
pub struct PatchCreateCliArgs {
    /// Unique patch name.
    pub name: String,
    /// Patch layer.
    #[arg(short = 'k', long, value_enum, help_heading = "Patch metadata")]
    pub kind: PatchKind,
    /// Why this downstream patch exists.
    #[arg(short = 'p', long, help_heading = "Patch metadata")]
    pub purpose: String,
    /// Current upstream disposition.
    #[arg(short = 'u', long, help_heading = "Patch metadata")]
    pub upstream_status: String,
    /// Objective condition for removing this patch.
    #[arg(short = 'd', long, help_heading = "Patch metadata")]
    pub drop_when: String,
    /// Persistent ownership glob; repeatable.
    #[arg(short = 's', long, required = true, help_heading = "Ownership")]
    pub scope: Vec<String>,
    #[command(flatten)]
    pub execution: DryRunArgs,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("scope_edit")
        .args(["set_scope", "add_scope", "remove_scope"])
        .multiple(true)
))]
pub struct PatchEditCliArgs {
    /// Patch name; defaults to the active patch.
    #[arg(add = crate::completion::patch_completer())]
    pub name: Option<String>,
    /// Replacement patch layer.
    #[arg(short = 'k', long, value_enum, help_heading = "Patch metadata")]
    pub kind: Option<PatchKind>,
    /// Replacement downstream purpose.
    #[arg(short = 'p', long, help_heading = "Patch metadata")]
    pub purpose: Option<String>,
    /// Replacement upstream disposition.
    #[arg(short = 'u', long, help_heading = "Patch metadata")]
    pub upstream_status: Option<String>,
    /// Objective condition for removing this patch.
    #[arg(short = 'd', long, help_heading = "Patch metadata")]
    pub drop_when: Option<String>,
    /// Replace complete ownership scope; repeatable.
    #[arg(short = 's', long, help_heading = "Ownership", conflicts_with_all = ["add_scope", "remove_scope"])]
    pub set_scope: Vec<String>,
    /// Add ownership globs; repeatable.
    #[arg(
        short = 'a',
        long,
        help_heading = "Ownership",
        conflicts_with = "set_scope"
    )]
    pub add_scope: Vec<String>,
    /// Remove exact ownership globs; repeatable.
    #[arg(
        short = 'r',
        long,
        help_heading = "Ownership",
        conflicts_with = "set_scope"
    )]
    pub remove_scope: Vec<String>,
    #[command(flatten)]
    pub execution: DryRunArgs,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("capture")
        .args(["staged", "all", "paths"])
        .multiple(false)
))]
pub struct PatchRefreshCliArgs {
    /// Patch name; defaults to the active patch.
    #[arg(add = crate::completion::patch_completer())]
    pub name: Option<String>,
    /// Explicitly select staged-index capture (the default).
    #[arg(short = 's', long, help_heading = "Capture")]
    pub staged: bool,
    /// Stage and capture every changed path owned by the patch.
    #[arg(short = 'a', long, help_heading = "Capture")]
    pub all: bool,
    /// Stage and capture an explicit Git pathspec; repeatable.
    #[arg(short = 'p', long = "path", help_heading = "Capture", value_hint = clap::ValueHint::AnyPath)]
    pub paths: Vec<String>,
    #[command(flatten)]
    pub execution: DryRunArgs,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("contract_change")
        .args(["clear", "allow_base", "required_text"])
        .required(true)
        .multiple(true)
))]
pub struct ContractEditCliArgs {
    /// Clear all existing contracts before adding the supplied values.
    #[arg(long, help_heading = "Contracts")]
    pub clear: bool,
    /// Add an allowed base-drift ownership glob; repeatable.
    #[arg(short = 'a', long, help_heading = "Contracts")]
    pub allow_base: Vec<String>,
    /// Add a required repository assertion as PATH=TEXT; repeatable.
    #[arg(short = 'r', long, help_heading = "Contracts", value_parser = parse_required_text)]
    pub required_text: Vec<RequiredText>,
    #[command(flatten)]
    pub execution: DryRunArgs,
}

#[derive(Subcommand)]
pub enum ContractCommand {
    /// Append contracts or clear and replace the complete contract set.
    Edit(ContractEditCliArgs),
}

#[derive(Args)]
pub struct RebaseCliArgs {
    /// Full upstream branch/tag ref or commit SHA.
    #[arg(short = 'o', long, help_heading = "Target", add = crate::completion::ref_completer())]
    pub onto: String,
    #[command(flatten)]
    pub execution: DryRunArgs,
}

#[derive(Subcommand)]
pub enum OperationCommand {
    /// Show the current operation and exact next actions.
    Status,
    /// Resume after operator conflict resolution.
    Continue(DryRunArgs),
    /// Restore the recorded old state.
    Abort {
        /// Confirm destructive restoration of the recorded old state.
        #[arg(short = 'y', long)]
        yes: bool,
        #[command(flatten)]
        execution: DryRunArgs,
    },
}

#[derive(Subcommand)]
pub enum ApiCommand {
    /// Emit JSON Schema 2020-12.
    Schema {
        /// Schema document to emit.
        #[arg(short = 'k', long, value_enum, default_value = "bundle")]
        kind: SchemaKind,
    },
    /// Read one invocation from stdin and emit one response.
    Call,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Nu,
    Powershell,
    Zsh,
}

pub enum CliAction {
    Request {
        request: Box<ApiRequest>,
        mode: ExecutionMode,
    },
    ApiSchema(SchemaKind),
    ApiCall,
    Completion(CompletionShell),
    Candidates(crate::completion::CandidateKind),
    UsageSpec(String),
}

impl Cli {
    pub fn into_action(self) -> Result<CliAction> {
        if let Some(bin) = self.usage_spec {
            return Ok(CliAction::UsageSpec(bin));
        }
        let command = self.command.context("a command is required")?;
        let action = match command {
            Command::Init(args) => CliAction::Request {
                request: Box::new(ApiRequest::Init(InitArgs {
                    upstream_remote: args.upstream_remote,
                    upstream_url: args.upstream_url,
                    upstream_ref: args.upstream_ref,
                    downstream_remote: args.downstream_remote,
                    downstream_branch: args.downstream_branch,
                    base: args.base,
                    ledger: args.ledger,
                    exports: args.exports,
                    bookkeeping_patch: args.bookkeeping_patch,
                    bookkeeping_scope: args.bookkeeping_scope,
                    allow_base: args.allow_base,
                    required_text: args.required_text,
                })),
                mode: mode(args.execution.dry_run),
            },
            Command::Status => request(ApiRequest::Status(EmptyArgs::default())),
            Command::Check(args) => request(ApiRequest::Check(CheckArgs {
                scope: if args.staged {
                    CheckScope::Staged
                } else {
                    CheckScope::Repository
                },
                patch: args.patch,
            })),
            Command::Patch { command } => patch_action(command)?,
            Command::Rebase(args) => CliAction::Request {
                request: Box::new(ApiRequest::Rebase(RebaseArgs { onto: args.onto })),
                mode: mode(args.execution.dry_run),
            },
            Command::Contract { command } => match command {
                ContractCommand::Edit(args) => CliAction::Request {
                    request: Box::new(ApiRequest::ContractEdit(ContractEditArgs {
                        clear: args.clear,
                        allow_base: args.allow_base,
                        required_text: args.required_text,
                    })),
                    mode: mode(args.execution.dry_run),
                },
            },
            Command::Publish(args) => CliAction::Request {
                request: Box::new(ApiRequest::Publish(EmptyArgs::default())),
                mode: mode(args.dry_run),
            },
            Command::Operation { command } => operation_action(command),
            Command::Instructions => request(ApiRequest::Instructions(EmptyArgs::default())),
            Command::Completion { shell } => CliAction::Completion(shell),
            Command::Api { command } => match command {
                ApiCommand::Schema { kind } => CliAction::ApiSchema(kind),
                ApiCommand::Call => CliAction::ApiCall,
            },
            Command::Candidates { kind } => CliAction::Candidates(kind),
        };
        ensure!(
            !(self.quiet && self.output == OutputFormat::Json),
            "--quiet conflicts with --format json"
        );
        Ok(action)
    }
}

fn patch_action(command: PatchCommand) -> Result<CliAction> {
    Ok(match command {
        PatchCommand::List => request(ApiRequest::PatchList(EmptyArgs::default())),
        PatchCommand::Show { name } => request(ApiRequest::PatchShow(PatchTarget { patch: name })),
        PatchCommand::Create(args) => CliAction::Request {
            request: Box::new(ApiRequest::PatchCreate(PatchCreateArgs {
                name: args.name,
                kind: args.kind,
                purpose: args.purpose,
                upstream_status: args.upstream_status,
                drop_when: args.drop_when,
                scope: args.scope,
            })),
            mode: mode(args.execution.dry_run),
        },
        PatchCommand::Select { name, execution } => CliAction::Request {
            request: Box::new(ApiRequest::PatchSelect(PatchName { patch: name })),
            mode: mode(execution.dry_run),
        },
        PatchCommand::Edit(args) => {
            ensure!(
                args.kind.is_some()
                    || args.purpose.is_some()
                    || args.upstream_status.is_some()
                    || args.drop_when.is_some()
                    || !args.set_scope.is_empty()
                    || !args.add_scope.is_empty()
                    || !args.remove_scope.is_empty(),
                "patch edit requires at least one metadata or scope change"
            );
            let scope = if args.set_scope.is_empty() {
                (!args.add_scope.is_empty() || !args.remove_scope.is_empty()).then_some(
                    ScopeEdit::AddRemove {
                        add: args.add_scope,
                        remove: args.remove_scope,
                    },
                )
            } else {
                Some(ScopeEdit::Set {
                    patterns: args.set_scope,
                })
            };
            CliAction::Request {
                request: Box::new(ApiRequest::PatchEdit(PatchEditArgs {
                    patch: args.name,
                    kind: args.kind,
                    purpose: args.purpose,
                    upstream_status: args.upstream_status,
                    drop_when: args.drop_when,
                    scope,
                })),
                mode: mode(args.execution.dry_run),
            }
        }
        PatchCommand::Refresh(args) => {
            let capture = if args.all {
                CaptureSource::All
            } else if args.paths.is_empty() {
                CaptureSource::Staged
            } else {
                CaptureSource::Paths {
                    pathspecs: args.paths,
                }
            };
            CliAction::Request {
                request: Box::new(ApiRequest::PatchRefresh(PatchRefreshArgs {
                    patch: args.name,
                    capture,
                })),
                mode: mode(args.execution.dry_run),
            }
        }
        PatchCommand::Finish { name, execution } => CliAction::Request {
            request: Box::new(ApiRequest::PatchFinish(PatchTarget { patch: name })),
            mode: mode(execution.dry_run),
        },
    })
}

fn operation_action(command: OperationCommand) -> CliAction {
    match command {
        OperationCommand::Status => request(ApiRequest::OperationStatus(EmptyArgs::default())),
        OperationCommand::Continue(args) => CliAction::Request {
            request: Box::new(ApiRequest::OperationContinue(EmptyArgs::default())),
            mode: mode(args.dry_run),
        },
        OperationCommand::Abort { yes, execution } => CliAction::Request {
            request: Box::new(ApiRequest::OperationAbort(OperationAbortArgs {
                confirmed: yes,
            })),
            mode: mode(execution.dry_run),
        },
    }
}

fn request(request: ApiRequest) -> CliAction {
    CliAction::Request {
        request: Box::new(request),
        mode: ExecutionMode::Execute,
    }
}

fn parse_required_text(value: &str) -> std::result::Result<RequiredText, String> {
    let (path, contains) = value
        .split_once('=')
        .ok_or_else(|| "required text must use PATH=TEXT".to_string())?;
    if path.trim().is_empty() || contains.trim().is_empty() {
        return Err("required text path and text must be non-empty".into());
    }
    Ok(RequiredText {
        path: path.to_string(),
        contains: contains.to_string(),
    })
}

fn mode(dry_run: bool) -> ExecutionMode {
    if dry_run {
        ExecutionMode::Plan
    } else {
        ExecutionMode::Execute
    }
}
