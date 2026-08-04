use crate::protocol::{
    ApiError, ApiResponse, CheckResult, ColorMode, CommandResult, Notice, OperationStatusResult,
    PatchListResult, PatchShowResult, StatusResult,
};
use anstyle::{AnsiColor, Style};
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use std::fmt::Write as _;
use std::io::{self, Write as _};

const HEADING: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan)));
const SUCCESS: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)));
const WARNING: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow)));
const FAILURE: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)));
const MUTED: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::BrightBlack)));

pub fn emit_pretty(response: &ApiResponse, color: ColorMode, quiet: bool) -> io::Result<()> {
    if quiet && matches!(response, ApiResponse::Success { .. }) {
        return Ok(());
    }
    let rendered = render(response);
    let choice = match color {
        ColorMode::Auto => anstream::ColorChoice::Auto,
        ColorMode::Always => anstream::ColorChoice::Always,
        ColorMode::Never => anstream::ColorChoice::Never,
    };
    match response {
        ApiResponse::Success { .. } => {
            let mut stream = anstream::AutoStream::new(io::stdout().lock(), choice);
            stream.write_all(rendered.as_bytes())?;
            stream.flush()
        }
        ApiResponse::Error { .. } => {
            let mut stream = anstream::AutoStream::new(io::stderr().lock(), choice);
            stream.write_all(rendered.as_bytes())?;
            stream.flush()
        }
    }
}

pub fn emit_json(value: &impl serde::Serialize) -> io::Result<()> {
    let mut stream = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stream, value)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

pub fn emit_text(value: &str) -> io::Result<()> {
    let mut stream = io::stdout().lock();
    stream.write_all(value.as_bytes())?;
    if !value.ends_with('\n') {
        stream.write_all(b"\n")?;
    }
    stream.flush()
}

pub fn render(response: &ApiResponse) -> String {
    match response {
        ApiResponse::Success {
            command,
            mode,
            result,
            notices,
            ..
        } => render_success(command, *mode, result, notices),
        ApiResponse::Error { command, error, .. } => render_error(command, error),
    }
}

fn render_success(
    command: &str,
    mode: crate::protocol::ExecutionMode,
    result: &CommandResult,
    notices: &[Notice],
) -> String {
    if let CommandResult::Instructions(result) = result {
        return result.markdown.clone();
    }
    let mut output = String::new();
    heading(&mut output, command);
    if matches!(mode, crate::protocol::ExecutionMode::Plan) {
        success_line(&mut output, "mutation plan ready; no state changed");
    }
    render_result(&mut output, result);
    for notice in notices {
        let _ = writeln!(
            output,
            "{}notice{} · {:?} · {}",
            WARNING.render(),
            WARNING.render_reset(),
            notice.code,
            notice.message
        );
    }
    output
}

fn render_result(output: &mut String, result: &CommandResult) {
    match result {
        CommandResult::Init(result) => render_init(output, result),
        CommandResult::Status(result) => render_status(output, result),
        CommandResult::Check(result) => render_check(output, result),
        CommandResult::PatchList(result) => render_patch_list(output, result),
        CommandResult::PatchShow(result) => render_patch_show(output, result),
        CommandResult::PatchCreate(result) => fields(
            output,
            [("active patch", result.active_patch.name().to_string())],
        ),
        CommandResult::PatchSelect(result) => render_patch_select(output, result),
        CommandResult::PatchEdit(result) => render_patch_edit(output, result),
        CommandResult::PatchRefresh(result) => render_patch_refresh(output, result),
        CommandResult::PatchFinish(result) => fields(
            output,
            [("patch", result.patch.clone()), ("check", "passed".into())],
        ),
        CommandResult::Rebase(result) => render_rebase(output, result),
        CommandResult::Publish(result) => fields(
            output,
            [
                ("branch", result.branch.clone()),
                ("head", result.head.clone()),
                ("recovery", result.recovery_tag.clone()),
                ("lease", result.expected_lease.clone()),
            ],
        ),
        CommandResult::OperationStatus(result) => render_operation(output, result),
        CommandResult::OperationContinue(result) => fields(
            output,
            [(
                "operation",
                result
                    .operation
                    .as_ref()
                    .map_or_else(|| "completed".into(), |value| value.id.clone()),
            )],
        ),
        CommandResult::OperationAbort(result) => fields(
            output,
            [
                ("operation", result.operation_id.clone()),
                ("restored tip", result.restored_tip.clone()),
            ],
        ),
        CommandResult::Plan(result) => render_plan(output, result),
        CommandResult::Instructions(_) => {}
    }
}

