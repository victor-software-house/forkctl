use crate::manifest::{CheckStage, HistoryEvent, Manifest, PatchKind};
use anyhow::{Context, Result};
use askama::Template;

#[derive(Template)]
#[template(path = "patches.md", escape = "none")]
struct LedgerTemplate<'a> {
    target_selector: String,
    base_sha: &'a str,
    patches: Vec<PatchRow>,
    disabled: Vec<DisabledRow>,
    checks: Vec<CheckRow>,
    history: Vec<HistoryRow>,
}

struct PatchRow {
    name: String,
    kind: &'static str,
    purpose: String,
    upstream_status: String,
    drop_when: String,
}

struct CheckRow {
    patch: String,
    name: String,
    stage: &'static str,
    glob: String,
    run: String,
}

struct HistoryRow {
    kind: &'static str,
    patch: String,
    commit: String,
    target: String,
    target_commit: String,
    purpose: String,
}

struct DisabledRow {
    name: String,
    commit: String,
    reason: String,
    position: usize,
}

pub fn render(manifest: &Manifest) -> Result<String> {
    let history = history_rows(manifest);
    let checks = manifest
        .patches
        .iter()
        .flat_map(|patch| {
            patch.checks.iter().map(|check| CheckRow {
                patch: escape(&patch.name),
                name: escape(&check.name),
                stage: match check.at {
                    CheckStage::Stack => "stack",
                    CheckStage::Patch => "patch",
                },
                glob: escape(
                    &patch
                        .check_globs(check)
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                run: escape(&check.run),
            })
        })
        .collect();
    LedgerTemplate {
        target_selector: escape(&manifest.base.target.selector),
        base_sha: &manifest.base.stack,
        patches: manifest
            .patches
            .iter()
            .map(|patch| PatchRow {
                name: escape(&patch.name),
                kind: patch_kind(patch.kind),
                purpose: escape(&patch.purpose),
                upstream_status: escape(&patch.upstream_status),
                drop_when: escape(&patch.drop_when),
            })
            .collect(),
        disabled: manifest
            .disabled_patches
            .iter()
            .map(|record| DisabledRow {
                name: escape(&record.patch.name),
                commit: record.commit.clone(),
                reason: escape(&record.reason),
                position: record.position,
            })
            .collect(),
        checks,
        history,
    }
    .render()
    .context("render PATCHES.md")
}

fn history_rows(manifest: &Manifest) -> Vec<HistoryRow> {
    manifest
        .history
        .iter()
        .flat_map(|event| match event {
            HistoryEvent::Rebase {
                target,
                dropped,
                path_changes,
                ..
            } => dropped
                .iter()
                .map(|item| HistoryRow {
                    kind: "upstream merged",
                    patch: escape(&item.patch.name),
                    commit: item.commit.clone(),
                    target: escape(&target.selector),
                    target_commit: target.commit.clone(),
                    purpose: escape(&item.patch.purpose),
                })
                .chain(path_changes.iter().map(|item| HistoryRow {
                    kind: "replay path change",
                    patch: escape(&item.patch),
                    commit: item.commit.clone(),
                    target: escape(&target.selector),
                    target_commit: target.commit.clone(),
                    purpose: format!(
                        "Lost paths after replay: {}",
                        escape(
                            &item
                                .lost_paths
                                .iter()
                                .map(String::as_str)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    ),
                }))
                .collect::<Vec<_>>(),
            HistoryEvent::PatchRemoved { record } => vec![HistoryRow {
                kind: "removed",
                patch: escape(&record.patch.name),
                commit: record.commit.clone(),
                target: "—".into(),
                target_commit: "—".into(),
                purpose: escape(&record.reason),
            }],
            HistoryEvent::PatchEnabled { record, .. } => vec![HistoryRow {
                kind: "re-enabled",
                patch: escape(&record.patch.name),
                commit: record.commit.clone(),
                target: "—".into(),
                target_commit: "—".into(),
                purpose: escape(&record.reason),
            }],
        })
        .collect()
}

fn patch_kind(kind: PatchKind) -> &'static str {
    match kind {
        PatchKind::Source => "source",
        PatchKind::Tooling => "tooling",
    }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
}
