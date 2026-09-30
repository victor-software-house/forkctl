use crate::protocol::{ApiError, ApiErrorCode, ErrorDetails};
use crate::state::OperationState;
use std::ffi::OsString;
use std::fmt::Display;
use std::path::Path;
use std::process::Output;

pub(crate) type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub(crate) enum AppError {
    Domain {
        error: Box<DomainError>,
        causes: Vec<String>,
    },
    Internal(anyhow::Error),
}

impl AppError {
    pub(crate) fn internal(
        error: impl Into<anyhow::Error>,
        context: impl Display + Send + Sync + 'static,
    ) -> Self {
        Self::Internal(error.into().context(context))
    }

    pub(crate) fn internal_message(message: impl Into<String>) -> Self {
        Self::Internal(anyhow::Error::msg(message.into()))
    }

    pub(crate) fn context(self, context: impl Into<String>) -> Self {
        let context = context.into();
        match self {
            Self::Domain { error, mut causes } => {
                causes.insert(0, context);
                Self::Domain { error, causes }
            }
            Self::Internal(error) => Self::Internal(error.context(context)),
        }
    }

    pub(crate) fn to_api_error(&self) -> ApiError {
        match self {
            Self::Domain { error, causes } => error.to_api_error(causes.clone()),
            Self::Internal(error) => ApiError {
                code: ApiErrorCode::InternalError,
                message: error.to_string(),
                causes: error.chain().skip(1).map(ToString::to_string).collect(),
                details: ErrorDetails::None,
                retryable: false,
                suggested_command: None,
            },
        }
    }
}

