use ctl_core::{
    Document, Fields, MessageKind, Notice as UiNotice, NoticeLevel, Present, Section, Table, Text,
};
use serde::{Serialize, Serializer};

use crate::protocol::{
    ApiError, ApiErrorCode, ApiResponse, CheckResult, CommandResult, ExecutionMode,
    OperationStatusResult, PatchListResult, PatchShowResult, StatusResult,
};

pub enum Report {
    Response {
        response: Box<ApiResponse>,
        update_notice: Option<String>,
    },
    Json(serde_json::Value),
    Text(String),
}

impl Report {
    pub fn response(response: ApiResponse, update_notice: Option<String>) -> Self {
        Self::Response {
            response: Box::new(response),
            update_notice,
        }
    }

    pub fn json(value: serde_json::Value) -> Self {
        Self::Json(value)
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }
}

impl Serialize for Report {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Response { response, .. } => response.serialize(serializer),
            Self::Json(value) => value.serialize(serializer),
            Self::Text(value) => value.serialize(serializer),
        }
    }
}

impl Present for Report {
    fn present(&self) -> Document {
        match self {
            Self::Response {
                response,
                update_notice,
            } => present_response(response, update_notice.as_deref()),
            Self::Json(value) => Document::new().verbatim(value.to_string()),
            Self::Text(value) => Document::new().verbatim(value.clone()),
        }
    }

    fn message_kind(&self) -> MessageKind {
        match self {
            Self::Response { response, .. }
                if matches!(response.as_ref(), ApiResponse::Error { .. }) =>
            {
                MessageKind::Error
            }
            _ => MessageKind::Success,
        }
    }

    fn exit_code(&self) -> u8 {
        match self {
            Self::Response { response, .. }
                if matches!(
                    response.as_ref(),
                    ApiResponse::Error {
                        error: ApiError {
                            code: ApiErrorCode::InvalidRequest,
                            ..
                        },
                        ..
                    }
                ) =>
            {
                2
            }
            _ => self.message_kind().default_exit_code(),
        }
    }
}

fn present_response(response: &ApiResponse, update_notice: Option<&str>) -> Document {
    match response {
        ApiResponse::Success {
            command,
            mode,
            result,
            notices,
            ..
        } => {
            if let CommandResult::Instructions(result) = result.as_ref() {
                return Document::new().verbatim(result.markdown.clone());
            }
            let mut document = Document::new().heading(format!("forkctl · {command}"));
            if *mode == ExecutionMode::Plan {
                document = document.notice(UiNotice::new(
                    NoticeLevel::Success,
                    "mutation plan ready; no state changed",
                ));
            }
            document = append_result(document, result);
            for notice in notices {
                document = document.notice(
                    UiNotice::new(NoticeLevel::Warning, notice.message.clone())
                        .code(format!("{:?}", notice.code)),
                );
            }
            if let Some(message) = update_notice {
                document = document.notice(UiNotice::new(NoticeLevel::Warning, message));
            }
            document
        }
        ApiResponse::Error { command, error, .. } => present_error(command, error),
    }
}

fn append_result(document: Document, result: &CommandResult) -> Document {
    match result {
        CommandResult::Init(result) => document.fields(
            Fields::new()
                .row("created", result.created.to_string())
                .row("hydrated", result.hydrated.to_string())
                .row("manifest", result.manifest.clone())
                .row("base", result.base_target.commit.clone())
                .row("bookkeeping", result.bookkeeping_commit.clone()),
        ),
        CommandResult::Status(result) => append_status(document, result),
        CommandResult::Check(result) => append_check(document, result),
        CommandResult::PatchList(result) => append_patch_list(document, result),
        CommandResult::PatchShow(result) => append_patch_show(document, result),
        CommandResult::Instructions(result) => document.verbatim(result.markdown.clone()),
        _ => append_mutation_result(document, result),
    }
}

