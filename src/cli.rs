use crate::manifest::{Check, CheckStage, PatchKind, RequiredText};
use crate::protocol::{
    ApiRequest, CaptureSource, CheckArgs, CheckEdit, CheckScope, ColorMode, ContractEditArgs,
    EmptyArgs, ExecutionMode, InitArgs, OperationAbortArgs, OutputFormat, PatchCreateArgs,
    PatchEditArgs, PatchName, PatchRefreshArgs, PatchTarget, PatchTransitionArgs, PublishArgs,
    RebaseArgs, SchemaKind, ScopeEdit,
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
    /// Manifest path; defaults to `FORK_MANIFEST` then `patches/fork.yaml`.
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
    /// Publish the managed branch using the repository default, or an explicit mode.
    Publish(PublishCliArgs),
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
    /// Default publish mode for this repository.
    #[arg(long, help_heading = "Repository", value_enum)]
    pub publish: Option<crate::manifest::PublishMode>,
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
    /// Permanently remove a patch while preserving recovery evidence.
    Remove(PatchTransitionCliArgs),
    /// Disable a patch while retaining enough evidence to re-enable it.
    Disable(PatchTransitionCliArgs),
    /// Re-enable a previously disabled patch.
    Enable {
        /// Disabled patch name.
        #[arg(add = crate::completion::patch_completer())]
        name: String,
        #[command(flatten)]
        execution: DryRunArgs,
    },
}

#[derive(Args)]
pub struct PatchTransitionCliArgs {
    /// Patch name.
    #[arg(add = crate::completion::patch_completer())]
    pub name: String,
    /// Auditable operator reason for this transition.
    #[arg(short = 'r', long, help_heading = "Audit")]
    pub reason: String,
    #[command(flatten)]
    pub execution: DryRunArgs,
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
    /// Declared check as NAME=COMMAND, where {files} expands to the checked files; repeatable.
    #[arg(short = 'C', long = "check", help_heading = "Checks", value_parser = parse_check)]
    pub checks: Vec<Check>,
    /// Restrict a declared check as NAME=GLOB, defaulting to the patch scope; repeatable.
    #[arg(short = 'g', long = "check-glob", help_heading = "Checks", value_parser = parse_named_value)]
    pub check_globs: Vec<(String, String)>,
    /// Evaluate a declared check at its own patch commit as NAME=patch; repeatable.
    #[arg(long = "check-at", help_heading = "Checks", value_parser = parse_check_stage)]
    pub check_stages: Vec<(String, CheckStage)>,
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
    /// Replace the complete check set before adding supplied checks.
    #[arg(long, help_heading = "Checks")]
    pub clear_checks: bool,
    /// Declared check as NAME=COMMAND, where {files} expands to the checked files; repeatable.
    #[arg(short = 'C', long = "check", help_heading = "Checks", value_parser = parse_check)]
    pub checks: Vec<Check>,
    /// Restrict a declared check as NAME=GLOB, defaulting to the patch scope; repeatable.
    #[arg(short = 'g', long = "check-glob", help_heading = "Checks", value_parser = parse_named_value)]
    pub check_globs: Vec<(String, String)>,
    /// Evaluate a declared check at its own patch commit as NAME=patch; repeatable.
    #[arg(long = "check-at", help_heading = "Checks", value_parser = parse_check_stage)]
    pub check_stages: Vec<(String, CheckStage)>,
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
    /// Refresh a non-top patch and unapply every patch above it.
    /// Everyday follow-up work should be a new top patch instead.
    #[arg(long, help_heading = "Stack")]
    pub rewrite_below: bool,
    #[command(flatten)]
    pub execution: DryRunArgs,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("contract_change")
        .args(["clear", "allow_base", "required_text", "publish_mode"])
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
    /// Add a required repository contract as PATH=TEXT; repeatable.
    #[arg(short = 'r', long, help_heading = "Contracts", value_parser = parse_required_text)]
    pub required_text: Vec<RequiredText>,
    /// Set the repository default publish mode.
    #[arg(long, help_heading = "Contracts", value_enum)]
    pub publish_mode: Option<crate::manifest::PublishMode>,
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

#[derive(Args)]
#[allow(clippy::struct_excessive_bools)]
pub struct PublishCliArgs {
    /// Exact-lease rewrite. Overrides the repository default.
    #[arg(long, group = "publish_mode", help_heading = "Mode")]
    pub rewrite: bool,
    /// Keep the previous tip as an ancestor and fast-forward.
    #[arg(long, group = "publish_mode", help_heading = "Mode")]
    pub append: bool,
    /// Push a net-tree proposal branch and open a PR when `gh` is available.
    #[arg(long, group = "publish_mode", help_heading = "Mode")]
    pub propose: bool,
    /// Promote an exact proposal to the downstream branch.
    #[arg(long, group = "publish_mode", help_heading = "Mode")]
    pub promote: bool,
    /// Proposal branch, tag, or URL used by --promote.
    #[arg(long, help_heading = "Mode")]
    pub proposal: Option<String>,
    /// Persist this mode as the repository default, then publish unless --set-default is the only action.
    #[arg(long, value_enum, help_heading = "Mode")]
    pub set_default: Option<crate::manifest::PublishMode>,
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
                    publish: args.publish,
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
                        publish_mode: args.publish_mode,
                    })),
                    mode: mode(args.execution.dry_run),
                },
            },
            Command::Publish(args) => {
                let publish_mode = if args.rewrite {
                    Some(crate::manifest::PublishMode::Rewrite)
                } else if args.append {
                    Some(crate::manifest::PublishMode::Append)
                } else if args.propose {
                    Some(crate::manifest::PublishMode::Propose)
                } else {
                    None
                };
                CliAction::Request {
                    request: Box::new(ApiRequest::Publish(PublishArgs {
                        mode: publish_mode,
                        set_default: args.set_default,
                        promote: args.promote,
                        proposal: args.proposal,
                    })),
                    mode: mode(args.execution.dry_run),
                }
            }
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

