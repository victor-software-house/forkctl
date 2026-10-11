//! Every kind of change a command makes to the tracked manifest, applied once
//! to the typed [`Manifest`] and once to the YAML text, so the file keeps
//! every byte the command did not change.

use crate::error::{AppError, AppResult as Result, DomainError};
use crate::manifest::{
    BaseTarget, Contracts, DisabledPatch, HistoryEvent, Manifest, Patch, PublishMode,
};
use crate::manifest_codec::ManifestFormat;
use serde::Serialize;
use std::path::Path;
use yamled::{Document, Path as YamlPath, Position};

#[derive(Debug, Clone)]
pub(crate) enum ManifestEdit {
    SetContracts(Contracts),
    SetPublishMode(PublishMode),
    SetBase {
        target: BaseTarget,
        stack: String,
        canonical: String,
    },
    InsertPatch {
        index: usize,
        patch: Patch,
    },
    ReplacePatch(Patch),
    RemovePatch {
        name: String,
    },
    PushDisabled(DisabledPatch),
    RemoveDisabled {
        name: String,
    },
    PushHistory(HistoryEvent),
}

impl Manifest {
    pub(crate) fn apply(&mut self, edit: &ManifestEdit) {
        match edit {
            ManifestEdit::SetContracts(contracts) => self.contracts = contracts.clone(),
            ManifestEdit::SetPublishMode(mode) => self.downstream.publish = *mode,
            ManifestEdit::SetBase {
                target,
                stack,
                canonical,
            } => {
                self.base.target = target.clone();
                self.base.stack.clone_from(stack);
                self.base.canonical.clone_from(canonical);
            }
            ManifestEdit::InsertPatch { index, patch } => {
                self.patches.insert(*index, patch.clone());
            }
            ManifestEdit::ReplacePatch(patch) => {
                if let Some(slot) = self
                    .patches
                    .iter_mut()
                    .find(|candidate| candidate.name == patch.name)
                {
                    *slot = patch.clone();
                }
            }
            ManifestEdit::RemovePatch { name } => {
                self.patches.retain(|candidate| &candidate.name != name);
            }
            ManifestEdit::PushDisabled(record) => self.disabled_patches.push(record.clone()),
            ManifestEdit::RemoveDisabled { name } => {
                self.disabled_patches
                    .retain(|candidate| &candidate.patch.name != name);
            }
            ManifestEdit::PushHistory(event) => self.history.push(event.clone()),
        }
    }

    pub(crate) fn edited(&self, edits: &[ManifestEdit]) -> Self {
        let mut manifest = self.clone();
        for edit in edits {
            manifest.apply(edit);
        }
        manifest
    }
}

/// Apply `edits` to the YAML `source`, and prove the result reads back as
/// `expected`. A failure leaves nothing written: the caller writes only the
/// returned text.
pub(crate) fn edit_yaml(
    source: &str,
    path: &Path,
    edits: &[ManifestEdit],
    expected: &Manifest,
) -> Result<String> {
    let mut before = ManifestFormat::Yaml.parse(source.as_bytes(), path)?;
    let mut document = Document::parse(source).map_err(|error| refused(path, "parse", &error))?;
    for edit in edits {
        apply_yaml(&mut document, &before, edit)
            .map_err(|error| refused(path, label(edit), &error))?;
        before.apply(edit);
    }
    let text = document.into_string();
    let written = ManifestFormat::Yaml.parse(text.as_bytes(), path)?;
    if serde_json::to_value(&written).ok() != serde_json::to_value(expected).ok() {
        return Err(DomainError::manifest_invalid(format!(
            "cannot edit {} in place: the edited text no longer reads as the change forkctl made; \
             an anchor or alias likely shares an edited value, so expand it by hand",
            path.display()
        ))
        .into());
    }
    Ok(text)
}

fn apply_yaml(
    document: &mut Document,
    before: &Manifest,
    edit: &ManifestEdit,
) -> std::result::Result<(), yamled::Error> {
    let root = YamlPath::root();
    match edit {
        ManifestEdit::SetContracts(contracts) => set_key(document, &root, "contracts", contracts),
        ManifestEdit::SetPublishMode(mode) => {
            let downstream = root.key("downstream");
            let publish = downstream.clone().key("publish");
            match (document.node(&publish).is_some(), mode.is_rewrite()) {
                (true, true) => document.remove(&publish),
                (true, false) => document.replace(&publish, mode),
                (false, true) => Ok(()),
                (false, false) => document.insert(&downstream, "publish", mode, Position::End),
            }
        }
        ManifestEdit::SetBase {
            target,
            stack,
            canonical,
        } => {
            let base = root.key("base");
            document.replace(&base.clone().key("target"), target)?;
            document.replace(&base.clone().key("stack"), stack)?;
            document.replace(&base.key("canonical"), canonical)
        }
        ManifestEdit::InsertPatch { index, patch } => {
            document.insert_item(&root.key("patches"), *index, patch)
        }
        ManifestEdit::ReplacePatch(patch) => {
            let index = patch_index(before, &patch.name, &root)?;
            document.replace(&root.key("patches").index(index), patch)
        }
        ManifestEdit::RemovePatch { name } => {
            let index = patch_index(before, name, &root)?;
            remove_item(document, &root.key("patches"), index, before.patches.len())
        }
        ManifestEdit::PushDisabled(record) => {
            push_item(document, &root, "disabled_patches", record)
        }
        ManifestEdit::RemoveDisabled { name } => {
            let sequence = root.key("disabled_patches");
            let index = before
                .disabled_patches
                .iter()
                .position(|candidate| &candidate.patch.name == name)
                .ok_or_else(|| yamled::Error::NoNode {
                    path: sequence.clone(),
                })?;
            remove_item(document, &sequence, index, before.disabled_patches.len())
        }
        ManifestEdit::PushHistory(event) => push_item(document, &root, "history", event),
    }
}