fn render_init(output: &mut String, result: &crate::protocol::InitResult) {
    fields(
        output,
        [
            ("created", result.created.to_string()),
            ("hydrated", result.hydrated.to_string()),
            ("manifest", result.manifest.clone()),
            ("base", result.base_target.commit.clone()),
            ("bookkeeping", result.bookkeeping_commit.clone()),
        ],
    );
}

fn render_patch_select(output: &mut String, result: &crate::protocol::PatchSelectResult) {
    fields(
        output,
        [
            (
                "previous",
                result
                    .previous
                    .as_ref()
                    .map_or_else(|| "none".into(), |value| value.name().to_string()),
            ),
            ("active patch", result.active_patch.name().to_string()),
        ],
    );
}

fn render_patch_edit(output: &mut String, result: &crate::protocol::PatchEditResult) {
    fields(
        output,
        [
            ("patch", result.patch.name.clone()),
            ("old commit", result.old_commit.clone()),
            ("new commit", result.new_commit.clone()),
            ("generated", display_list(&result.generated_paths)),
        ],
    );
}

fn render_patch_refresh(output: &mut String, result: &crate::protocol::PatchRefreshResult) {
    fields(
        output,
        [
            ("patch", result.patch.clone()),
            ("captured", display_list(&result.captured_paths)),
            (
                "old commit",
                result.old_commit.clone().unwrap_or_else(|| "draft".into()),
            ),
            ("new commit", result.new_commit.clone()),
            ("generated", display_list(&result.generated_paths)),
        ],
    );
}

fn render_rebase(output: &mut String, result: &crate::protocol::RebaseResult) {
    fields(
        output,
        [
            ("target", result.selected_target.clone()),
            ("old tip", result.old_tip.clone()),
            ("new tip", result.new_tip.clone()),
            ("recovery", result.recovery_tag.clone()),
            ("report", result.report_path.clone()),
            ("dropped", display_list(&result.dropped_patches)),
        ],
    );
}

fn render_plan(output: &mut String, result: &crate::protocol::MutationPlan) {
    fields(
        output,
        [
            ("command", result.command.clone()),
            ("reads", display_list(&result.reads)),
            ("writes", display_list(&result.writes)),
            ("hooks", display_list(&result.hooks)),
            ("ref updates", display_list(&result.ref_updates)),
            ("paths", display_list(&result.paths)),
            (
                "confirmation",
                if result.requires_confirmation {
                    "required".into()
                } else {
                    "not required".into()
                },
            ),
        ],
    );
}

