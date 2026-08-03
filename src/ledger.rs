use crate::manifest::{Manifest, PatchKind};
use anyhow::{Context, Result};
use askama::Template;

#[derive(Template)]
#[template(path = "patches.md", escape = "none")]
struct LedgerTemplate<'a> {
    base_label: String,
    base_sha: &'a str,
    patches: Vec<PatchRow>,
}

struct PatchRow {
    name: String,
    kind: &'static str,
    purpose: String,
    upstream_status: String,
    drop_when: String,
}

pub fn render(manifest: &Manifest) -> Result<String> {
    LedgerTemplate {
        base_label: escape(&manifest.base.label),
        base_sha: &manifest.base.stack,
        patches: manifest
            .patches
            .iter()
            .map(|patch| PatchRow {
                name: escape(&patch.name),
                kind: match patch.kind {
                    PatchKind::Source => "source",
                    PatchKind::Tooling => "tooling",
                },
                purpose: escape(&patch.purpose),
                upstream_status: escape(&patch.upstream_status),
                drop_when: escape(&patch.drop_when),
            })
            .collect(),
    }
    .render()
    .context("render PATCHES.md")
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
    use crate::manifest::{Allow, Base, Downstream, Patch, Upstream};

    #[test]
    fn renders_stable_escaped_table() {
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
                label: "refs/tags/v1".into(),
                canonical: "0".repeat(40),
                stack: "0".repeat(40),
            },
            ledger: "PATCHES.md".into(),
            bookkeeping_patch: "fork-tooling".into(),
            patches: vec![Patch {
                name: "fork-tooling".into(),
                kind: PatchKind::Tooling,
                purpose: "Own A | B.".into(),
                upstream_status: "downstream-only".into(),
                drop_when: "The fork is retired.".into(),
                paths: vec!["fork.json".into(), "PATCHES.md".into()],
                export: None,
            }],
            allow: Allow::default(),
            required: Vec::new(),
        };
        let first = render(&manifest).unwrap();
        assert_eq!(first, render(&manifest).unwrap());
        assert!(first.contains("Own A \\| B."));
        assert!(first.ends_with('\n'));
        assert_eq!(first.matches("| 1 | `fork-tooling`").count(), 1);
    }
}
