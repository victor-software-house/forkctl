use crate::manifest::PatchKind;
use crate::state::PendingState;
use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiInvocation {
    pub protocol_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
    pub request: ApiRequest,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "command", content = "params", rename_all = "snake_case")]
pub enum ApiRequest {
    Init,
    Status,
    New(NewPatchRequest),
    FinishNew,
    Verify,
    Rebase { onto: String },
    Publish,
    Instructions,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewPatchRequest {
    pub name: String,
    pub kind: PatchKind,
    pub purpose: String,
    pub upstream_status: String,
    pub drop_when: String,
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApiResponse {
    Success {
        protocol_version: u32,
        result: Box<CommandResult>,
        notices: Vec<Notice>,
    },
    Error {
        protocol_version: u32,
        error: ApiError,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "command", content = "data", rename_all = "snake_case")]
pub enum CommandResult {
    Init(InitResult),
    Status(Box<StatusResult>),
    New(NewResult),
    Verify(VerificationResult),
    Rebase(RebaseResult),
    Publish(PublishResult),
    Instructions(InstructionsResult),
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct Notice {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    pub causes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    InvalidRequest,
    UnsupportedProtocol,
    OperationFailed,
}

impl std::fmt::Display for ApiErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "invalid_request",
            Self::UnsupportedProtocol => "unsupported_protocol",
            Self::OperationFailed => "operation_failed",
        })
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct Outcome {
    pub result: CommandResult,
    pub notices: Vec<Notice>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct InitResult {
    pub already_initialized: bool,
    pub verification: VerificationResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct VerificationResult {
    pub canonical_base: String,
    pub stack_base: String,
    pub patch_count: usize,
    pub source_tree: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct StatusResult {
    pub repository: String,
    pub current_branch: Option<String>,
    pub declared_branch: String,
    pub downstream_remote: String,
    pub downstream_sha: Option<String>,
    pub upstream_remote: String,
    pub upstream_fetch_ref: String,
    pub selected_target: String,
    pub canonical_base: String,
    pub stack_base: String,
    pub applied_patches: Vec<String>,
    pub unapplied_patches: Vec<String>,
    pub exports: Vec<String>,
    pub dirty: Vec<String>,
    pub pending: Option<PendingState>,
    pub verification: VerificationStatus,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct VerificationStatus {
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct NewResult {
    pub phase: NewPhase,
    pub patch: Option<String>,
    pub kind: Option<PatchKind>,
    pub allowed_paths: Vec<String>,
    pub verification: Option<VerificationResult>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NewPhase {
    Created,
    Finished,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct RebaseResult {
    pub selected_target: String,
    pub new_base: String,
    pub new_tip: String,
    pub recovery_tag: String,
    pub report_path: String,
    pub report_object_id: String,
    pub dropped_patches: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PublishResult {
    pub branch: String,
    pub head: String,
    pub recovery_tag: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct InstructionsResult {
    pub markdown: String,
}

impl Outcome {
    pub fn new(result: CommandResult) -> Self {
        Self {
            result,
            notices: Vec::new(),
        }
    }

    pub fn with_notices(result: CommandResult, notices: Vec<Notice>) -> Self {
        Self { result, notices }
    }
}

impl ApiResponse {
    pub fn success(outcome: Outcome) -> Self {
        Self::Success {
            protocol_version: PROTOCOL_VERSION,
            result: Box::new(outcome.result),
            notices: outcome.notices,
        }
    }

    pub fn error(error: ApiError) -> Self {
        Self::Error {
            protocol_version: PROTOCOL_VERSION,
            error,
        }
    }
}

pub fn schema_document() -> serde_json::Value {
    serde_json::json!({
        "protocol_version": PROTOCOL_VERSION,
        "invocation": schemars::schema_for!(ApiInvocation),
        "response": schemars::schema_for!(ApiResponse),
    })
}