/// Replace a top-level key, or add it after the last one.
fn set_key<T: Serialize>(
    document: &mut Document,
    root: &YamlPath,
    key: &str,
    value: &T,
) -> std::result::Result<(), yamled::Error> {
    let path = root.clone().key(key);
    if document.node(&path).is_some() {
        document.replace(&path, value)
    } else {
        document.insert(root, key, value, Position::End)
    }
}

/// Append to a top-level sequence, adding the key when it is absent.
fn push_item<T: Serialize>(
    document: &mut Document,
    root: &YamlPath,
    key: &str,
    value: &T,
) -> std::result::Result<(), yamled::Error> {
    let path = root.clone().key(key);
    if document.node(&path).is_some() {
        document.push(&path, value)
    } else {
        document.insert(root, key, &[value], Position::End)
    }
}

/// Remove one item; the last one leaves `[]` rather than an empty value.
fn remove_item(
    document: &mut Document,
    sequence: &YamlPath,
    index: usize,
    len: usize,
) -> std::result::Result<(), yamled::Error> {
    if len == 1 {
        document.replace(sequence, &[] as &[()])
    } else {
        document.remove(&sequence.clone().index(index))
    }
}

fn patch_index(
    manifest: &Manifest,
    name: &str,
    root: &YamlPath,
) -> std::result::Result<usize, yamled::Error> {
    manifest
        .patches
        .iter()
        .position(|candidate| candidate.name == name)
        .ok_or_else(|| yamled::Error::NoNode {
            path: root.clone().key("patches"),
        })
}

fn label(edit: &ManifestEdit) -> &'static str {
    match edit {
        ManifestEdit::SetContracts(_) => "set contracts",
        ManifestEdit::SetPublishMode(_) => "set downstream.publish",
        ManifestEdit::SetBase { .. } => "set base",
        ManifestEdit::InsertPatch { .. } => "insert a patch",
        ManifestEdit::ReplacePatch(_) => "replace a patch",
        ManifestEdit::RemovePatch { .. } => "remove a patch",
        ManifestEdit::PushDisabled(_) => "add a disabled patch",
        ManifestEdit::RemoveDisabled { .. } => "remove a disabled patch",
        ManifestEdit::PushHistory(_) => "add a history event",
    }
}

