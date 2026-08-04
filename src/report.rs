use anyhow::{Context, Result};
use askama::Template;

pub struct RebaseReport {
    pub target: String,
    pub old_base: String,
    pub old_tip: String,
    pub new_base: String,
    pub new_tip: String,
    pub recovery_tag: String,
    pub exports: Vec<ExportEvidence>,
    pub range_diff: String,
}

pub struct ExportEvidence {
    pub path: String,
    pub hash: String,
}

#[derive(Template)]
#[template(path = "rebase-report.md", escape = "none")]
struct RebaseReportTemplate {
    target: String,
    old_base: String,
    old_tip: String,
    new_base: String,
    new_tip: String,
    recovery_tag: String,
    exports: Vec<ExportRow>,
    range_diff: String,
}

struct ExportRow {
    path: String,
    hash: String,
}

pub fn render(report: RebaseReport) -> Result<String> {
    let mut output = RebaseReportTemplate {
        target: escape_inline(&report.target),
        old_base: escape_inline(&report.old_base),
        old_tip: escape_inline(&report.old_tip),
        new_base: escape_inline(&report.new_base),
        new_tip: escape_inline(&report.new_tip),
        recovery_tag: escape_inline(&report.recovery_tag),
        exports: report
            .exports
            .into_iter()
            .map(|export| ExportRow {
                path: escape_inline(&export.path),
                hash: escape_inline(&export.hash),
            })
            .collect(),
        range_diff: report.range_diff,
    }
    .render()
    .context("render rebase report")?;
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn escape_inline(value: &str) -> String {
    value.replace('\\', "\\\\").replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_no_exports_and_range_diff() {
        let output = render(RebaseReport {
            target: "refs/tags/v1".into(),
            old_base: "a".repeat(40),
            old_tip: "b".repeat(40),
            new_base: "c".repeat(40),
            new_tip: "d".repeat(40),
            recovery_tag: "vsh/pre-sync".into(),
            exports: Vec::new(),
            range_diff: "1: old\n2: new".into(),
        })
        .unwrap();
        assert!(output.contains("- None"));
        assert!(output.contains("1: old\n2: new\n"));
        assert!(output.ends_with('\n'));
    }
}
