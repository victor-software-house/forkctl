mod app;
mod cli;
mod completion;
mod error;
mod ledger;
mod manifest;
mod manifest_codec;
mod presentation;
mod process;
mod protocol;
mod report;
mod state;
mod update;

use std::env;
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use app::App;
use clap::CommandFactory;
use cli::{Cli, CliAction, CompletionShell};
use ctl_core::{App as Chassis, ColorMode, OutputFormat, View};
use presentation::Report;
use protocol::{
    ApiError, ApiErrorCode, ApiInvocation, ApiRequest, ApiResponse, CommandResult, ErrorDetails,
    ExecutionMode, InstructionsResult, Outcome, PROTOCOL_VERSION,
};

const DEFAULT_MANIFEST: &str = "patches/fork.yaml";
const INSTRUCTIONS: &str = include_str!("instructions.md");

#[cfg(test)]
mod operator_docs;

fn main() -> ExitCode {
    Chassis::<Cli>::new("forkctl")
        .mounted_as("fork")
        .usage_spec(|command, bin| usage_spec(&command, bin))
        .before_parse(complete_from_environment)
        .view(select_view)
        .run(execute_cli)
}

fn complete_from_environment(args: &[OsString]) -> Option<ExitCode> {
    let current_dir = env::current_dir().ok();
    match clap_complete::CompleteEnv::with_factory(Cli::command)
        .try_complete(args.iter().cloned(), current_dir.as_deref())
    {
        Ok(true) => Some(ExitCode::SUCCESS),
        Ok(false) => None,
        Err(error) => {
            let code = u8::try_from(error.exit_code()).map_or(ExitCode::FAILURE, ExitCode::from);
            let _ = error.print();
            Some(code)
        }
    }
}

fn select_view(cli: &Cli) -> View {
    match &cli.command {
        Some(cli::Command::Api { .. }) => View::new(OutputFormat::Json, ColorMode::Never),
        Some(cli::Command::Completion { .. } | cli::Command::Candidates { .. }) => {
            View::new(OutputFormat::Pretty, ColorMode::Never)
        }
        _ => cli.output.view(),
    }
}

fn execute_cli(cli: Cli) -> Result<Report> {
    let manifest = cli.manifest.as_ref().map(|path| path.display().to_string());
    let format = cli.output.format;
    let quiet = cli.output.quiet;
    let action = cli.into_action();
    Ok(match action {
        Ok(CliAction::ApiSchema(kind)) => Report::json(protocol::schema_document(kind)),
        Ok(CliAction::ApiCall) => Report::response(run_api_call(), None),
        Ok(CliAction::Completion(shell)) => Report::text(completion_text(shell)?),
        Ok(CliAction::Candidates(kind)) => {
            Report::text(completion::candidate_lines(kind).join("\n"))
        }
        Ok(CliAction::Request { request, mode }) => {
            if format == OutputFormat::Pretty {
                process::set_stream_operator_output(true);
            }
            let command = request.command();
            let response = execute(manifest.as_deref(), *request, mode).map_or_else(
                |error| ApiResponse::error(command, mode, api_error(&error)),
                |outcome| ApiResponse::success(command, mode, outcome),
            );
            let update_notice = matches!(response, ApiResponse::Success { .. })
                .then(|| {
                    (format == OutputFormat::Pretty && !quiet)
                        .then(update::available_notice)
                        .flatten()
                })
                .flatten();
            Report::response(response, update_notice)
        }
        Err(error) => Report::response(
            ApiResponse::error("cli", ExecutionMode::Execute, request_error(&error)),
            None,
        ),
    })
}

fn execute(manifest: Option<&str>, request: ApiRequest, mode: ExecutionMode) -> Result<Outcome> {
    if request.is_read_only() && mode == ExecutionMode::Plan {
        return Err(error::DomainError::invalid_request(format!(
            "read-only command {} does not support plan mode",
            request.command()
        ))
        .into());
    }
    if matches!(request, ApiRequest::Instructions(_)) {
        return Ok(Outcome::new(CommandResult::Instructions(
            InstructionsResult {
                markdown: INSTRUCTIONS.to_string(),
            },
        )));
    }
    let manifest = manifest
        .map(PathBuf::from)
        .or_else(|| env::var_os("FORK_MANIFEST").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MANIFEST));
    let mut app = App::discover(&manifest)?;
    let result = match request {
        ApiRequest::Init(args) => app.init(args, mode)?,
        ApiRequest::Status(_) => CommandResult::Status(Box::new(app.status()?)),
        ApiRequest::Check(args) => CommandResult::Check(app.check(&args)?),
        ApiRequest::PatchList(_) => CommandResult::PatchList(app.patch_list()?),
        ApiRequest::PatchShow(args) => CommandResult::PatchShow(app.patch_show(&args)?),
        ApiRequest::PatchCreate(args) => app.patch_create(args, mode)?,
        ApiRequest::PatchSelect(args) => app.patch_select(&args.patch, mode)?,
        ApiRequest::PatchEdit(args) => app.patch_edit(args, mode)?,
        ApiRequest::PatchRefresh(args) => app.patch_refresh(args, mode)?,
        ApiRequest::PatchFinish(args) => app.patch_finish(&args, mode)?,
        ApiRequest::PatchRemove(args) => app.patch_remove(args, mode)?,
        ApiRequest::PatchDisable(args) => app.patch_disable(args, mode)?,
        ApiRequest::PatchEnable(args) => app.patch_enable(&args.patch, mode)?,
        ApiRequest::ContractEdit(args) => app.contract_edit(args, mode)?,
        ApiRequest::Rebase(args) => app.rebase(&args.onto, mode)?,
        ApiRequest::Publish(args) => app.publish(&args, mode)?,
        ApiRequest::OperationStatus(_) => {
            CommandResult::OperationStatus(Box::new(app.operation_status()?))
        }
        ApiRequest::OperationContinue(_) => app.operation_continue(mode)?,
        ApiRequest::OperationAbort(args) => app.operation_abort(args.confirmed, mode)?,
        ApiRequest::Instructions(_) => unreachable!("instructions handled without repository"),
    };
    let operation_id = app.read_operation()?.map(|operation| operation.id);
    Ok(Outcome::new(result).with_optional_operation(operation_id))
}