fn patch_edit_action(args: PatchEditCliArgs) -> Result<CliAction> {
    Ok({
        let declared = declared_checks(args.checks, args.check_globs, args.check_stages)?;
        ensure!(
            args.kind.is_some()
                || args.purpose.is_some()
                || args.upstream_status.is_some()
                || args.drop_when.is_some()
                || !args.set_scope.is_empty()
                || !args.add_scope.is_empty()
                || !args.remove_scope.is_empty()
                || args.clear_checks
                || !declared.is_empty(),
            "patch edit requires at least one metadata, scope, or check change"
        );
        let checks = if args.clear_checks {
            Some(CheckEdit::Set { checks: declared })
        } else if declared.is_empty() {
            None
        } else {
            Some(CheckEdit::Add { checks: declared })
        };
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
                checks,
            })),
            mode: mode(args.execution.dry_run),
        }
    })
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
                checks: declared_checks(args.checks, args.check_globs, args.check_stages)?,
            })),
            mode: mode(args.execution.dry_run),
        },
        PatchCommand::Select { name, execution } => CliAction::Request {
            request: Box::new(ApiRequest::PatchSelect(PatchName { patch: name })),
            mode: mode(execution.dry_run),
        },
        PatchCommand::Edit(args) => patch_edit_action(args)?,
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
                    rewrite_below: args.rewrite_below,
                })),
                mode: mode(args.execution.dry_run),
            }
        }
        PatchCommand::Finish { name, execution } => CliAction::Request {
            request: Box::new(ApiRequest::PatchFinish(PatchTarget { patch: name })),
            mode: mode(execution.dry_run),
        },
        PatchCommand::Remove(args) => CliAction::Request {
            request: Box::new(ApiRequest::PatchRemove(PatchTransitionArgs {
                patch: args.name,
                reason: args.reason,
            })),
            mode: mode(args.execution.dry_run),
        },
        PatchCommand::Disable(args) => CliAction::Request {
            request: Box::new(ApiRequest::PatchDisable(PatchTransitionArgs {
                patch: args.name,
                reason: args.reason,
            })),
            mode: mode(args.execution.dry_run),
        },
        PatchCommand::Enable { name, execution } => CliAction::Request {
            request: Box::new(ApiRequest::PatchEnable(PatchName { patch: name })),
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

fn parse_check(value: &str) -> std::result::Result<Check, String> {
    let (name, run) = split_named(value, "COMMAND")?;
    Ok(Check {
        name,
        run,
        glob: Vec::new(),
        at: CheckStage::default(),
    })
}

fn parse_named_value(value: &str) -> std::result::Result<(String, String), String> {
    split_named(value, "GLOB")
}

fn parse_check_stage(value: &str) -> std::result::Result<(String, CheckStage), String> {
    let (name, stage) = split_named(value, "stack|patch")?;
    let stage = match stage.as_str() {
        "stack" => CheckStage::Stack,
        "patch" => CheckStage::Patch,
        other => return Err(format!("expected stack or patch, found {other}")),
    };
    Ok((name, stage))
}

fn split_named(value: &str, label: &str) -> std::result::Result<(String, String), String> {
    let (name, rest) = value
        .split_once('=')
        .ok_or_else(|| format!("expected NAME={label}"))?;
    if name.is_empty() || rest.is_empty() {
        return Err(format!("expected NAME={label}"));
    }
    Ok((name.to_string(), rest.to_string()))
}

fn declared_checks(
    mut checks: Vec<Check>,
    globs: Vec<(String, String)>,
    stages: Vec<(String, CheckStage)>,
) -> Result<Vec<Check>> {
    for (name, glob) in globs {
        let check = checks
            .iter_mut()
            .find(|check| check.name == name)
            .with_context(|| format!("--check-glob names an undeclared check: {name}"))?;
        if !check.glob.contains(&glob) {
            check.glob.push(glob);
        }
    }
    for (name, stage) in stages {
        let check = checks
            .iter_mut()
            .find(|check| check.name == name)
            .with_context(|| format!("--check-at names an undeclared check: {name}"))?;
        check.at = stage;
    }
    Ok(checks)
}

fn mode(dry_run: bool) -> ExecutionMode {
    if dry_run {
        ExecutionMode::Plan
    } else {
        ExecutionMode::Execute
    }
}
