use crate::manifest::{BaseTarget, Check, Contracts, Manifest, Patch, PatchKind, RequiredText};
use crate::state::{ActivePatchState, OperationState};
use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Execute,
    Plan,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiInvocation {
    pub protocol_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
    pub mode: ExecutionMode,
    pub request: ApiRequest,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "command", content = "arguments", deny_unknown_fields)]
pub enum ApiRequest {
    #[serde(rename = "init")]
    Init(InitArgs),
    #[serde(rename = "status")]
    Status(EmptyArgs),
    #[serde(rename = "check")]
    Check(CheckArgs),
    #[serde(rename = "patch.list")]
    PatchList(EmptyArgs),
    #[serde(rename = "patch.show")]
    PatchShow(PatchTarget),
    #[serde(rename = "patch.create")]
    PatchCreate(PatchCreateArgs),
    #[serde(rename = "patch.select")]
    PatchSelect(PatchName),
    #[serde(rename = "patch.edit")]
    PatchEdit(PatchEditArgs),
    #[serde(rename = "patch.refresh")]
    PatchRefresh(PatchRefreshArgs),
    #[serde(rename = "patch.finish")]
    PatchFinish(PatchTarget),
    #[serde(rename = "patch.remove")]
    PatchRemove(PatchTransitionArgs),
    #[serde(rename = "patch.disable")]
    PatchDisable(PatchTransitionArgs),
    #[serde(rename = "patch.enable")]
    PatchEnable(PatchName),
    #[serde(rename = "contract.edit")]
    ContractEdit(ContractEditArgs),
    #[serde(rename = "rebase")]
    Rebase(RebaseArgs),
    #[serde(rename = "publish")]
    Publish(EmptyArgs),
    #[serde(rename = "operation.status")]
    OperationStatus(EmptyArgs),
    #[serde(rename = "operation.continue")]
    OperationContinue(EmptyArgs),
    #[serde(rename = "operation.abort")]
    OperationAbort(OperationAbortArgs),
    #[serde(rename = "instructions")]
    Instructions(EmptyArgs),
}

impl ApiRequest {
    pub fn command(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Status(_) => "status",
            Self::Check(_) => "check",
            Self::PatchList(_) => "patch.list",
            Self::PatchShow(_) => "patch.show",
            Self::PatchCreate(_) => "patch.create",
            Self::PatchSelect(_) => "patch.select",
            Self::PatchEdit(_) => "patch.edit",
            Self::PatchRefresh(_) => "patch.refresh",
            Self::PatchFinish(_) => "patch.finish",
            Self::PatchRemove(_) => "patch.remove",
            Self::PatchDisable(_) => "patch.disable",
            Self::PatchEnable(_) => "patch.enable",
            Self::ContractEdit(_) => "contract.edit",
            Self::Rebase(_) => "rebase",
            Self::Publish(_) => "publish",
            Self::OperationStatus(_) => "operation.status",
            Self::OperationContinue(_) => "operation.continue",
            Self::OperationAbort(_) => "operation.abort",
            Self::Instructions(_) => "instructions",
        }
    }

    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            Self::Status(_)
                | Self::Check(_)
                | Self::PatchList(_)
                | Self::PatchShow(_)
                | Self::OperationStatus(_)
                | Self::Instructions(_)
        )
    }
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyArgs {}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InitArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downstream_remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downstream_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exports: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookkeeping_patch: Option<String>,
    #[serde(default)]
    pub bookkeeping_scope: Vec<String>,
    #[serde(default)]
    pub allow_base: Vec<String>,
    #[serde(default)]
    pub required_text: Vec<RequiredText>,
}