fn refused(path: &Path, what: &str, error: &yamled::Error) -> AppError {
    DomainError::manifest_invalid(format!(
        "cannot {what} in {} in place: {error}; forkctl edits only the nodes it changes, \
         so expand any anchor, alias, or flow collection there by hand",
        path.display()
    ))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{RecoveryEvidence, TargetKind};

    const EXAMPLE: &str = include_str!("../examples/fork.yaml");

    fn commented() -> String {
        EXAMPLE
            .replacen(
                "downstream:\n",
                "# owned by the platform team\ndownstream:\n",
                1,
            )
            .replacen("project.git\n", "project.git # mirror\n", 1)
    }

    fn parse(source: &str) -> Manifest {
        ManifestFormat::Yaml
            .parse(source.as_bytes(), Path::new("fork.yaml"))
            .unwrap()
    }

    fn edit(source: &str, edits: &[ManifestEdit]) -> String {
        let expected = parse(source).edited(edits);
        edit_yaml(source, Path::new("fork.yaml"), edits, &expected).unwrap()
    }

    fn assert_comments_kept(text: &str) {
        assert!(text.contains("# owned by the platform team\ndownstream:\n"));
        assert!(text.contains("project.git # mirror\n"));
    }

    fn recovery() -> RecoveryEvidence {
        RecoveryEvidence {
            tag: "forkctl/recovery/1".into(),
            tag_object: "1".repeat(40),
            old_base: "2".repeat(40),
            old_tip: "3".repeat(40),
        }
    }

    #[test]
    fn publish_mode_edits_one_line_and_rewrite_restores_the_bytes() {
        let source = commented();
        let append = edit(
            &source,
            &[ManifestEdit::SetPublishMode(PublishMode::Append)],
        );
        assert_eq!(
            append,
            source.replacen(
                "recovery_tag_prefix: forkctl/recovery\n",
                "recovery_tag_prefix: forkctl/recovery\n  publish: append\n",
                1
            )
        );
        let rewrite = edit(
            &append,
            &[ManifestEdit::SetPublishMode(PublishMode::Rewrite)],
        );
        assert_eq!(rewrite, source);
    }

    #[test]
    fn base_edits_change_only_the_base_values() {
        let source = commented();
        let target = BaseTarget {
            kind: TargetKind::Branch,
            selector: "refs/heads/main".into(),
            commit: "a".repeat(40),
            tag_object: None,
        };
        let text = edit(
            &source,
            &[ManifestEdit::SetBase {
                target,
                stack: "b".repeat(40),
                canonical: "c".repeat(40),
            }],
        );
        assert_comments_kept(&text);
        let zeros = format!("'{}'", "0".repeat(40));
        let lines = |text: &str| text.lines().map(str::to_owned).collect::<Vec<_>>();
        let changed = lines(&source)
            .iter()
            .zip(lines(&text))
            .filter(|(old, new)| *old != new)
            .count();
        assert_eq!(changed, 3, "{text}");
        assert!(!text.contains(&zeros), "{text}");
    }

    #[test]
    fn removing_and_reinserting_a_patch_restores_the_bytes() {
        let source = commented();
        let patch = parse(&source).patches[0].clone();
        let removed = edit(
            &source,
            &[ManifestEdit::RemovePatch {
                name: patch.name.clone(),
            }],
        );
        assert_comments_kept(&removed);
        assert!(!removed.contains("downstream-change"));
        let restored = edit(&removed, &[ManifestEdit::InsertPatch { index: 0, patch }]);
        assert_eq!(restored, source);
    }

    #[test]
    fn replacing_a_patch_keeps_its_neighbours() {
        let source = commented();
        let mut patch = parse(&source).patches[0].clone();
        patch.purpose = "A new purpose.".into();
        let text = edit(&source, &[ManifestEdit::ReplacePatch(patch)]);
        assert_eq!(
            text,
            source.replacen(
                "Describe why this downstream change exists.",
                "A new purpose.",
                1
            )
        );
    }

    #[test]
    fn disabling_adds_the_absent_key_and_enabling_leaves_an_empty_list() {
        let source = commented();
        let manifest = parse(&source);
        let record = DisabledPatch {
            patch: manifest.patches[0].clone(),
            commit: "d".repeat(40),
            position: 0,
            reason: "obsolete".into(),
            recovery: recovery(),
        };
        let disabled = edit(
            &source,
            &[
                ManifestEdit::RemovePatch {
                    name: record.patch.name.clone(),
                },
                ManifestEdit::PushDisabled(record.clone()),
            ],
        );
        assert_comments_kept(&disabled);
        assert_eq!(parse(&disabled).disabled_patches.len(), 1);
        let enabled = edit(
            &disabled,
            &[
                ManifestEdit::RemoveDisabled {
                    name: record.patch.name.clone(),
                },
                ManifestEdit::InsertPatch {
                    index: 0,
                    patch: record.patch.clone(),
                },
                ManifestEdit::PushHistory(HistoryEvent::PatchEnabled {
                    record,
                    recovery: recovery(),
                }),
            ],
        );
        assert_comments_kept(&enabled);
        let manifest = parse(&enabled);
        assert!(manifest.disabled_patches.is_empty());
        assert_eq!(manifest.history.len(), 1);
        assert_eq!(manifest.patches.len(), 2);
    }

    #[test]
    fn contracts_edit_replaces_only_the_contracts() {
        let source = commented();
        let text = edit(&source, &[ManifestEdit::SetContracts(Contracts::default())]);
        let kept = source.split("contracts:").next().unwrap();
        assert!(text.starts_with(kept), "{text}");
        assert!(parse(&text).contracts.required_text.is_empty());
    }

    #[test]
    fn an_edit_through_an_alias_is_refused() {
        let source = commented()
            .replacen("    scope:\n      - src/**\n", "    scope: &shared\n      - src/**\n", 1)
            .replacen(
                "    upstream_status: 'inappropriate: downstream-only tooling'\n",
                "    upstream_status: 'inappropriate: downstream-only tooling'\n    checks:\n      - name: lint\n        run: lint {files}\n        glob: *shared\n",
                1,
            );
        let manifest = parse(&source);
        let mut patch = manifest.patches[0].clone();
        patch.scope = vec!["lib/**".into()];
        let edits = [ManifestEdit::ReplacePatch(patch)];
        let expected = manifest.edited(&edits);
        let error = edit_yaml(&source, Path::new("fork.yaml"), &edits, &expected).unwrap_err();
        assert!(error.to_string().contains("cannot"), "{error}");
    }
}