fn run_api_call() -> ApiResponse {
    match read_invocation() {
        Err(error) => ApiResponse::error("api.call", ExecutionMode::Execute, request_error(&error)),
        Ok(invocation) if invocation.protocol_version != PROTOCOL_VERSION => ApiResponse::error(
            invocation.request.command(),
            invocation.mode,
            ApiError {
                code: ApiErrorCode::UnsupportedProtocol,
                message: format!(
                    "unsupported protocol version: {}",
                    invocation.protocol_version
                ),
                causes: Vec::new(),
                details: ErrorDetails::Request {
                    field: Some("protocol_version".into()),
                    issue: format!("expected {PROTOCOL_VERSION}"),
                },
                retryable: false,
                suggested_command: Some("forkctl api schema".into()),
            },
        ),
        Ok(invocation) => {
            let command = invocation.request.command();
            execute(
                invocation.manifest.as_deref(),
                invocation.request,
                invocation.mode,
            )
            .map_or_else(
                |error| ApiResponse::error(command, invocation.mode, api_error(&error)),
                |outcome| ApiResponse::success(command, invocation.mode, outcome),
            )
        }
    }
}

fn usage_spec(command: &clap::Command, bin: &str) -> String {
    usage_document(command, bin).to_string()
}

fn usage_document(command: &clap::Command, bin: &str) -> usage::Spec {
    let mut spec = usage::Spec::from(command);
    spec.name = bin.to_string();
    spec.bin = bin.to_string();
    let candidate_command = if bin == "forkctl" {
        "forkctl".to_string()
    } else {
        format!("mise run --quiet {bin} --")
    };
    add_usage_completions(&mut spec, &candidate_command);
    spec
}

fn add_usage_completions(spec: &mut usage::Spec, candidate_command: &str) {
    use usage::SpecComplete;
    let entries: &[(&[&str], &str, &str)] = &[
        (&["check"], "patch", "patch"),
        (&["patch", "show"], "name", "patch"),
        (&["patch", "select"], "name", "patch"),
        (&["patch", "edit"], "name", "patch"),
        (&["patch", "refresh"], "name", "patch"),
        (&["patch", "finish"], "name", "patch"),
        (&["rebase"], "onto", "ref"),
        (&["init"], "base", "ref"),
        (&["init"], "upstream_remote", "remote"),
        (&["init"], "downstream_remote", "remote"),
    ];
    for (path, name, kind) in entries {
        let mut command = &mut spec.cmd;
        for segment in *path {
            let Some(next) = command.subcommands.get_mut(*segment) else {
                break;
            };
            command = next;
        }
        command.complete.insert(
            (*name).to_string(),
            SpecComplete::new(*name)
                .run(format!("{candidate_command} __candidates {kind}"))
                .descriptions(true),
        );
    }
}

fn completion_text(shell: CompletionShell) -> Result<String> {
    use clap_complete::env::{Bash, Elvish, EnvCompleter, Fish, Powershell, Zsh};

    let mut output = Vec::new();
    match shell {
        CompletionShell::Bash => {
            Bash.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)?;
        }
        CompletionShell::Elvish => {
            Elvish.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)?;
        }
        CompletionShell::Fish => {
            Fish.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)?;
        }
        CompletionShell::Nu => {
            return usage::complete::complete(&usage::complete::CompleteOptions {
                usage_bin: "usage".to_string(),
                shell: "nu".to_string(),
                bin: "forkctl".to_string(),
                cache_key: Some(env!("CARGO_PKG_VERSION").to_string()),
                spec: Some(usage_document(&Cli::command(), "forkctl")),
                usage_cmd: None,
                source_file: None,
            })
            .context("generate Nu completion");
        }
        CompletionShell::Powershell => {
            Powershell.write_registration(
                "COMPLETE",
                "forkctl",
                "forkctl",
                "forkctl",
                &mut output,
            )?;
        }
        CompletionShell::Zsh => {
            Zsh.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)?;
        }
    }
    String::from_utf8(output).context("completion registration is UTF-8")
}

fn read_invocation() -> Result<ApiInvocation> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read API invocation from stdin")?;
    serde_json::from_str(&input).context("parse API invocation")
}

fn request_error(error: &anyhow::Error) -> ApiError {
    ApiError {
        code: ApiErrorCode::InvalidRequest,
        message: error.to_string(),
        causes: error.chain().skip(1).map(ToString::to_string).collect(),
        details: ErrorDetails::Request {
            field: None,
            issue: error.to_string(),
        },
        retryable: false,
        suggested_command: None,
    }
}

fn api_error(error: &anyhow::Error) -> ApiError {
    let causes = error.chain().skip(1).map(ToString::to_string).collect();
    if let Some(domain) = error.downcast_ref::<error::DomainError>() {
        return domain.to_api_error(causes);
    }
    ApiError {
        code: ApiErrorCode::InternalError,
        message: error.to_string(),
        causes,
        details: ErrorDetails::None,
        retryable: false,
        suggested_command: None,
    }
}

trait OutcomeExt {
    fn with_optional_operation(self, operation_id: Option<String>) -> Self;
}

impl OutcomeExt for Outcome {
    fn with_optional_operation(mut self, operation_id: Option<String>) -> Self {
        self.operation_id = operation_id;
        self
    }
}
