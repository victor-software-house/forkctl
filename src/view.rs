use crate::protocol::{
    ApiError, ApiResponse, CommandResult, InitResult, NewPhase, Notice, PublishResult,
    RebaseResult, StatusResult, VerificationResult,
};
use anstyle::{AnsiColor, Style as AnsiStyle};
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use std::fmt::Write as _;
use std::io::{self, Write as _};

const HEADING: AnsiStyle = AnsiStyle::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan)));
const SUCCESS: AnsiStyle = AnsiStyle::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)));
const WARNING: AnsiStyle = AnsiStyle::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow)));
const FAILURE: AnsiStyle = AnsiStyle::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)));
const MUTED: AnsiStyle =
    AnsiStyle::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::BrightBlack)));

struct FieldRow {
    field: String,
    value: String,
}

struct PatchRow {
    state: String,
    patch: String,
}

pub fn emit_pretty(response: &ApiResponse) -> io::Result<()> {
    let rendered = render(response);
    match response {
        ApiResponse::Success { .. } => {
            let mut stream = anstream::stdout();
            stream.write_all(rendered.as_bytes())?;
            stream.flush()
        }
        ApiResponse::Error { .. } => {
            let mut stream = anstream::stderr();
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

pub fn render(response: &ApiResponse) -> String {
    match response {
        ApiResponse::Success {
            result, notices, ..
        } => render_success(result, notices),
        ApiResponse::Error { error, .. } => render_error(error),
    }
}

fn render_success(result: &CommandResult, notices: &[Notice]) -> String {
    let mut output = String::new();
    match result {
        CommandResult::Init(result) => render_init(&mut output, result),
        CommandResult::Status(result) => render_status(&mut output, result),
        CommandResult::New(result) => {
            heading(&mut output, "new");
            let phase = match result.phase {
                NewPhase::Created => "created",
                NewPhase::Finished => "finished",
            };
            fields(
                &mut output,
                [
                    ("phase", phase.to_string()),
                    (
                        "patch",
                        result
                            .patch
                            .clone()
                            .unwrap_or_else(|| "pending patch".into()),
                    ),
                    ("paths", display_list(&result.allowed_paths)),
                ],
            );
            if let Some(verification) = &result.verification {
                verification_fields(&mut output, verification);
            }
        }
        CommandResult::Verify(result) => {
            heading(&mut output, "verify");
            success_line(&mut output, "structural verification passed");
            verification_fields(&mut output, result);
        }
        CommandResult::Rebase(result) => render_rebase(&mut output, result),
        CommandResult::Publish(result) => render_publish(&mut output, result),
        CommandResult::Instructions(result) => output.push_str(&result.markdown),
    }
    for notice in notices {
        let _ = writeln!(
            output,
            "{}warning{} · {} · {}",
            WARNING.render(),
            WARNING.render_reset(),
            notice.code,
            notice.message
        );
    }
    output
}

fn render_init(output: &mut String, result: &InitResult) {
    heading(output, "init");
    success_line(
        output,
        if result.already_initialized {
            "stack already initialized"
        } else {
            "stack metadata reconstructed"
        },
    );
    verification_fields(output, &result.verification);
}

fn render_status(output: &mut String, result: &StatusResult) {
    heading(output, "status");
    fields(
        output,
        [
            ("repository", result.repository.clone()),
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
                "upstream",
                format!("{} {}", result.upstream_remote, result.upstream_fetch_ref),
            ),
            (
                "base",
                format!("{} {}", result.selected_target, result.stack_base),
            ),
            ("canonical", result.canonical_base.clone()),
            ("exports", display_list(&result.exports)),
            (
                "worktree",
                if result.dirty.is_empty() {
                    "clean".into()
                } else {
                    format!("dirty · {}", result.dirty.join(", "))
                },
            ),
        ],
    );
    let rows = result
        .applied_patches
        .iter()
        .map(|patch| PatchRow {
            state: "applied".into(),
            patch: patch.clone(),
        })
        .chain(result.unapplied_patches.iter().map(|patch| PatchRow {
            state: "unapplied".into(),
            patch: patch.clone(),
        }))
        .collect::<Vec<_>>();
    if !rows.is_empty() {
        section(output, "patches");
        output.push_str(&table(
            &["State", "Patch"],
            rows.into_iter().map(|row| vec![row.state, row.patch]),
        ));
    }
    section(output, "verification");
    if result.verification.ok {
        success_line(output, "passed");
    } else {
        failure_line(
            output,
            result
                .verification
                .error
                .as_deref()
                .unwrap_or("unknown verification error"),
        );
    }
    section(output, "pending");
    match &result.pending {
        Some(pending) => fields(
            output,
            [
                (
                    "operation",
                    format!("{:?}", pending.operation).to_lowercase(),
                ),
                ("lease", pending.expected_remote_sha.clone()),
                ("recovery", pending.backup_tag.clone()),
                (
                    "report",
                    pending
                        .report
                        .as_ref()
                        .map_or_else(|| "none".into(), |report| report.path.clone()),
                ),
            ],
        ),
        None => muted_line(output, "none"),
    }
}

fn render_rebase(output: &mut String, result: &RebaseResult) {
    heading(output, "rebase");
    success_line(output, "stack rebased and structurally verified");
    fields(
        output,
        [
            ("target", result.selected_target.clone()),
            ("base", result.new_base.clone()),
            ("tip", result.new_tip.clone()),
            ("recovery", result.recovery_tag.clone()),
            ("report", result.report_path.clone()),
            ("report object", result.report_object_id.clone()),
            ("dropped", display_list(&result.dropped_patches)),
        ],
    );
}

fn render_publish(output: &mut String, result: &PublishResult) {
    heading(output, "publish");
    success_line(output, "branch and recovery tag published atomically");
    fields(
        output,
        [
            ("branch", result.branch.clone()),
            ("head", result.head.clone()),
            ("recovery", result.recovery_tag.clone()),
        ],
    );
}

fn verification_fields(output: &mut String, result: &VerificationResult) {
    fields(
        output,
        [
            ("canonical", result.canonical_base.clone()),
            ("stack base", result.stack_base.clone()),
            ("patches", result.patch_count.to_string()),
            ("source tree", result.source_tree.clone()),
        ],
    );
}

fn render_error(error: &ApiError) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}forkctl · error{}",
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

fn failure_line(output: &mut String, message: &str) {
    let _ = writeln!(
        output,
        "{}failed{} · {message}",
        FAILURE.render(),
        FAILURE.render_reset()
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
    let rows = values
        .into_iter()
        .map(|(field, value)| FieldRow {
            field: field.to_string(),
            value,
        })
        .collect::<Vec<_>>();
    output.push_str(&table(
        &["Field", "Value"],
        rows.into_iter().map(|row| vec![row.field, row.value]),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ApiResponse, CommandResult, Outcome, VerificationResult};

    #[test]
    fn verify_view_is_stable_without_color() {
        let response =
            ApiResponse::success(Outcome::new(CommandResult::Verify(VerificationResult {
                canonical_base: "a".repeat(40),
                stack_base: "b".repeat(40),
                patch_count: 2,
                source_tree: "c".repeat(40),
            })));
        let rendered = render(&response);
        let mut plain = Vec::new();
        {
            let mut stream = anstream::AutoStream::new(&mut plain, anstream::ColorChoice::Never);
            stream.write_all(rendered.as_bytes()).unwrap();
        }
        insta::assert_snapshot!(String::from_utf8(plain).unwrap(), @r###"
forkctl · verify
ok · structural verification passed
┌─────────────┬──────────────────────────────────────────┐
│ Field       ┆ Value                                    │
╞═════════════╪══════════════════════════════════════════╡
│ canonical   ┆ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa │
│ stack base  ┆ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb │
│ patches     ┆ 2                                        │
│ source tree ┆ cccccccccccccccccccccccccccccccccccccccc │
└─────────────┴──────────────────────────────────────────┘
"###);
    }
}
