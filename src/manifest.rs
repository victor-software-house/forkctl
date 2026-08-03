use anyhow::{Result, ensure};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub downstream: Downstream,
    pub upstream: Upstream,
    pub base: Base,
    pub ledger: String,
    pub bookkeeping_patch: String,
    pub patches: Vec<Patch>,
    #[serde(default)]
    pub history: Vec<PatchEvent>,
    #[serde(default)]
    pub allow: Allow,
    #[serde(default)]
    pub required: Vec<RequiredText>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Downstream {
    pub remote: String,
    pub branch: String,
    pub backup_tag_prefix: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub remote: String,
    pub url: String,
    pub fetch_ref: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Base {
    pub target: BaseTarget,
    pub canonical: String,
    pub stack: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaseTarget {
    pub kind: TargetKind,
    pub selector: String,
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_object: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Commit,
    Branch,
    Tag,
}

#[derive(
    Debug, Clone, Copy, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize, ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum PatchKind {
    Source,
    Tooling,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    pub name: String,
    pub kind: PatchKind,
    pub purpose: String,
    pub upstream_status: String,
    pub drop_when: String,
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchEvent {
    pub kind: PatchEventKind,
    pub patch: Patch,
    pub commit: String,
    pub target: BaseTarget,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, schemars::JsonSchema, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatchEventKind {
    UpstreamMerged,
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Allow {
    pub base: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredText {
    pub path: String,
    pub contains: String,
}

impl Manifest {
    pub fn validate(&self, repo: &Path, manifest_path: &Path) -> Result<()> {
        self.validate_identity()?;
        self.validate_patches(repo)?;
        self.validate_bookkeeping(repo, manifest_path)?;
        self.validate_history(repo)?;
        self.validate_extra_paths(repo)?;
        Ok(())
    }

    fn validate_identity(&self) -> Result<()> {
        ensure!(
            self.schema == 1,
            "unsupported manifest schema: {}",
            self.schema
        );
        for (label, value) in [
            ("downstream remote", self.downstream.remote.as_str()),
            ("downstream branch", self.downstream.branch.as_str()),
            (
                "backup tag prefix",
                self.downstream.backup_tag_prefix.as_str(),
            ),
            ("upstream remote", self.upstream.remote.as_str()),
            ("upstream URL", self.upstream.url.as_str()),
            ("upstream fetch ref", self.upstream.fetch_ref.as_str()),
            ("ledger", self.ledger.as_str()),
            ("bookkeeping patch", self.bookkeeping_patch.as_str()),
        ] {
            ensure!(!value.trim().is_empty(), "{label} is required");
        }
        ensure!(
            self.upstream.fetch_ref.starts_with("refs/heads/"),
            "upstream fetch_ref must be a full branch ref"
        );
        ensure!(
            !self.downstream.backup_tag_prefix.starts_with("refs/")
                && !self.downstream.backup_tag_prefix.ends_with('/'),
            "backup_tag_prefix must be a tag-name prefix without refs/tags"
        );
        for (label, value) in [
            ("canonical base", self.base.canonical.as_str()),
            ("stack base", self.base.stack.as_str()),
        ] {
            ensure!(is_full_sha(value), "{label} must be a full commit SHA");
        }
        self.base.target.validate()?;
        ensure!(
            self.base.target.commit == self.base.stack,
            "base target commit must equal stack base"
        );
        Ok(())
    }

    fn validate_patches(&self, repo: &Path) -> Result<()> {
        ensure!(!self.patches.is_empty(), "at least one patch is required");
        let mut names = HashSet::new();
        let mut exports = HashSet::new();
        let mut seen_tooling = false;
        for patch in &self.patches {
            validate_patch(repo, patch, &mut names, &mut exports, &mut seen_tooling)?;
        }
        Ok(())
    }

    fn validate_bookkeeping(&self, repo: &Path, manifest_path: &Path) -> Result<()> {
        let manifest_relative = manifest_path.strip_prefix(repo)?.to_string_lossy();
        validate_repo_path(repo, &manifest_relative)?;
        validate_repo_path(repo, &self.ledger)?;
        let bookkeeping = self.patches.last().expect("patches validated as non-empty");
        ensure!(
            bookkeeping.name == self.bookkeeping_patch,
            "bookkeeping patch {} must be final",
            self.bookkeeping_patch
        );
        ensure!(
            bookkeeping.kind == PatchKind::Tooling,
            "bookkeeping patch must be tooling"
        );
        for (label, path) in [
            ("ledger", self.ledger.as_str()),
            ("manifest", manifest_relative.as_ref()),
        ] {
            ensure!(
                bookkeeping
                    .paths
                    .iter()
                    .any(|pattern| path_matches(pattern, path)),
                "bookkeeping patch must own {label} {path}"
            );
        }
        self.validate_exports(bookkeeping, &manifest_relative)
    }

    fn validate_exports(&self, bookkeeping: &Patch, manifest_path: &str) -> Result<()> {
        for patch in &self.patches {
            let Some(export) = patch.export.as_deref() else {
                continue;
            };
            ensure!(
                !contains_glob(export),
                "export path must be a concrete file: {export}"
            );
            ensure!(
                bookkeeping
                    .paths
                    .iter()
                    .any(|pattern| path_matches(pattern, export)),
                "bookkeeping patch must own export {export}"
            );
            ensure!(
                export != manifest_path && export != self.ledger,
                "export collides with manifest or ledger: {export}"
            );
            ensure!(
                !self.required.iter().any(|required| required.path == export),
                "export collides with required contract: {export}"
            );
            for owner in self
                .patches
                .iter()
                .filter(|candidate| candidate.name != self.bookkeeping_patch)
            {
                ensure!(
                    !owner
                        .paths
                        .iter()
                        .any(|pattern| path_matches(pattern, export)),
                    "export {export} overlaps patch {}",
                    owner.name
                );
            }
        }
        Ok(())
    }

    fn validate_history(&self, repo: &Path) -> Result<()> {
        let mut identities = HashSet::new();
        for event in &self.history {
            ensure!(
                is_full_sha(&event.commit),
                "history commit must be a full SHA"
            );
            event.target.validate()?;
            validate_historical_patch(repo, &event.patch)?;
            ensure!(
                identities.insert((event.patch.name.as_str(), event.commit.as_str())),
                "duplicate history event for {} at {}",
                event.patch.name,
                event.commit
            );
        }
        Ok(())
    }

    fn validate_extra_paths(&self, repo: &Path) -> Result<()> {
        for pattern in &self.allow.base {
            validate_pattern(pattern)?;
        }
        for required in &self.required {
            validate_repo_path(repo, &required.path)?;
            ensure!(
                valid_metadata(&required.contains),
                "invalid required text for {}",
                required.path
            );
        }
        Ok(())
    }

    pub fn patch_names(&self) -> Vec<String> {
        self.patches
            .iter()
            .map(|patch| patch.name.clone())
            .collect()
    }

    pub fn exported_patches(&self) -> impl Iterator<Item = &Patch> {
        self.patches.iter().filter(|patch| patch.export.is_some())
    }

    pub fn insertion_index(&self, kind: PatchKind) -> usize {
        match kind {
            PatchKind::Source => self
                .patches
                .iter()
                .position(|patch| patch.kind == PatchKind::Tooling)
                .unwrap_or(self.patches.len()),
            PatchKind::Tooling => self.patches.len() - 1,
        }
    }
}

impl BaseTarget {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            is_full_sha(&self.commit),
            "target commit must be a full SHA"
        );
        match self.kind {
            TargetKind::Commit => {
                ensure!(
                    self.selector == self.commit,
                    "commit selector must equal target commit"
                );
                ensure!(
                    self.tag_object.is_none(),
                    "commit target cannot have tag_object"
                );
            }
            TargetKind::Branch => {
                ensure!(
                    self.selector.starts_with("refs/heads/"),
                    "branch selector must be a full refs/heads ref"
                );
                ensure!(
                    self.tag_object.is_none(),
                    "branch target cannot have tag_object"
                );
            }
            TargetKind::Tag => {
                ensure!(
                    self.selector.starts_with("refs/tags/"),
                    "tag selector must be a full refs/tags ref"
                );
                if let Some(object) = &self.tag_object {
                    ensure!(is_full_sha(object), "tag_object must be a full object ID");
                }
            }
        }
        Ok(())
    }
}

impl Patch {
    pub fn message(&self) -> String {
        format!(
            "{}\n\nDownstream-Reason: {}\nUpstream-Status: {}\nDrop-When: {}",
            self.name, self.purpose, self.upstream_status, self.drop_when
        )
    }
}

fn validate_patch<'a>(
    repo: &Path,
    patch: &'a Patch,
    names: &mut HashSet<&'a String>,
    exports: &mut HashSet<&'a String>,
    seen_tooling: &mut bool,
) -> Result<()> {
    validate_patch_fields(repo, patch)?;
    ensure!(
        names.insert(&patch.name),
        "duplicate patch name: {}",
        patch.name
    );
    match patch.kind {
        PatchKind::Source => ensure!(
            !*seen_tooling,
            "source patch {} appears after tooling",
            patch.name
        ),
        PatchKind::Tooling => {
            *seen_tooling = true;
            ensure!(
                patch.export.is_none(),
                "tooling patch {} cannot export",
                patch.name
            );
        }
    }
    if let Some(path) = &patch.export {
        validate_repo_path(repo, path)?;
        ensure!(exports.insert(path), "duplicate export path: {path}");
    }
    Ok(())
}

fn validate_historical_patch(repo: &Path, patch: &Patch) -> Result<()> {
    validate_patch_fields(repo, patch)?;
    if let Some(path) = &patch.export {
        validate_repo_path(repo, path)?;
    }
    Ok(())
}

fn validate_patch_fields(repo: &Path, patch: &Patch) -> Result<()> {
    ensure!(
        valid_patch_name(&patch.name),
        "invalid patch name: {:?}",
        patch.name
    );
    for (label, value) in [
        ("purpose", patch.purpose.as_str()),
        ("upstream_status", patch.upstream_status.as_str()),
        ("drop_when", patch.drop_when.as_str()),
    ] {
        ensure!(
            valid_metadata(value),
            "invalid {label} for patch {}",
            patch.name
        );
    }
    ensure!(
        !patch.paths.is_empty(),
        "patch {} must declare paths",
        patch.name
    );
    let mut paths = HashSet::new();
    for path in &patch.paths {
        validate_pattern(path)?;
        ensure!(
            paths.insert(path),
            "duplicate path {path} in patch {}",
            patch.name
        );
    }
    let _ = repo;
    Ok(())
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_patch_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_whitespace)
}

fn valid_metadata(value: &str) -> bool {
    !value.trim().is_empty() && value.trim() == value && !value.contains(['\n', '\r'])
}

fn contains_glob(value: &str) -> bool {
    value.contains(['*', '?', '[', ']'])
}

fn validate_pattern(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && !value.starts_with('/') && !value.contains(".."),
        "invalid repository path pattern: {value}"
    );
    Ok(())
}

fn validate_repo_path(repo: &Path, value: &str) -> Result<()> {
    let path = Path::new(value);
    ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "path must be repository-relative: {value}"
    );
    ensure!(
        repo.join(path).starts_with(repo),
        "path escapes repository: {value}"
    );
    Ok(())
}

