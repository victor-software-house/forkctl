use crate::manifest::{HistoryEvent, Manifest, PatchKind};
use anyhow::{Context, Result};
use askama::Template;

#[derive(Template)]
#[template(path = "patches.md", escape = "none")]
struct LedgerTemplate<'a> {
    target_selector: String,
    base_sha: &'a str,
    patches: Vec<PatchRow>,
    history: Vec<HistoryRow>,
}

struct PatchRow {
    name: String,
    kind: &'static str,
    purpose: String,
    upstream_status: String,
    drop_when: String,
}

struct HistoryRow {
    kind: &'static str,
    patch: String,
    commit: String,
    target: String,
    target_commit: String,
    purpose: String,
}

pub fn render(manifest: &Manifest) -> Result<String> {
    let history = manifest
        .history
        .iter()
        .flat_map(|event| match event {
            HistoryEvent::Rebase {
                target, dropped, ..
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
                .collect::<Vec<_>>(),
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
        history,
    }
    .render()
    .context("render PATCHES.md")
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