fn append_mutation_result(document: Document, result: &CommandResult) -> Document {
    match result {
        CommandResult::PatchCreate(_)
        | CommandResult::PatchSelect(_)
        | CommandResult::PatchEdit(_)
        | CommandResult::PatchRefresh(_)
        | CommandResult::PatchFinish(_)
        | CommandResult::PatchRemove(_)
        | CommandResult::PatchDisable(_)
        | CommandResult::PatchEnable(_) => append_patch_mutation(document, result),
        CommandResult::ContractEdit(result) => document.fields(
            Fields::new()
                .row(
                    "allowed base globs",
                    result.contracts.allow_base.len().to_string(),
                )
                .row(
                    "required text assertions",
                    result.contracts.required_text.len().to_string(),
                )
                .row("check", "passed"),
        ),
        CommandResult::CommitMessageMigration(result) => document.fields(
            Fields::new()
                .row("old tip", result.old_tip.clone())
                .row("new tip", result.new_tip.clone())
                .row("recovery", result.recovery_tag.clone())
                .row("rewritten", display_list(&result.rewritten_patches))
                .row("check", "passed"),
        ),
        CommandResult::Rebase(result) => document.fields(
            Fields::new()
                .row("target", result.selected_target.clone())
                .row("old tip", result.old_tip.clone())
                .row("new tip", result.new_tip.clone())
                .row("recovery", result.recovery_tag.clone())
                .row("report", result.report_path.clone())
                .row("dropped", display_list(&result.dropped_patches))
                .row("paths changed", display_list(&result.path_changed_patches)),
        ),
        CommandResult::Publish(result) => append_publish(document, result),
        CommandResult::OperationStatus(result) => append_operation(document, result),
        CommandResult::OperationContinue(result) => document.fields(
            Fields::new().row(
                "operation",
                result
                    .operation
                    .as_ref()
                    .map_or_else(|| "completed".into(), |value| value.id.clone()),
            ),
        ),
        CommandResult::OperationAbort(result) => document.fields(
            Fields::new()
                .row("operation", result.operation_id.clone())
                .row("restored tip", result.restored_tip.clone()),
        ),
        CommandResult::Plan(result) => document.fields(
            Fields::new()
                .row("command", result.command.clone())
                .row("reads", display_list(&result.reads))
                .row("writes", display_list(&result.writes))
                .row("hooks", display_list(&result.hooks))
                .row("ref updates", display_list(&result.ref_updates))
                .row("paths", display_list(&result.paths))
                .row(
                    "confirmation",
                    if result.requires_confirmation {
                        "required"
                    } else {
                        "not required"
                    },
                ),
        ),
        _ => unreachable!("read-only results handled before mutation results"),
    }
}

fn append_patch_mutation(document: Document, result: &CommandResult) -> Document {
    match result {
        CommandResult::PatchCreate(result) => document
            .fields(Fields::new().row("active patch", result.active_patch.name().to_string())),
        CommandResult::PatchSelect(result) => document.fields(
            Fields::new()
                .row(
                    "previous",
                    result
                        .previous
                        .as_ref()
                        .map_or_else(|| "none".into(), |value| value.name().to_string()),
                )
                .row("active patch", result.active_patch.name().to_string()),
        ),
        CommandResult::PatchEdit(result) => document.fields(
            Fields::new()
                .row("patch", result.patch.name.clone())
                .row("old commit", result.old_commit.clone())
                .row("new commit", result.new_commit.clone())
                .row("generated", display_list(&result.generated_paths)),
        ),
        CommandResult::PatchRefresh(result) => document.fields(
            Fields::new()
                .row("patch", result.patch.clone())
                .row("captured", display_list(&result.captured_paths))
                .row(
                    "old commit",
                    result.old_commit.clone().unwrap_or_else(|| "draft".into()),
                )
                .row("new commit", result.new_commit.clone())
                .row("generated", display_list(&result.generated_paths)),
        ),
        CommandResult::PatchFinish(result) => document.fields(
            Fields::new()
                .row("patch", result.patch.clone())
                .row("check", "passed"),
        ),
        CommandResult::PatchRemove(result)
        | CommandResult::PatchDisable(result)
        | CommandResult::PatchEnable(result) => document.fields(
            Fields::new()
                .row("patch", result.patch.clone())
                .row("former commit", result.commit.clone())
                .row("new tip", result.new_tip.clone())
                .row("recovery", result.recovery_tag.clone())
                .row("check", "passed"),
        ),
        _ => unreachable!("non-patch result passed to patch presenter"),
    }
}

fn append_publish(document: Document, result: &crate::protocol::PublishResult) -> Document {
    document.fields(
        Fields::new()
            .row("branch", result.branch.clone())
            .row("head", result.head.clone())
            .row("mode", result.mode.to_string())
            .row(
                "publication",
                if result.already_published {
                    "already published".into()
                } else if result.proposal_branch.is_some() {
                    format!(
                        "proposal {}",
                        result
                            .proposal_url
                            .as_deref()
                            .unwrap_or(result.proposal_branch.as_deref().unwrap_or("open"))
                    )
                } else if result.fast_forward {
                    "fast-forward".into()
                } else {
                    "leased rewrite".into()
                },
            )
            .row(
                "recovery",
                if result.recovery_tags.is_empty() {
                    "not required".into()
                } else {
                    result.recovery_tags.join(", ")
                },
            )
            .row("lease", result.expected_lease.clone()),
    )
}

