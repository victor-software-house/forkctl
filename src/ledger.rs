use anyhow::{Context, Result};
use askama::Template;

use crate::manifest::{CheckStage, HistoryEvent, Manifest, PatchLayer};

#[derive(Template)]
#[template(path = "patches.md", escape = "none")]
struct LedgerTemplate<'a> {
    target_selector: String,
    base_sha: &'a str,
    patches: Vec<PatchRow>,
    disabled: Vec<DisabledRow>,
    checks: Vec<CheckRow>,
    trailers: Vec<TrailerRow>,
    history: Vec<HistoryRow>,
}

struct PatchRow {
    name: String,
    layer: &'static str,
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

struct TrailerRow {
    patch: String,
    key: String,
    value: String,
}

struct HistoryRow {
    event: &'static str,
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

pub(crate) fn render(manifest: &Manifest) -> Result<String> {
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
                layer: patch_layer(patch.layer),
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
        trailers: manifest
            .patches
            .iter()
            .flat_map(|patch| {
                patch.trailers.iter().map(|trailer| TrailerRow {
                    patch: escape(&patch.name),
                    key: escape(&trailer.key),
                    value: escape(&trailer.value),
                })
            })
            .collect(),
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
                    event: "upstream merged",
                    patch: escape(&item.patch.name),
                    commit: item.commit.clone(),
                    target: escape(&target.selector),
                    target_commit: target.commit.clone(),
                    purpose: escape(&item.patch.purpose),
                })
                .chain(path_changes.iter().map(|item| HistoryRow {
                    event: "replay path change",
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
                event: "removed",
                patch: escape(&record.patch.name),
                commit: record.commit.clone(),
                target: "—".into(),
                target_commit: "—".into(),
                purpose: escape(&record.reason),
            }],
            HistoryEvent::PatchEnabled { record, .. } => vec![HistoryRow {
                event: "re-enabled",
                patch: escape(&record.patch.name),
                commit: record.commit.clone(),
                target: "—".into(),
                target_commit: "—".into(),
                purpose: escape(&record.reason),
            }],
        })
        .collect()
}

pub(crate) fn patch_layer(layer: PatchLayer) -> &'static str {
    match layer {
        PatchLayer::Source => "source",
        PatchLayer::Tooling => "tooling",
    }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::render;
    use crate::manifest::Trailer;
    use crate::manifest_codec::ManifestFormat;

    const EXAMPLE: &str = include_str!("../examples/fork.yaml");

    #[test]
    fn ledger_lists_trailers_only_when_declared() {
        let mut manifest = ManifestFormat::Yaml
            .parse(EXAMPLE.as_bytes(), Path::new("fork.yaml"))
            .unwrap();
        let plain = render(&manifest).unwrap();
        let lines = plain.lines().collect::<Vec<_>>();
        let history = lines.iter().position(|line| *line == "## History").unwrap();
        assert_eq!(
            lines[history - 2..history],
            ["| None | — | — | — | — |", ""]
        );
        assert!(!plain.contains("## Trailers"), "{plain}");

        manifest.patches[0].trailers = ["Ada <ada@example.com>", "Grace <grace@example.com>"]
            .map(|value| Trailer {
                key: "Co-authored-by".into(),
                value: value.into(),
            })
            .to_vec();
        let declared = render(&manifest).unwrap();
        let section = declared
            .lines()
            .skip_while(|line| *line != "## Trailers")
            .take_while(|line| *line != "## History")
            .collect::<Vec<_>>();
        assert_eq!(
            section,
            [
                "## Trailers",
                "",
                "| Patch | Trailer | Value |",
                "|:--|:--|:--|",
                "| `downstream-change` | `Co-authored-by` | `Ada <ada@example.com>` |",
                "| `downstream-change` | `Co-authored-by` | `Grace <grace@example.com>` |",
                "",
            ]
        );
    }
}