impl From<DomainError> for AppError {
    fn from(error: DomainError) -> Self {
        Self::Domain {
            error: Box::new(error),
            causes: Vec::new(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Domain { error, .. } => error.fmt(formatter),
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AppError {}

/// Explicitly marks an infrastructure failure as internal.
///
/// `AppError` intentionally has no blanket `From<anyhow::Error>` or I/O conversion: callers must
/// choose domain classification or invoke this adapter with actionable context.
pub(crate) trait InternalResultExt<T> {
    fn internal(self, context: impl Display + Send + Sync + 'static) -> AppResult<T>;
}

impl<T, E> InternalResultExt<T> for std::result::Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn internal(self, context: impl Display + Send + Sync + 'static) -> AppResult<T> {
        self.map_err(|error| AppError::internal(error, context))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DomainError {
    code: ApiErrorCode,
    message: String,
    details: ErrorDetails,
    retryable: bool,
    suggested_command: Option<String>,
}

impl DomainError {
    pub(crate) fn invalid_request(message: impl Into<String>) -> Self {
        let message = message.into();
        Self::new(
            ApiErrorCode::InvalidRequest,
            message.clone(),
            ErrorDetails::Request {
                field: None,
                issue: message,
            },
        )
    }

    pub(crate) fn repository_not_found(message: impl Into<String>) -> Self {
        Self::new(
            ApiErrorCode::RepositoryNotFound,
            message,
            ErrorDetails::None,
        )
    }

    pub(crate) fn manifest_invalid(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::ManifestInvalid, message, ErrorDetails::None)
    }

    pub(crate) fn dirty_worktree(paths: Vec<String>) -> Self {
        Self::new(
            ApiErrorCode::DirtyWorktree,
            "worktree is not clean",
            ErrorDetails::Paths { patch: None, paths },
        )
    }

    pub(crate) fn active_patch_required() -> Self {
        Self::new(
            ApiErrorCode::ActivePatchRequired,
            "an active patch is required",
            ErrorDetails::Patch {
                requested: None,
                available: Vec::new(),
                active: None,
            },
        )
        .suggest("forkctl patch create NAME ... or forkctl patch select NAME")
    }

    pub(crate) fn active_patch_exists(active: String) -> Self {
        Self::new(
            ApiErrorCode::ActivePatchExists,
            format!("an active patch already exists: {active}"),
            ErrorDetails::Patch {
                requested: None,
                available: Vec::new(),
                active: Some(active),
            },
        )
        .suggest("forkctl patch finish")
    }

    pub(crate) fn patch_not_found(
        requested: impl Into<String>,
        available: Vec<String>,
        active: Option<String>,
    ) -> Self {
        let requested = requested.into();
        Self::new(
            ApiErrorCode::PatchNotFound,
            format!("patch not found: {requested}"),
            ErrorDetails::Patch {
                requested: Some(requested),
                available,
                active,
            },
        )
        .suggest("forkctl patch list")
    }

    pub(crate) fn staged_scope_violation(patch: String, paths: Vec<String>) -> Self {
        Self::new(
            ApiErrorCode::StagedScopeViolation,
            format!(
                "captured paths are outside patch {patch}: {}",
                paths.join(", ")
            ),
            ErrorDetails::Paths {
                patch: Some(patch),
                paths,
            },
        )
        .suggest("forkctl patch edit --add-scope GLOB")
    }

    pub(crate) fn rewrite_below_required(patch: &str, above: &[String]) -> Self {
        let above_list = above.join(", ");
        Self::new(
            ApiErrorCode::OperationConflict,
            format!(
                "patch {patch} has {} patch(es) above it ({above_list}). Everyday follow-up is a new top patch. Pass --rewrite-below only when you intend to unapply those patches.",
                above.len()
            ),
            ErrorDetails::Request {
                field: Some("rewrite_below".into()),
                issue: format!("patches above {patch}: {above_list}"),
            },
        )
        .suggest("mise run fork patch create NAME")
    }

    pub(crate) fn capture_conflict(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::CaptureConflict, message, ErrorDetails::None)
    }

    pub(crate) fn operation_in_progress(operation: &OperationState) -> Self {
        Self::new(
            ApiErrorCode::OperationInProgress,
            format!("operation {} is in progress", operation.id),
            ErrorDetails::Operation {
                operation_id: operation.id.clone(),
                kind: format!("{:?}", operation.kind).to_lowercase(),
                phase: operation.phase.clone(),
                next_actions: operation.next_actions.clone(),
            },
        )
        .suggest("forkctl operation status")
    }

    pub(crate) fn check_failed(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::CheckFailed, message, ErrorDetails::None)
    }

    pub(crate) fn declared_checks_failed(findings: Vec<crate::protocol::CheckFinding>) -> Self {
        Self::new(
            ApiErrorCode::CheckFailed,
            format!("{} declared patch check(s) failed", findings.len()),
            ErrorDetails::Check { findings },
        )
        .suggest("forkctl patch show PATCH")
    }

    pub(crate) fn operation_conflict(
        message: impl Into<String>,
        operation: Option<&OperationState>,
    ) -> Self {
        let details = operation.map_or(ErrorDetails::None, |operation| ErrorDetails::Operation {
            operation_id: operation.id.clone(),
            kind: format!("{:?}", operation.kind).to_lowercase(),
            phase: operation.phase.clone(),
            next_actions: operation.next_actions.clone(),
        });
        Self::new(ApiErrorCode::OperationConflict, message, details)
            .suggest("forkctl operation status")
    }

    pub(crate) fn remote_advanced(
        remote: String,
        git_ref: String,
        expected: String,
        actual: String,
    ) -> Self {
        Self::new(
            ApiErrorCode::RemoteAdvanced,
            format!("remote {git_ref} advanced to {actual}; expected {expected}"),
            ErrorDetails::Remote {
                remote,
                git_ref,
                expected: Some(expected),
                actual: Some(actual),
                stderr: String::new(),
            },
        )
        .retryable()
    }

    pub(crate) fn publication_rejected(error: &Self) -> Self {
        Self::new(
            ApiErrorCode::PublicationRejected,
            "remote rejected atomic publication",
            error.details.clone(),
        )
    }

    pub(crate) fn publication_restoration_failed(
        publication: &Self,
        restoration: impl std::fmt::Display,
    ) -> Self {
        let mut combined = publication.clone();
        combined.message = format!(
            "{}; also failed to restore local stack: {restoration}",
            publication.message
        );
        combined.retryable = false;
        combined
    }

    pub(crate) fn publication_ref_mismatch(
        remote: String,
        git_ref: String,
        expected: String,
        actual: String,
    ) -> Self {
        Self::new(
            ApiErrorCode::PublicationRejected,
            format!("remote {git_ref} points to {actual}, expected {expected}"),
            ErrorDetails::Remote {
                remote,
                git_ref,
                expected: Some(expected),
                actual: Some(actual),
                stderr: String::new(),
            },
        )
    }

    pub(crate) fn subprocess(
        program: &str,
        args: &[OsString],
        cwd: &Path,
        output: &Output,
    ) -> Self {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let invocation = if args.is_empty() {
            program.to_string()
        } else {
            format!("{program} {}", args.join(" "))
        };
        let message = if stderr.is_empty() {
            format!("{invocation} failed with {}", output.status)
        } else {
            format!("{invocation}: {stderr}")
        };
        Self::new(
            ApiErrorCode::SubprocessFailed,
            message,
            ErrorDetails::Subprocess {
                program: program.to_string(),
                args,
                cwd: cwd.display().to_string(),
                exit_code: output.status.code(),
                stderr,
            },
        )
    }

    pub(crate) fn wrong_tracking(branch: &str, actual: Option<&str>, expected: &str) -> Self {
        let message = match actual {
            Some(actual) => format!("branch {branch} tracks {actual}, expected {expected}"),
            None => format!("branch {branch} tracks no upstream, expected {expected}"),
        };
        Self::invalid_request(message)
            .suggest(format!("git branch --set-upstream-to={expected} {branch}"))
    }

    pub(crate) fn program_unavailable(
        program: &str,
        args: &[&str],
        cwd: &Path,
        cause: &str,
    ) -> Self {
        Self::new(
            ApiErrorCode::SubprocessFailed,
            format!("{program} could not run: {cause}"),
            ErrorDetails::Subprocess {
                program: program.to_string(),
                args: args.iter().map(ToString::to_string).collect(),
                cwd: cwd.display().to_string(),
                exit_code: None,
                stderr: cause.to_string(),
            },
        )
    }

    /// Marks a failure that happened after the proposal branch was pushed. Rerunning the
    /// proposal is safe, so the error is retryable.
    pub(crate) fn after_proposal_push(mut self, proposal_branch: &str) -> Self {
        self.message = format!(
            "proposal branch {proposal_branch} was pushed, but its pull request was not updated: {}",
            self.message
        );
        self.retryable().suggest("forkctl publish --propose")
    }

    fn new(code: ApiErrorCode, message: impl Into<String>, details: ErrorDetails) -> Self {
        Self {
            code,
            message: message.into(),
            details,
            retryable: false,
            suggested_command: None,
        }
    }

    fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    fn suggest(mut self, command: impl Into<String>) -> Self {
        self.suggested_command = Some(command.into());
        self
    }

    pub(crate) fn to_api_error(&self, causes: Vec<String>) -> ApiError {
        ApiError {
            code: self.code,
            message: self.message.clone(),
            causes,
            details: self.details.clone(),
            retryable: self.retryable,
            suggested_command: self.suggested_command.clone(),
        }
    }
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_restoration_failure_preserves_the_original_error_contract() {
        let publication = DomainError::remote_advanced(
            "origin".into(),
            "refs/heads/main".into(),
            "expected".into(),
            "actual".into(),
        );
        let combined =
            DomainError::publication_restoration_failed(&publication, "git reset --soft failed")
                .to_api_error(Vec::new());

        assert!(matches!(combined.code, ApiErrorCode::RemoteAdvanced));
        assert!(!combined.retryable);
        assert!(combined.message.contains("advanced to actual"));
        assert!(combined.message.contains("git reset --soft failed"));
    }

    #[test]
    fn domain_error_keeps_public_classification_and_context() {
        let error = AppError::from(DomainError::invalid_request("bad request"))
            .context("while dispatching command");

        let api = error.to_api_error();
        assert_eq!(api.code.to_string(), "invalid_request");
        assert_eq!(api.message, "bad request");
        assert_eq!(api.causes, ["while dispatching command"]);
    }

    #[test]
    fn internal_error_preserves_contextual_causes() {
        let error = AppError::internal(std::io::Error::other("disk failure"), "write state");

        let api = error.to_api_error();
        assert_eq!(api.code.to_string(), "internal_error");
        assert_eq!(api.message, "write state");
        assert_eq!(api.causes, ["disk failure"]);
    }
}
