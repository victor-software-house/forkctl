mod app;
mod cli;
mod completion;
mod error;
mod help;
mod layout;
mod ledger;
mod manifest;
mod manifest_codec;
mod process;
mod protocol;
mod report;
mod state;
mod update;
mod view;

use anyhow::{Context, Result};
use app::App;
use clap::{CommandFactory, Parser};
use cli::{Cli, CliAction, CompletionShell};
use protocol::{
    ApiError, ApiErrorCode, ApiInvocation, ApiRequest, ApiResponse, CommandResult, ErrorDetails,
    ExecutionMode, InstructionsResult, Outcome, OutputFormat, PROTOCOL_VERSION,
};
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_MANIFEST: &str = "patches/fork.yaml";
const INSTRUCTIONS: &str = include_str!("instructions.md");

fn main() -> ExitCode {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    match help::try_emit::<Cli>() {
        Ok(true) => return ExitCode::SUCCESS,
        Ok(false) => {}
        Err(_) => return ExitCode::FAILURE,
    }
    let cli = Cli::parse();
    let manifest = cli.manifest.as_ref().map(|path| path.display().to_string());
    let output = cli.output;
    let color = cli.color;
    let quiet = cli.quiet;
    match cli.into_action() {
        Ok(CliAction::ApiSchema(kind)) => emit_schema(kind),
        Ok(CliAction::ApiCall) => run_api_call(),
        Ok(CliAction::Completion(shell)) => emit_completion(shell),
        Ok(CliAction::Candidates(kind)) => emit_candidates(kind),
        Ok(CliAction::UsageSpec(bin)) => emit_usage_spec(&bin),
        Ok(CliAction::Request { request, mode }) => {
            if output == OutputFormat::Pretty {
                process::set_stream_operator_output(true);
            }
            let command = request.command();
            let response = execute(manifest.as_deref(), *request, mode).map_or_else(
                |error| ApiResponse::error(command, mode, api_error(&error)),
                |outcome| ApiResponse::success(command, mode, outcome),
            );
            if emit_response(&response, output, color, quiet).is_err() {
                return ExitCode::FAILURE;
            }
            if matches!(response, ApiResponse::Success { .. })
                && output == OutputFormat::Pretty
                && !quiet
                && let Some(notice) = update::available_notice()
            {
                eprintln!("{notice}");
            }
            response_exit(&response)
        }
        Err(error) => {
            let response = ApiResponse::error("cli", ExecutionMode::Execute, request_error(&error));
            let _ = view::emit_pretty(&response, color, false);
            ExitCode::from(2)
        }
    }
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
        ApiRequest::Publish(_) => app.publish(mode)?,
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

fn run_api_call() -> ExitCode {
    let response = match read_invocation() {
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
    };
    if view::emit_json(&response).is_ok() {
        response_exit(&response)
    } else {
        ExitCode::FAILURE
    }
}

fn emit_schema(kind: protocol::SchemaKind) -> ExitCode {
    if view::emit_json(&protocol::schema_document(kind)).is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn emit_usage_spec(bin: &str) -> ExitCode {
    if view::emit_text(&usage_spec(bin).to_string()).is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn usage_spec(bin: &str) -> usage::Spec {
    let mut command = Cli::command();
    command.set_bin_name(bin);
    let mut spec = usage::Spec::from(&command);
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

fn emit_candidates(kind: completion::CandidateKind) -> ExitCode {
    let output = completion::candidate_lines(kind).join("\n");
    if output.is_empty() || view::emit_text(&output).is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn emit_completion(shell: CompletionShell) -> ExitCode {
    use clap_complete::env::{Bash, Elvish, EnvCompleter, Fish, Powershell, Zsh};

    let mut output = Vec::new();
    let result = match shell {
        CompletionShell::Bash => {
            Bash.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)
        }
        CompletionShell::Elvish => {
            Elvish.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)
        }
        CompletionShell::Fish => {
            Fish.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)
        }
        CompletionShell::Nu => {
            return match usage::complete::complete(&usage::complete::CompleteOptions {
                usage_bin: "usage".to_string(),
                shell: "nu".to_string(),
                bin: "forkctl".to_string(),
                cache_key: Some(env!("CARGO_PKG_VERSION").to_string()),
                spec: Some(usage_spec("forkctl")),
                usage_cmd: None,
                include_bash_completion_lib: false,
                source_file: None,
            }) {
                Ok(output) if view::emit_text(&output).is_ok() => ExitCode::SUCCESS,
                _ => ExitCode::FAILURE,
            };
        }
        CompletionShell::Powershell => {
            Powershell.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)
        }
        CompletionShell::Zsh => {
            Zsh.write_registration("COMPLETE", "forkctl", "forkctl", "forkctl", &mut output)
        }
    };
    match result.and_then(|()| {
        let output = String::from_utf8(output).map_err(std::io::Error::other)?;
        view::emit_text(&output)
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
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

fn emit_response(
    response: &ApiResponse,
    output: OutputFormat,
    color: protocol::ColorMode,
    quiet: bool,
) -> std::io::Result<()> {
    match output {
        OutputFormat::Pretty => view::emit_pretty(response, color, quiet),
        OutputFormat::Json => view::emit_json(response),
    }
}

fn response_exit(response: &ApiResponse) -> ExitCode {
    match response {
        ApiResponse::Success { .. } => ExitCode::SUCCESS,
        ApiResponse::Error { error, .. } if matches!(error.code, ApiErrorCode::InvalidRequest) => {
            ExitCode::from(2)
        }
        ApiResponse::Error { .. } => ExitCode::FAILURE,
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