fn path_matches(pattern: &str, path: &str) -> bool {
    crate::pattern::matches(pattern, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        let commit = "0".repeat(40);
        Manifest {
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
                stack: commit,
            },
            ledger: "PATCHES.md".into(),
            bookkeeping_patch: "fork-tooling".into(),
            patches: vec![Patch {
                name: "fork-tooling".into(),
                kind: PatchKind::Tooling,
                purpose: "Own downstream tooling.".into(),
                upstream_status: "downstream-only".into(),
                drop_when: "The fork is retired.".into(),
                paths: vec!["fork.json".into(), "PATCHES.md".into(), "patches/*".into()],
                export: None,
            }],
            history: Vec::new(),
            allow: Allow::default(),
            required: Vec::new(),
        }
    }

    #[test]
    fn accepts_tooling_only_stack() {
        let directory = tempfile::tempdir().unwrap();
        manifest()
            .validate(directory.path(), &directory.path().join("fork.json"))
            .unwrap();
    }

    #[test]
    fn rejects_source_after_tooling() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value.patches.push(source_patch(None));
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .is_err()
        );
    }

    #[test]
    fn inserts_before_the_correct_layer() {
        let mut value = manifest();
        value.patches.insert(0, source_patch(None));
        assert_eq!(value.insertion_index(PatchKind::Source), 1);
        assert_eq!(value.insertion_index(PatchKind::Tooling), 1);
    }

    #[test]
    fn rejects_export_not_owned_by_bookkeeping() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value
            .patches
            .insert(0, source_patch(Some("exports/source.patch")));
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .is_err()
        );
    }

    #[test]
    fn rejects_export_colliding_with_source_path() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value
            .patches
            .insert(0, source_patch(Some("patches/source.patch")));
        value.patches[0].paths.push("patches/*".into());
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .is_err()
        );
    }

    #[test]
    fn rejects_glob_export_path() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value
            .patches
            .insert(0, source_patch(Some("patches/*.patch")));
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .is_err()
        );
    }

    fn source_patch(export: Option<&str>) -> Patch {
        Patch {
            name: "source".into(),
            kind: PatchKind::Source,
            purpose: "Change source.".into(),
            upstream_status: "not-submitted".into(),
            drop_when: "Upstream changes.".into(),
            paths: vec!["src/lib.rs".into()],
            export: export.map(str::to_string),
        }
    }
}