fn render_status(output: &mut String, result: &StatusResult) {
    fields(
        output,
        [
            ("repository", result.repository.clone()),
            ("manifest", result.manifest.clone()),
            (
                "branch",
                format!(
                    "{} (declared {})",
                    result.current_branch.as_deref().unwrap_or("detached"),
                    result.declared_branch
                ),
            ),
            (
                "downstream",
                format!(
                    "{} {}",
                    result.downstream_remote,
                    result.downstream_sha.as_deref().unwrap_or("unavailable")
                ),
            ),
            (
                "base",
                format!("{} {}", result.selected_target, result.stack_base),
            ),
            (
                "active",
                result
                    .active_patch
                    .as_ref()
                    .map_or_else(|| "none".into(), |value| value.name().to_string()),
            ),
            ("staged", display_list(&result.staged)),
            ("unstaged", display_list(&result.unstaged)),
            ("untracked", display_list(&result.untracked)),
            (
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
        ],
    );
    render_patch_rows(output, &result.patches);
    render_operation(
        output,
        &OperationStatusResult {
            operation: result.operation.clone(),
        },
    );
}

fn render_check(output: &mut String, result: &CheckResult) {
    success_line(output, "check passed");
    fields(
        output,
        [
            ("scope", format!("{:?}", result.scope).to_lowercase()),
            (
                "patch",
                result.patch.clone().unwrap_or_else(|| "repository".into()),
            ),
            ("checked paths", display_list(&result.checked_paths)),
            (
                "canonical",
                result
                    .canonical_base
                    .clone()
                    .unwrap_or_else(|| "n/a".into()),
            ),
            (
                "stack base",
                result.stack_base.clone().unwrap_or_else(|| "n/a".into()),
            ),
            (
                "patch count",
                result
                    .patch_count
                    .map_or_else(|| "n/a".into(), |value| value.to_string()),
            ),
            (
                "source tree",
                result.source_tree.clone().unwrap_or_else(|| "n/a".into()),
            ),
        ],
    );
}

fn render_patch_list(output: &mut String, result: &PatchListResult) {
    render_patch_rows(output, &result.patches);
}

fn render_patch_rows(output: &mut String, patches: &[crate::protocol::PatchSummary]) {
    if patches.is_empty() {
        muted_line(output, "no patches");
        return;
    }
    output.push_str(&table(
        &["State", "Patch", "Kind", "Commit", "Active"],
        patches.iter().map(|patch| {
            vec![
                patch.state.clone(),
                patch.name.clone(),
                format!("{:?}", patch.kind).to_lowercase(),
                patch.commit.clone().unwrap_or_else(|| "draft".into()),
                if patch.active {
                    "yes".into()
                } else {
                    String::new()
                },
            ]
        }),
    ));
}

fn render_patch_show(output: &mut String, result: &PatchShowResult) {
    fields(
        output,
        [
            ("patch", result.patch.name.clone()),
            ("kind", format!("{:?}", result.patch.kind).to_lowercase()),
            ("purpose", result.patch.purpose.clone()),
            ("upstream status", result.patch.upstream_status.clone()),
            ("drop when", result.patch.drop_when.clone()),
            ("scope", display_list(&result.patch.scope)),
            (
                "commit",
                result.commit.clone().unwrap_or_else(|| "draft".into()),
            ),
            ("changed paths", display_list(&result.changed_paths)),
            (
                "export",
                result.export.clone().unwrap_or_else(|| "none".into()),
            ),
            ("active", result.active.to_string()),
        ],
    );
}

fn render_operation(output: &mut String, result: &OperationStatusResult) {
    section(output, "operation");
    match &result.operation {
        Some(operation) => fields(
            output,
            [
                ("id", operation.id.clone()),
                ("kind", format!("{:?}", operation.kind).to_lowercase()),
                ("phase", operation.phase.clone()),
                ("recovery", operation.recovery.tag.clone()),
                ("next", display_list(&operation.next_actions)),
            ],
        ),
        None => muted_line(output, "none"),
    }
}

fn render_error(command: &str, error: &ApiError) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}forkctl · {command} · error{}",
        FAILURE.render(),
        FAILURE.render_reset()
    );
    let _ = writeln!(
        output,
        "{}{}{}",
        FAILURE.render(),
        error.message,
        FAILURE.render_reset()
    );
    let _ = writeln!(
        output,
        "{}code · {}{}",
        MUTED.render(),
        error.code,
        MUTED.render_reset()
    );
    for cause in &error.causes {
        let _ = writeln!(
            output,
            "{}caused by · {}{}",
            MUTED.render(),
            cause,
            MUTED.render_reset()
        );
    }
    if let Some(command) = &error.suggested_command {
        let _ = writeln!(
            output,
            "{}next · {}{}",
            HEADING.render(),
            command,
            HEADING.render_reset()
        );
    }
    output
}

fn heading(output: &mut String, command: &str) {
    let _ = writeln!(
        output,
        "{}forkctl · {command}{}",
        HEADING.render(),
        HEADING.render_reset()
    );
}

fn section(output: &mut String, title: &str) {
    let _ = writeln!(
        output,
        "\n{}{title}{}",
        HEADING.render(),
        HEADING.render_reset()
    );
}

fn success_line(output: &mut String, message: &str) {
    let _ = writeln!(
        output,
        "{}ok{} · {message}",
        SUCCESS.render(),
        SUCCESS.render_reset()
    );
}

fn muted_line(output: &mut String, message: &str) {
    let _ = writeln!(
        output,
        "{}{}{}",
        MUTED.render(),
        message,
        MUTED.render_reset()
    );
}

fn fields<const N: usize>(output: &mut String, values: [(&str, String); N]) {
    output.push_str(&table(
        &["Field", "Value"],
        values
            .into_iter()
            .map(|(field, value)| vec![field.to_string(), value]),
    ));
}

fn table(headers: &[&str], rows: impl IntoIterator<Item = Vec<String>>) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers);
    for row in rows {
        table.add_row(row);
    }
    format!("{table}\n")
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values.join(", ")
    }
}
