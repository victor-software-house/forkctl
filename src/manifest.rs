use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub upstream: Upstream,
    pub bases: Bases,
    pub patches: Vec<Patch>,
    #[serde(default)]
    pub allow: Allow,
    #[serde(default)]
    pub required: Vec<RequiredText>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub remote: String,
    pub url: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bases {
    pub canonical: String,
    pub stack: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Allow {
    pub base: Vec<String>,
    pub tooling: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredText {
    pub path: String,
    pub contains: String,
}

impl Manifest {
    pub fn validate(&self, repo: &Path, manifest_path: &Path) -> Result<()> {
        ensure!(
            self.schema == 1,
            "unsupported manifest schema: {}",
            self.schema
        );
        ensure!(
            !self.upstream.remote.is_empty()
                && !self.upstream.url.is_empty()
                && !self.upstream.git_ref.is_empty(),
            "upstream remote, url, and ref are required"
        );
        ensure!(!self.patches.is_empty(), "at least one patch is required");
        for (label, value) in [
            ("canonical base", self.bases.canonical.as_str()),
            ("stack base", self.bases.stack.as_str()),
        ] {
            ensure!(
                value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{label} must be a full 40-character commit SHA"
            );
        }

        let mut names = HashSet::new();
        let mut seen_tooling = false;
        for patch in &self.patches {
            ensure!(
                !patch.name.is_empty() && !patch.name.chars().any(char::is_whitespace),
                "invalid patch name: {:?}",
                patch.name
            );
            ensure!(
                names.insert(&patch.name),
                "duplicate patch name: {}",
                patch.name
            );
            match &patch.export {
                Some(path) => {
                    ensure!(
                        !seen_tooling,
                        "exported patch {} appears after a tooling-only patch",
                        patch.name
                    );
                    validate_repo_path(repo, path)?;
                }
                None => seen_tooling = true,
            }
        }
        self.source_top()?;
        for pattern in self.allow.base.iter().chain(&self.allow.tooling) {
            ensure!(
                !pattern.is_empty() && !pattern.starts_with('/') && !pattern.contains(".."),
                "invalid allow pattern: {pattern}"
            );
        }
        for required in &self.required {
            validate_repo_path(repo, &required.path)?;
            ensure!(
                !required.contains.is_empty(),
                "empty required text for {}",
                required.path
            );
        }
        ensure!(
            manifest_path.starts_with(repo),
            "manifest must live inside the repository"
        );
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

    pub fn source_top(&self) -> Result<&Patch> {
        self.exported_patches()
            .last()
            .context("at least one exported source patch is required")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            schema: 1,
            upstream: Upstream {
                remote: "upstream".into(),
                url: "https://example.com/upstream.git".into(),
                git_ref: "upstream/main".into(),
            },
            bases: Bases {
                canonical: "0".repeat(40),
                stack: "0".repeat(40),
            },
            patches: vec![
                Patch {
                    name: "source".into(),
                    export: Some("patches/0001-source.patch".into()),
                },
                Patch {
                    name: "tooling".into(),
                    export: None,
                },
            ],
            allow: Allow::default(),
            required: Vec::new(),
        }
    }

    #[test]
    fn derives_source_top_from_last_exported_patch() {
        assert_eq!(manifest().source_top().unwrap().name, "source");
    }

    #[test]
    fn rejects_export_after_tooling_patch() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value.patches.push(Patch {
            name: "late-source".into(),
            export: Some("patches/0002-late.patch".into()),
        });
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .unwrap_err()
                .to_string()
                .contains("appears after a tooling-only patch")
        );
    }

    #[test]
    fn rejects_duplicate_patch_names() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value.patches[1].name = "source".into();
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .unwrap_err()
                .to_string()
                .contains("duplicate patch name")
        );
    }

    #[test]
    fn rejects_short_base_sha() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value.bases.stack = "abc123".into();
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .unwrap_err()
                .to_string()
                .contains("full 40-character commit SHA")
        );
    }

    #[test]
    fn rejects_paths_that_escape_repository() {
        let directory = tempfile::tempdir().unwrap();
        let mut value = manifest();
        value.patches[0].export = Some("../outside.patch".into());
        assert!(
            value
                .validate(directory.path(), &directory.path().join("fork.json"))
                .unwrap_err()
                .to_string()
                .contains("repository-relative")
        );
    }
}