impl InitArgs {
    pub fn is_bootstrap(&self) -> bool {
        self.upstream_remote.is_some()
            || self.upstream_url.is_some()
            || self.upstream_ref.is_some()
            || self.downstream_remote.is_some()
            || self.downstream_branch.is_some()
            || self.base.is_some()
            || self.ledger.is_some()
            || self.exports.is_some()
            || self.bookkeeping_patch.is_some()
            || !self.bookkeeping_scope.is_empty()
            || !self.allow_base.is_empty()
            || !self.required_text.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, JsonSchema, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum CheckScope {
    Repository,
    Staged,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckArgs {
    pub scope: CheckScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchName {
    pub patch: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchTransitionArgs {
    pub patch: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchCreateArgs {
    pub name: String,
    pub kind: PatchKind,
    pub purpose: String,
    pub upstream_status: String,
    pub drop_when: String,
    pub scope: Vec<String>,
    #[serde(default)]
    pub checks: Vec<Check>,
}

impl From<PatchCreateArgs> for Patch {
    fn from(value: PatchCreateArgs) -> Self {
        Self {
            name: value.name,
            kind: value.kind,
            purpose: value.purpose,
            upstream_status: value.upstream_status,
            drop_when: value.drop_when,
            scope: value.scope,
            checks: value.checks,
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScopeEdit {
    Set {
        patterns: Vec<String>,
    },
    AddRemove {
        add: Vec<String>,
        remove: Vec<String>,
    },
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchEditArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PatchKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<CheckEdit>,
}

/// Replacement or additive edit of a patch's declared checks.
#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckEdit {
    Set { checks: Vec<Check> },
    Add { checks: Vec<Check> },
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaptureSource {
    Staged,
    All,
    Paths { pathspecs: Vec<String> },
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchRefreshArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    pub capture: CaptureSource,
    /// Unapply patches above this one. Default is false: create a new top patch instead.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rewrite_below: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractEditArgs {
    #[serde(default)]
    pub clear: bool,
    #[serde(default)]
    pub allow_base: Vec<String>,
    #[serde(default)]
    pub required_text: Vec<RequiredText>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RebaseArgs {
    pub onto: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperationAbortArgs {
    pub confirmed: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApiResponse {
    Success {
        protocol_version: u32,
        command: String,
        mode: ExecutionMode,
        #[serde(skip_serializing_if = "Option::is_none")]
        operation_id: Option<String>,
        result: Box<CommandResult>,
        notices: Vec<Notice>,
    },
    Error {
        protocol_version: u32,
        command: String,
        mode: ExecutionMode,
        error: ApiError,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult {
    Init(InitResult),
    Status(Box<StatusResult>),
    Check(CheckResult),
    PatchList(PatchListResult),
    PatchShow(PatchShowResult),
    PatchCreate(PatchCreateResult),
    PatchSelect(PatchSelectResult),
    PatchEdit(PatchEditResult),
    PatchRefresh(PatchRefreshResult),
    PatchFinish(PatchFinishResult),
    PatchRemove(PatchTransitionResult),
    PatchDisable(PatchTransitionResult),
    PatchEnable(PatchTransitionResult),
    ContractEdit(ContractEditResult),
    Rebase(Box<RebaseResult>),
    Publish(PublishResult),
    OperationStatus(Box<OperationStatusResult>),
    OperationContinue(Box<OperationContinueResult>),
    OperationAbort(OperationAbortResult),
    Instructions(InstructionsResult),
    Plan(MutationPlan),
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct Outcome {
    pub result: CommandResult,
    pub notices: Vec<Notice>,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct Notice {
    pub code: NoticeCode,
    pub message: String,
    pub details: NoticeDetails,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeCode {
    UpstreamPatchDropped,
    ActivePatchRetained,
    HookModifiedIndex,
    NoChangesCaptured,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NoticeDetails {
    None,
    Patch {
        patch: String,
    },
    Paths {
        paths: Vec<String>,
    },
    Dropped {
        patch: String,
        commit: String,
        recovery_tag: String,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    pub causes: Vec<String>,
    pub details: ErrorDetails,
    pub retryable: bool,
    pub suggested_command: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    InvalidRequest,
    UnsupportedProtocol,
    RepositoryNotFound,
    ManifestInvalid,
    DirtyWorktree,
    ActivePatchRequired,
    ActivePatchExists,
    PatchNotFound,
    StagedScopeViolation,
    CaptureConflict,
    OperationInProgress,
    OperationConflict,
    CheckFailed,
    RemoteAdvanced,
    PublicationRejected,
    SubprocessFailed,
    InternalError,
}

impl std::fmt::Display for ApiErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "invalid_request",
            Self::UnsupportedProtocol => "unsupported_protocol",
            Self::RepositoryNotFound => "repository_not_found",
            Self::ManifestInvalid => "manifest_invalid",
            Self::DirtyWorktree => "dirty_worktree",
            Self::ActivePatchRequired => "active_patch_required",
            Self::ActivePatchExists => "active_patch_exists",
            Self::PatchNotFound => "patch_not_found",
            Self::StagedScopeViolation => "staged_scope_violation",
            Self::CaptureConflict => "capture_conflict",
            Self::OperationInProgress => "operation_in_progress",
            Self::OperationConflict => "operation_conflict",
            Self::CheckFailed => "check_failed",
            Self::RemoteAdvanced => "remote_advanced",
            Self::PublicationRejected => "publication_rejected",
            Self::SubprocessFailed => "subprocess_failed",
            Self::InternalError => "internal_error",
        })
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorDetails {
    None,
    Paths {
        patch: Option<String>,
        paths: Vec<String>,
    },
    Patch {
        requested: Option<String>,
        available: Vec<String>,
        active: Option<String>,
    },
    Operation {
        operation_id: String,
        kind: String,
        phase: String,
        next_actions: Vec<String>,
    },
    Check {
        findings: Vec<CheckFinding>,
    },
    Remote {
        remote: String,
        git_ref: String,
        expected: Option<String>,
        actual: Option<String>,
        stderr: String,
    },
    Subprocess {
        program: String,
        args: Vec<String>,
        cwd: String,
        exit_code: Option<i32>,
        stderr: String,
    },
    Request {
        field: Option<String>,
        issue: String,
    },
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct CheckFinding {
    pub code: String,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct CheckResult {
    pub scope: CheckScope,
    pub ok: bool,
    pub patch: Option<String>,
    pub checked_paths: Vec<String>,
    pub rejected_paths: Vec<String>,
    pub findings: Vec<CheckFinding>,
    pub canonical_base: Option<String>,
    pub stack_base: Option<String>,
    pub patch_count: Option<usize>,
    pub declared_checks: Option<usize>,
    pub source_tree: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct InitResult {
    pub created: bool,
    pub hydrated: bool,
    pub manifest: String,
    pub base_target: BaseTarget,
    pub bookkeeping_commit: String,
    pub check: CheckResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct StatusResult {
    pub repository: String,
    pub manifest: String,
    pub current_branch: Option<String>,
    pub declared_branch: String,
    pub downstream_remote: String,
    pub downstream_sha: Option<String>,
    pub upstream_remote: String,
    pub upstream_fetch_ref: String,
    pub selected_target: String,
    pub canonical_base: String,
    pub stack_base: String,
    pub patches: Vec<PatchSummary>,
    pub active_patch: Option<ActivePatchState>,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
    pub operation: Option<OperationState>,
    pub check: CheckSummary,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct CheckSummary {
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchSummary {
    pub name: String,
    pub kind: PatchKind,
    pub state: String,
    pub commit: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchListResult {
    pub patches: Vec<PatchSummary>,
    pub active_patch: Option<ActivePatchState>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchShowResult {
    pub patch: Patch,
    pub commit: Option<String>,
    pub changed_paths: Vec<String>,
    pub export: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchCreateResult {
    pub active_patch: ActivePatchState,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchSelectResult {
    pub previous: Option<ActivePatchState>,
    pub active_patch: ActivePatchState,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchEditResult {
    pub patch: Patch,
    pub old_commit: String,
    pub new_commit: String,
    pub generated_paths: Vec<String>,
    pub check: CheckResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchRefreshResult {
    pub patch: String,
    pub capture: CaptureSource,
    pub captured_paths: Vec<String>,
    pub old_commit: Option<String>,
    pub new_commit: String,
    pub generated_paths: Vec<String>,
    pub check: CheckResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchFinishResult {
    pub patch: String,
    pub check: CheckResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PatchTransitionResult {
    pub patch: String,
    pub commit: String,
    pub recovery_tag: String,
    pub new_tip: String,
    pub check: CheckResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct ContractEditResult {
    pub contracts: Contracts,
    pub generated_paths: Vec<String>,
    pub check: CheckResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct MutationPlan {
    pub command: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub hooks: Vec<String>,
    pub ref_updates: Vec<String>,
    pub paths: Vec<String>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct RebaseResult {
    pub selected_target: String,
    pub old_base: String,
    pub old_tip: String,
    pub new_base: String,
    pub new_tip: String,
    pub recovery_tag: String,
    pub recovery_tag_object: String,
    pub report_path: String,
    pub report_object_id: String,
    pub dropped_patches: Vec<String>,
    pub path_changed_patches: Vec<String>,
    pub check: CheckResult,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct PublishResult {
    pub branch: String,
    pub head: String,
    pub already_published: bool,
    pub fast_forward: bool,
    pub recovery_tags: Vec<String>,
    pub pushed_refs: Vec<String>,
    pub expected_lease: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct OperationStatusResult {
    pub operation: Option<OperationState>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct OperationContinueResult {
    pub operation: Option<OperationState>,
    pub result: Option<Box<CommandResult>>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Serialize)]
pub struct OperationAbortResult {
    pub operation_id: String,
    pub restored_tip: String,
    pub check: CheckResult,
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
            operation_id: None,
        }
    }
}

impl ApiResponse {
    pub fn success(command: &str, mode: ExecutionMode, outcome: Outcome) -> Self {
        Self::Success {
            protocol_version: PROTOCOL_VERSION,
            command: command.to_string(),
            mode,
            operation_id: outcome.operation_id,
            result: Box::new(outcome.result),
            notices: outcome.notices,
        }
    }

    pub fn error(command: &str, mode: ExecutionMode, error: ApiError) -> Self {
        Self::Error {
            protocol_version: PROTOCOL_VERSION,
            command: command.to_string(),
            mode,
            error,
        }
    }
}

pub fn schema_document(kind: SchemaKind) -> serde_json::Value {
    match kind {
        SchemaKind::Bundle => serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "forkctl_protocol_version": PROTOCOL_VERSION,
            "schemas": {
                "manifest": schemars::schema_for!(Manifest),
                "invocation": schemars::schema_for!(ApiInvocation),
                "response": schemars::schema_for!(ApiResponse),
                "active_state": schemars::schema_for!(ActivePatchState),
                "operation": schemars::schema_for!(OperationState),
            }
        }),
        SchemaKind::Manifest => serde_json::to_value(schemars::schema_for!(Manifest)).unwrap(),
        SchemaKind::Invocation => {
            serde_json::to_value(schemars::schema_for!(ApiInvocation)).unwrap()
        }
        SchemaKind::Response => serde_json::to_value(schemars::schema_for!(ApiResponse)).unwrap(),
        SchemaKind::ActiveState => {
            serde_json::to_value(schemars::schema_for!(ActivePatchState)).unwrap()
        }
        SchemaKind::Operation => {
            serde_json::to_value(schemars::schema_for!(OperationState)).unwrap()
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum SchemaKind {
    Bundle,
    Manifest,
    Invocation,
    Response,
    ActiveState,
    Operation,
}
