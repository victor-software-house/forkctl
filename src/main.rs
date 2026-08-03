mod app;
mod ledger;
mod manifest;
mod pattern;
mod process;
mod protocol;
mod report;
mod state;
mod view;

use anyhow::{Context, Result, ensure};
use app::App;
use clap::{Args, Parser, Subcommand};
use manifest::PatchKind;
use protocol::{
    ApiError, ApiErrorCode, ApiInvocation, ApiRequest, ApiResponse, CommandResult,
    InstructionsResult, NewPatchRequest, Outcome, OutputFormat, PROTOCOL_VERSION,
};
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_MANIFEST: &str = "patches/fork.json";
const INSTRUCTIONS: &str = include_str!("instructions.md");

#[derive(Parser)]
#[command(version, about = "Maintain a declared StGit downstream patch stack")]
struct Cli {
    #[arg(long, global = true)]
    manifest: Option<PathBuf>,

    #[arg(long, global = true, value_enum, default_value = "pretty")]
    output: OutputFormat,

    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    /// Reconstruct `StGit` metadata after cloning a fork.
    Init,
    /// Explain repository, stack, verification, and pending operation state.
    Status,
    /// Create, or finish, a documented downstream patch.
    New(NewArgs),
    /// Verify stack identity, audit metadata, exports, and source contracts.
    Verify,
    /// Rebase onto an explicit full upstream branch/tag ref or commit SHA.
    Rebase {
        #[arg(long)]
        onto: String,
    },
    /// Publish the verified branch and recovery tag with an exact lease.
    Publish,
    /// Print the repository and agent workflow contract.
    Instructions,
    /// Use the versioned local JSON API.
    Api {
        #[command(subcommand)]
        operation: ApiOperation,
    },
}

#[derive(Subcommand)]
enum ApiOperation {
    /// Print JSON Schema for API invocation and response envelopes.
    Schema,
    /// Read one API invocation from stdin and emit one JSON response.
    Call,
}

#[derive(Args)]
struct NewArgs {
    #[arg(long)]
    finish: bool,
    name: Option<String>,
    #[arg(long, value_enum)]
    kind: Option<PatchKind>,
    #[arg(long)]
    purpose: Option<String>,
    #[arg(long)]
    upstream_status: Option<String>,
    #[arg(long)]
    drop_when: Option<String>,
    #[arg(long = "path")]
    paths: Vec<String>,
    #[arg(long)]
    export: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Operation::Api { operation } = &cli.operation {
        return run_api(operation);
    }

    match cli_request(cli) {
        Ok((manifest, request, output)) => {
            let response = execute(manifest.as_deref(), request).map_or_else(
                |error| ApiResponse::error(api_error(&error, ApiErrorCode::OperationFailed)),
                ApiResponse::success,
            );
            if emit_response(&response, output).is_ok() {
                response_exit(&response)
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            let response = ApiResponse::error(api_error(&error, ApiErrorCode::InvalidRequest));
            let _ = view::emit_pretty(&response);
            ExitCode::FAILURE
        }
    }
}

fn run_api(operation: &ApiOperation) -> ExitCode {
    match operation {
        ApiOperation::Schema => match view::emit_json(&protocol::schema_document()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
        ApiOperation::Call => {
            let response = read_invocation()
                .and_then(|invocation| {
                    ensure!(
                        invocation.protocol_version == PROTOCOL_VERSION,
                        "unsupported protocol version: {}",
                        invocation.protocol_version
                    );
                    execute(invocation.manifest.as_deref(), invocation.request)
                })
                .map_or_else(
                    |error| {
                        let code = if error
                            .to_string()
                            .starts_with("unsupported protocol version")
                        {
                            ApiErrorCode::UnsupportedProtocol
                        } else {
                            ApiErrorCode::InvalidRequest
                        };
                        ApiResponse::error(api_error(&error, code))
                    },
                    ApiResponse::success,
                );
            let emitted = view::emit_json(&response).is_ok();
            if emitted {
                response_exit(&response)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

fn cli_request(cli: Cli) -> Result<(Option<String>, ApiRequest, OutputFormat)> {
    let request = match cli.operation {
        Operation::Init => ApiRequest::Init,
        Operation::Status => ApiRequest::Status,
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
            ApiRequest::FinishNew
        }
        Operation::New(args) => ApiRequest::New(NewPatchRequest {
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
        Operation::Verify => ApiRequest::Verify,
        Operation::Rebase { onto } => ApiRequest::Rebase { onto },
        Operation::Publish => ApiRequest::Publish,
        Operation::Instructions => ApiRequest::Instructions,
        Operation::Api { .. } => unreachable!("API operations handled before mapping"),
    };
    Ok((
        cli.manifest.map(|path| path.display().to_string()),
        request,
        if cli.json {
            OutputFormat::Json
        } else {
            cli.output
        },
    ))
}

fn execute(manifest: Option<&str>, request: ApiRequest) -> Result<Outcome> {
    if matches!(request, ApiRequest::Instructions) {
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
    let mut app = App::load(&manifest)?;
    match request {
        ApiRequest::Init => Ok(Outcome::new(CommandResult::Init(app.init()?))),
        ApiRequest::Status => Ok(Outcome::new(CommandResult::Status(Box::new(app.status()?)))),
        ApiRequest::New(request) => Ok(Outcome::new(CommandResult::New(app.new_patch(request)?))),
        ApiRequest::FinishNew => Ok(Outcome::new(CommandResult::New(app.finish_new()?))),
        ApiRequest::Verify => Ok(Outcome::new(CommandResult::Verify(app.verify()?))),
        ApiRequest::Rebase { onto } => {
            let result = app.rebase(&onto)?;
            let notices = result
                .dropped_patches
                .iter()
                .map(|patch| protocol::Notice {
                    code: "upstream_merged_patch_dropped".into(),
                    message: format!(
                        "dropped upstream-merged patch {patch}; recorded in PATCHES.md history"
                    ),
                })
                .collect();
            Ok(Outcome::with_notices(
                CommandResult::Rebase(result),
                notices,
            ))
        }
        ApiRequest::Publish => Ok(Outcome::new(CommandResult::Publish(app.publish()?))),
        ApiRequest::Instructions => unreachable!("instructions handled without repository"),
    }
}

fn read_invocation() -> Result<ApiInvocation> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read API invocation from stdin")?;
    serde_json::from_str(&input).context("parse API invocation")
}

fn api_error(error: &anyhow::Error, code: ApiErrorCode) -> ApiError {
    let mut chain = error.chain();
    let message = chain
        .next()
        .map_or_else(|| "unknown error".to_string(), ToString::to_string);
    ApiError {
        code,
        message,
        causes: chain.map(ToString::to_string).collect(),
    }
}

fn emit_response(response: &ApiResponse, output: OutputFormat) -> std::io::Result<()> {
    match output {
        OutputFormat::Pretty => view::emit_pretty(response),
        OutputFormat::Json => view::emit_json(response),
    }
}

fn response_exit(response: &ApiResponse) -> ExitCode {
    match response {
        ApiResponse::Success { .. } => ExitCode::SUCCESS,
        ApiResponse::Error { .. } => ExitCode::FAILURE,
    }
}
