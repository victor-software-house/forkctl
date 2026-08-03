use crate::manifest::{Manifest, PatchEventKind, PatchKind};
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
        history: manifest
            .history
            .iter()
            .map(|event| HistoryRow {
                kind: match event.kind {
                    PatchEventKind::UpstreamMerged => "upstream merged",
                },
                patch: escape(&event.patch.name),
                commit: event.commit.clone(),
                target: escape(&event.target.selector),
                target_commit: event.target.commit.clone(),
                purpose: escape(&event.patch.purpose),
            })
            .collect(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{
        Allow, Base, BaseTarget, Downstream, Patch, PatchEvent, TargetKind, Upstream,
    };

    #[test]
    fn renders_stable_escaped_table_and_history() {
        let commit = "0".repeat(40);
        let patch = Patch {
            name: "fork-tooling".into(),
            kind: PatchKind::Tooling,
            purpose: "Own A | B.".into(),
            upstream_status: "downstream-only".into(),
            drop_when: "The fork is retired.".into(),
            paths: vec!["fork.json".into(), "PATCHES.md".into()],
            export: None,
        };
        let manifest = Manifest {
            schema: 1,
            downstream: Downstream {
                remote: "origin".into(),
                branch: "main".into(),
                backup_tag_prefix: "vsh/pre-sync".into(),
            },
            upstream: Upstream {
                remote: "upstream".into(),
                url: "https://example.com/upstream.git".into(),
                fetch_ref: "refs/heads/main".into(),
            },
            base: Base {
                target: BaseTarget {
                    kind: TargetKind::Commit,
                    selector: commit.clone(),
                    commit: commit.clone(),
                    tag_object: None,
                },
                canonical: commit.clone(),
                stack: commit.clone(),
            },
            ledger: "PATCHES.md".into(),
            bookkeeping_patch: "fork-tooling".into(),
            patches: vec![patch.clone()],
            history: vec![PatchEvent {
                kind: PatchEventKind::UpstreamMerged,
                patch,
                commit: "1".repeat(40),
                target: BaseTarget {
                    kind: TargetKind::Commit,
                    selector: commit.clone(),
                    commit,
                    tag_object: None,
                },
            }],
            allow: Allow::default(),
            required: Vec::new(),
        };
        let first = render(&manifest).unwrap();
        assert_eq!(first, render(&manifest).unwrap());
        assert!(first.contains("Own A \\| B."));
        assert!(first.contains("upstream merged"));
        assert!(first.ends_with('\n'));
    }
}