fn append_status(document: Document, result: &StatusResult) -> Document {
    let document = document.fields(
        Fields::new()
            .row("repository", result.repository.clone())
            .row("manifest", result.manifest.clone())
            .row(
                "branch",
                format!(
                    "{} (declared {})",
                    result.current_branch.as_deref().unwrap_or("detached"),
                    result.declared_branch
                ),
            )
            .row("publish", result.publish_mode.to_string())
            .row(
                "downstream",
                format!(
                    "{} {}",
                    result.downstream_remote,
                    result.downstream_sha.as_deref().unwrap_or("unavailable")
                ),
            )
            .row(
                "base",
                format!("{} {}", result.selected_target, result.stack_base),
            )
            .row(
                "active",
                result
                    .active_patch
                    .as_ref()
                    .map_or_else(|| "none".into(), |value| value.name().to_string()),
            )
            .row("staged", display_list(&result.staged))
            .row("unstaged", display_list(&result.unstaged))
            .row("untracked", display_list(&result.untracked))
            .row(
                "check",
                if result.check.ok {
                    "passed".into()
                } else {
                    result
                        .check
                        .message
                        .clone()
                        .unwrap_or_else(|| "failed".into())
                },
            ),
    );
    let document = append_patch_rows(document, &result.patches);
    append_operation(
        document,
        &OperationStatusResult {
            operation: result.operation.clone(),
        },
    )
}

fn append_check(document: Document, result: &CheckResult) -> Document {
    document
        .notice(UiNotice::new(NoticeLevel::Success, "check passed"))
        .fields(
            Fields::new()
                .row("scope", format!("{:?}", result.scope).to_lowercase())
                .row(
                    "patch",
                    result.patch.clone().unwrap_or_else(|| "repository".into()),
                )
                .row("checked paths", display_list(&result.checked_paths))
                .row(
                    "canonical",
                    result
                        .canonical_base
                        .clone()
                        .unwrap_or_else(|| "n/a".into()),
                )
                .row(
                    "stack base",
                    result.stack_base.clone().unwrap_or_else(|| "n/a".into()),
                )
                .row(
                    "patch count",
                    result
                        .patch_count
                        .map_or_else(|| "n/a".into(), |value| value.to_string()),
                )
                .row(
                    "declared checks",
                    result
                        .declared_checks
                        .map_or_else(|| "n/a".into(), |value| value.to_string()),
                )
                .row(
                    "source tree",
                    result.source_tree.clone().unwrap_or_else(|| "n/a".into()),
                ),
        )
}

fn append_patch_list(document: Document, result: &PatchListResult) -> Document {
    append_patch_rows(document, &result.patches)
}

fn append_patch_rows(document: Document, patches: &[crate::protocol::PatchSummary]) -> Document {
    if patches.is_empty() {
        return document.paragraph(Text::new().muted("no patches"));
    }
    let table = patches.iter().fold(
        Table::new(["state", "patch", "kind", "subject", "commit", "active"]).token_column(1),
        |table, patch| {
            table.row([
                patch.state.clone(),
                patch.name.clone(),
                format!("{:?}", patch.kind).to_lowercase(),
                patch.subject.clone(),
                patch.commit.clone().unwrap_or_else(|| "draft".into()),
                if patch.active {
                    "yes".into()
                } else {
                    String::new()
                },
            ])
        },
    );
    document.table(table)
}

fn append_patch_show(document: Document, result: &PatchShowResult) -> Document {
    document.fields(
        Fields::new()
            .row("patch", result.patch.name.clone())
            .row("kind", format!("{:?}", result.patch.kind).to_lowercase())
            .row("subject", result.subject.clone())
            .row("purpose", result.patch.purpose.clone())
            .row("upstream status", result.patch.upstream_status.clone())
            .row("drop when", result.patch.drop_when.clone())
            .row("scope", display_list(&result.patch.scope))
            .row(
                "commit",
                result.commit.clone().unwrap_or_else(|| "draft".into()),
            )
            .row("changed paths", display_list(&result.changed_paths))
            .row(
                "export",
                result.export.clone().unwrap_or_else(|| "none".into()),
            )
            .row("active", result.active.to_string()),
    )
}

fn append_operation(document: Document, result: &OperationStatusResult) -> Document {
    let body = match &result.operation {
        Some(operation) => Document::new().fields(
            Fields::new()
                .row("id", operation.id.clone())
                .row("kind", format!("{:?}", operation.kind).to_lowercase())
                .row("phase", operation.phase.clone())
                .row("recovery", operation.recovery.tag.clone())
                .row("next", display_list(&operation.next_actions)),
        ),
        None => Document::new().paragraph(Text::new().muted("none")),
    };
    document.section(Section::new("operation", body))
}

fn present_error(command: &str, error: &ApiError) -> Document {
    let mut fields = Fields::new().row("code", error.code.to_string());
    for cause in &error.causes {
        fields = fields.row("caused by", cause.clone());
    }
    if let Some(command) = &error.suggested_command {
        fields = fields.row("next", Text::new().token(command.clone()));
    }
    Document::new()
        .heading(format!("forkctl · {command} · error"))
        .notice(UiNotice::new(NoticeLevel::Error, error.message.clone()))
        .fields(fields)
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values.join(", ")
    }
}
