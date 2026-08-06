mod check;
mod contract;
mod declared;
mod init;
mod operation;
mod patch;
mod publish;
mod rebase;
mod status;

use crate::error::DomainError;
use crate::ledger;
use crate::manifest::{BaseTarget, Manifest, Patch, RecoveryEvidence, TargetKind};
use crate::manifest_codec::ManifestFormat;
use crate::process::{capture, output, run, succeeds};
use crate::state::{ActivePatchState, OperationKind, OperationState, PatchCommitEvidence};
use anyhow::{Context, Result, ensure};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

const EXPORT_TEMPLATE: &str = include_str!("../patchexport.tmpl");

pub struct App {
    pub(super) repo: PathBuf,
    pub(super) manifest_path: PathBuf,
    pub(super) manifest_format: ManifestFormat,
    pub(super) manifest: Option<Manifest>,
    pub(super) manifest_error: Option<String>,
}

impl App {
    pub fn discover(manifest_arg: &Path) -> Result<Self> {
        let cwd = env::current_dir().context("read current directory")?;
        let repo = capture(&cwd, "git", ["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .map_err(|error| DomainError::repository_not_found(error.to_string()))?;
        let manifest_path = if manifest_arg.is_absolute() {
            manifest_arg.to_owned()
        } else {
            repo.join(manifest_arg)
        };
        let manifest_format = ManifestFormat::from_path(&manifest_path)?;
        let (manifest, manifest_error) = match fs::read(&manifest_path) {
            Ok(bytes) => match manifest_format.parse(&bytes, &manifest_path) {
                Ok(manifest) => match manifest.validate(&repo, &manifest_path) {
                    Ok(()) => (Some(manifest), None),
                    Err(error) => (None, Some(error.to_string())),
                },
                Err(error) => (None, Some(error.to_string())),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, None),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", manifest_path.display()));
            }
        };
        Ok(Self {
            repo,
            manifest_path,
            manifest_format,
            manifest,
            manifest_error,
        })
    }

    pub(super) fn manifest(&self) -> Result<&Manifest> {
        match (&self.manifest, &self.manifest_error) {
            (Some(manifest), _) => Ok(manifest),
            (None, Some(error)) => Err(DomainError::manifest_invalid(error.clone()).into()),
            (None, None) => Err(DomainError::manifest_invalid(format!(
                "no forkctl manifest at {}",
                self.manifest_path.display()
            ))
            .into()),
        }
    }

    pub(super) fn manifest_present(&self) -> bool {
        self.manifest.is_some() || self.manifest_error.is_some()
    }

    pub(super) fn manifest_mut(&mut self) -> Result<&mut Manifest> {
        if self.manifest.is_none() {
            self.manifest()?;
        }
        self.manifest
            .as_mut()
            .context("forkctl manifest is unavailable")
    }

    pub(super) fn require_clean(&self) -> Result<()> {
        let paths = self.dirty_paths()?;
        if paths.is_empty() {
            Ok(())
        } else {
            Err(DomainError::dirty_worktree(paths).into())
        }
    }

    pub(super) fn require_declared_branch(&self) -> Result<()> {
        let manifest = self.manifest()?;
        let actual = self.current_branch()?;
        if actual != manifest.downstream.branch {
            return Err(DomainError::invalid_request(format!(
                "current branch is {actual}, expected {}",
                manifest.downstream.branch
            ))
            .into());
        }
        let tracking = capture(
            &self.repo,
            "git",
            [
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        )?;
        let expected = format!(
            "{}/{}",
            manifest.downstream.remote, manifest.downstream.branch
        );
        if tracking != expected {
            return Err(DomainError::invalid_request(format!(
                "branch tracks {tracking}, expected {expected}"
            ))
            .into());
        }
        Ok(())
    }

    pub(super) fn current_branch(&self) -> Result<String> {
        capture(
            &self.repo,
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
        )
        .map_err(|_| DomainError::invalid_request("repository is in detached HEAD state").into())
    }

    pub(super) fn worktree_inventory(&self) -> Result<WorktreeInventory> {
        let output = capture(
            &self.repo,
            "git",
            ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        )?;
        let mut inventory = WorktreeInventory::default();
        let mut fields = output.split('\0').filter(|entry| !entry.is_empty());
        while let Some(entry) = fields.next() {
            if entry.len() < 4 {
                continue;
            }
            let bytes = entry.as_bytes();
            let x = bytes[0] as char;
            let y = bytes[1] as char;
            let path = entry[3..].to_string();
            // Rename and copy entries carry their original path in the following field. Both
            // sides are affected, so both are inventoried; reading the original as its own
            // entry would strip three characters from a real repository path.
            let origin = if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
                fields.next().map(str::to_string)
            } else {
                None
            };
            if x == '?' && y == '?' {
                inventory.untracked.push(path);
            } else {
                if x != ' ' {
                    inventory.staged.push(path.clone());
                    inventory.staged.extend(origin.clone());
                }
                if y != ' ' {
                    inventory.unstaged.push(path);
                    inventory.unstaged.extend(origin);
                }
            }
        }
        for values in [
            &mut inventory.staged,
            &mut inventory.unstaged,
            &mut inventory.untracked,
        ] {
            values.sort();
            values.dedup();
        }
        Ok(inventory)
    }

    pub(super) fn dirty_paths(&self) -> Result<Vec<String>> {
        let inventory = self.worktree_inventory()?;
        let mut paths = inventory.staged;
        paths.extend(inventory.unstaged);
        paths.extend(inventory.untracked);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub(super) fn upstream_tracking_ref(&self) -> Result<String> {
        let manifest = self.manifest()?;
        let branch = manifest
            .upstream
            .fetch_ref
            .strip_prefix("refs/heads/")
            .context("validated upstream branch ref")?;
        Ok(format!(
            "refs/remotes/{}/{branch}",
            manifest.upstream.remote
        ))
    }

    pub(super) fn fetch_upstream(&self, quiet: bool) -> Result<()> {
        let manifest = self.manifest()?;
        let destination = self.upstream_tracking_ref()?;
        let refspec = format!("+{}:{destination}", manifest.upstream.fetch_ref);
        let mut args = vec!["fetch"];
        if quiet {
            args.push("--quiet");
        }
        args.extend([
            "--no-tags",
            manifest.upstream.remote.as_str(),
            refspec.as_str(),
        ]);
        run(&self.repo, "git", args)
    }

    pub(super) fn fetch_target(&self, target: &BaseTarget, quiet: bool) -> Result<()> {
        let manifest = self.manifest()?;
        let selector = if target.kind == TargetKind::Tag && target.tag_object.is_some() {
            target.selector.as_str()
        } else {
            target.commit.as_str()
        };
        let mut args = vec!["fetch"];
        if quiet {
            args.push("--quiet");
        }
        args.extend(["--no-tags", manifest.upstream.remote.as_str(), selector]);
        run(&self.repo, "git", args)?;
        let resolved = capture(&self.repo, "git", ["rev-parse", "FETCH_HEAD^{commit}"])?;
        ensure!(
            resolved == target.commit,
            "recorded target {} resolves to {resolved}, expected {}",
            target.selector,
            target.commit
        );
        self.verify_target_evidence(target)
    }

    pub(super) fn resolve_target(&self, selector: &str) -> Result<BaseTarget> {
        let manifest = self.manifest()?;
        resolve_target(&self.repo, &manifest.upstream.remote, selector)
    }

    pub(super) fn verify_target_evidence(&self, target: &BaseTarget) -> Result<()> {
        target.validate()?;
        run(
            &self.repo,
            "git",
            ["cat-file", "-e", &format!("{}^{{commit}}", target.commit)],
        )?;
        if let Some(object) = &target.tag_object {
            ensure!(
                capture(&self.repo, "git", ["cat-file", "-t", object])? == "tag",
                "tag_object is not an annotated tag"
            );
            let peeled = capture(
                &self.repo,
                "git",
                ["rev-parse", &format!("{object}^{{commit}}")],
            )?;
            ensure!(
                peeled == target.commit,
                "tag object peels to {peeled}, expected {}",
                target.commit
            );
        }
        Ok(())
    }

    pub(super) fn stg_series(&self) -> Result<Vec<String>> {
        Ok(nonempty_lines(&capture(
            &self.repo,
            "stg",
            ["series", "--all", "--no-prefix"],
        )?))
    }

    pub(super) fn patch_commit(&self, patch: &str) -> Result<String> {
        capture(&self.repo, "stg", ["id", patch])
    }

    pub(super) fn patch_paths(&self, commit: &str) -> Result<Vec<String>> {
        Ok(nonempty_lines(&capture(
            &self.repo,
            "git",
            ["diff-tree", "--no-commit-id", "--name-only", "-r", commit],
        )?))
    }

    pub(super) fn export_patch(&self, patch: &Patch) -> Result<Vec<u8>> {
        let directory = tempfile::tempdir().context("create patch export directory")?;
        let template_path = directory.path().join("patchexport.tmpl");
        fs::write(&template_path, EXPORT_TEMPLATE)
            .with_context(|| format!("write {}", template_path.display()))?;
        Ok(output(
            &self.repo,
            "stg",
            [
                OsStr::new("export"),
                OsStr::new("--stdout"),
                OsStr::new("--template"),
                template_path.as_os_str(),
                OsStr::new(&patch.name),
            ],
        )?
        .stdout)
    }

    pub(super) fn write_exports(&self) -> Result<Vec<PathBuf>> {
        let manifest = self.manifest()?;
        let mut expected = Vec::new();
        let exports_dir = self.repo.join(&manifest.documents.exports);
        fs::create_dir_all(&exports_dir)?;
        for export in manifest.source_exports() {
            let path = self.repo.join(&export.path);
            write_atomic(&path, &self.export_patch(export.patch)?)?;
            expected.push(path);
        }
        let expected_set = expected
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        for entry in fs::read_dir(exports_dir)? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "patch")
                && !expected_set.contains(&path)
            {
                fs::remove_file(&path)?;
                expected.push(path);
            }
        }
        Ok(expected)
    }

    pub(super) fn reconstruct_tree(&self) -> Result<String> {
        let manifest = self.manifest()?;
        let temp = tempfile::tempdir().context("create verification directory")?;
        let clone = temp.path().join("repo");
        run(
            &self.repo,
            "git",
            [
                OsStr::new("clone"),
                OsStr::new("--shared"),
                OsStr::new("--quiet"),
                OsStr::new("--no-checkout"),
                self.repo.as_os_str(),
                clone.as_os_str(),
            ],
        )?;
        run(
            &clone,
            "git",
            [
                "checkout",
                "--quiet",
                "-b",
                "check-stack",
                &manifest.base.stack,
            ],
        )?;
        run(&clone, "stg", ["init"])?;
        for export in manifest.source_exports() {
            let path = self.repo.join(export.path);
            run(
                &clone,
                "stg",
                [OsStr::new("import"), OsStr::new("--3way"), path.as_os_str()],
            )?;
        }
        capture(&clone, "git", ["rev-parse", "HEAD^{tree}"])
    }

    pub(super) fn expected_reconstructed_tree(&self) -> Result<String> {
        let manifest = self.manifest()?;
        if let Some(export) = manifest.source_exports().last() {
            let commit = self.patch_commit(&export.patch.name)?;
            capture(
                &self.repo,
                "git",
                ["rev-parse", &format!("{commit}^{{tree}}")],
            )
        } else {
            capture(
                &self.repo,
                "git",
                ["rev-parse", &format!("{}^{{tree}}", manifest.base.stack)],
            )
        }
    }

    pub(super) fn write_manifest(&self) -> Result<()> {
        write_atomic(
            &self.manifest_path,
            &self.manifest_format.serialize(self.manifest()?)?,
        )
    }

    pub(super) fn write_ledger(&self) -> Result<PathBuf> {
        let manifest = self.manifest()?;
        let path = self.repo.join(&manifest.documents.ledger);
        write_atomic(&path, ledger::render(manifest)?.as_bytes())?;
        Ok(path)
    }

    pub(super) fn refresh_bookkeeping(&self, paths: &[PathBuf]) -> Result<()> {
        let manifest = self.manifest()?;
        let mut relative = paths
            .iter()
            .map(|path| relative_to(&self.repo, path))
            .collect::<Result<Vec<_>>>()?;
        relative.sort();
        relative.dedup();
        if relative.is_empty() {
            return Ok(());
        }
        let mut add_args = vec![OsString::from("add"), OsString::from("--")];
        add_args.extend(relative.iter().cloned().map(OsString::from));
        run(&self.repo, "git", add_args)?;
        let mut diff_args = vec![
            OsString::from("diff"),
            OsString::from("--cached"),
            OsString::from("--quiet"),
            OsString::from("--"),
        ];
        diff_args.extend(relative.into_iter().map(OsString::from));
        if !succeeds(&self.repo, "git", diff_args)? {
            let previous_active = self.read_active()?;
            self.write_active(&ActivePatchState::Existing {
                patch: manifest.bookkeeping_patch.clone(),
            })?;
            let refresh_result = run(
                &self.repo,
                "stg",
                ["refresh", "--patch", &manifest.bookkeeping_patch, "--index"],
            );
            match previous_active {
                Some(active) => self.write_active(&active)?,
                None => self.clear_active()?,
            }
            refresh_result?;
        }
        Ok(())
    }

    pub(super) fn downstream_ref(&self) -> Result<String> {
        Ok(format!("refs/heads/{}", self.manifest()?.downstream.branch))
    }

    pub(super) fn remote_ref_sha(&self, remote: &str, git_ref: &str) -> Result<String> {
        let line = capture(
            &self.repo,
            "git",
            ["ls-remote", "--exit-code", remote, git_ref],
        )?;
        let mut fields = line.split_whitespace();
        let sha = fields.next().context("remote ref output has no SHA")?;
        ensure!(
            fields.next() == Some(git_ref),
            "unexpected remote ref output: {line}"
        );
        Ok(sha.to_string())
    }

    pub(super) fn downstream_sha(&self) -> Result<String> {
        let manifest = self.manifest()?;
        self.remote_ref_sha(&manifest.downstream.remote, &self.downstream_ref()?)
    }

    pub(super) fn downstream_tracking_sha(&self) -> Result<String> {
        let manifest = self.manifest()?;
        capture(
            &self.repo,
            "git",
            [
                "rev-parse",
                &format!(
                    "refs/remotes/{}/{}",
                    manifest.downstream.remote, manifest.downstream.branch
                ),
            ],
        )
    }

    pub(super) fn git_private_path(&self, relative: &str) -> Result<PathBuf> {
        git_private_path(&self.repo, relative)
    }

    pub(super) fn active_path(&self) -> Result<PathBuf> {
        self.git_private_path("forkctl/active.json")
    }

    pub(super) fn operation_path(&self) -> Result<PathBuf> {
        self.git_private_path("forkctl/operation.json")
    }

    pub(super) fn operation_manifest_snapshot_path(&self) -> Result<PathBuf> {
        self.git_private_path("forkctl/manifest.json")
    }

    pub(super) fn read_active(&self) -> Result<Option<ActivePatchState>> {
        read_optional_json(&self.active_path()?)
    }

    pub(super) fn write_active(&self, state: &ActivePatchState) -> Result<()> {
        write_json_atomic(&self.active_path()?, state)
    }

    pub(super) fn clear_active(&self) -> Result<()> {
        remove_optional(&self.active_path()?)
    }

    pub(super) fn read_operation(&self) -> Result<Option<OperationState>> {
        read_optional_json(&self.operation_path()?)
    }

    pub(super) fn load_operation_manifest(&mut self) -> Result<()> {
        if self.manifest.is_some() {
            return Ok(());
        }
        let snapshot = self.operation_manifest_snapshot_path()?;
        let bytes = fs::read(&snapshot)
            .with_context(|| format!("read operation manifest snapshot {}", snapshot.display()))?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse operation manifest snapshot {}", snapshot.display()))?;
        manifest.validate(&self.repo, &self.manifest_path)?;
        self.manifest = Some(manifest);
        self.manifest_error = None;
        Ok(())
    }

    pub(super) fn write_operation(&self, state: &OperationState) -> Result<()> {
        write_json_atomic(&self.operation_path()?, state)
    }

    pub(super) fn clear_operation(&self) -> Result<()> {
        remove_optional(&self.operation_path()?)
    }

    pub(super) fn complete_local_operation(&self, operation: &OperationState) -> Result<()> {
        run(
            &self.repo,
            "git",
            ["tag", "--delete", &operation.recovery.tag],
        )?;
        if let Some(recovery) = &operation.publication_recovery {
            run(&self.repo, "git", ["tag", "--delete", &recovery.tag])?;
        }
        remove_optional(&self.operation_manifest_snapshot_path()?)?;
        self.clear_operation()
    }

    pub(super) fn complete_published_operation(&self) -> Result<()> {
        remove_optional(&self.operation_manifest_snapshot_path()?)?;
        self.clear_operation()
    }

    pub(super) fn create_operation(
        &self,
        kind: OperationKind,
        target: Option<BaseTarget>,
    ) -> Result<OperationState> {
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        let manifest = self.manifest()?;
        let expected_remote_sha = self.downstream_sha()?;
        let old_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        let old_tip = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        let old_patches = manifest
            .patches
            .iter()
            .map(|patch| {
                Ok(PatchCommitEvidence {
                    name: patch.name.clone(),
                    commit: self.patch_commit(&patch.name)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before Unix epoch")?;
        let short = old_tip.get(..12).context("old tip is not a full SHA")?;
        let id = format!("{}-{short}", epoch.as_nanos());
        let (tag, tag_object) = self.create_recovery_tag(&id, &old_tip, &format!("{kind:?}"))?;
        let snapshot = self.operation_manifest_snapshot_path()?;
        write_atomic(&snapshot, &serde_json::to_vec_pretty(manifest)?)?;
        Ok(OperationState {
            schema: 1,
            id,
            kind,
            phase: "prepared".into(),
            started_at_unix_ms: epoch.as_millis(),
            expected_remote_sha,
            old_base: old_base.clone(),
            old_tip: old_tip.clone(),
            old_patches,
            recovery: RecoveryEvidence {
                tag,
                tag_object,
                old_base,
                old_tip,
            },
            publication_recovery: None,
            intent: None,
            target,
            new_base: None,
            new_tip: None,
            report: None,
            next_actions: Vec::new(),
        })
    }

    pub(super) fn create_recovery_tag(
        &self,
        id: &str,
        commit: &str,
        purpose: &str,
    ) -> Result<(String, String)> {
        let tag = format!("{}/{id}", self.manifest()?.downstream.recovery_tag_prefix);
        run(
            &self.repo,
            "git",
            [
                "tag",
                "-a",
                &tag,
                "-m",
                &format!("forkctl recovery for {purpose}"),
                commit,
            ],
        )?;
        let tag_object = capture(
            &self.repo,
            "git",
            ["rev-parse", &format!("refs/tags/{tag}")],
        )?;
        Ok((tag, tag_object))
    }

    pub(super) fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        succeeds(
            &self.repo,
            "git",
            ["merge-base", "--is-ancestor", ancestor, descendant],
        )
    }

    pub(super) fn local_tag_object(&self, tag: &str) -> Option<String> {
        capture(
            &self.repo,
            "git",
            ["rev-parse", &format!("refs/tags/{tag}")],
        )
        .ok()
    }

    pub(super) fn file_object_id(&self, path: &Path) -> Result<String> {
        capture(
            &self.repo,
            "git",
            [OsStr::new("hash-object"), path.as_os_str()],
        )
    }
}

pub(super) fn recovery_id(commit: &str) -> Result<String> {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?;
    let short = commit.get(..12).context("commit is not a full SHA")?;
    Ok(format!("{}-{short}", epoch.as_nanos()))
}

#[derive(Default)]
pub(super) struct WorktreeInventory {
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
}

pub(super) fn resolve_target(repo: &Path, remote: &str, selector: &str) -> Result<BaseTarget> {
    if selector.trim().is_empty() {
        return Err(DomainError::invalid_request("target is required").into());
    }
    let kind = if crate::manifest::is_full_sha(selector) {
        TargetKind::Commit
    } else if selector.starts_with("refs/heads/") {
        TargetKind::Branch
    } else if selector.starts_with("refs/tags/") {
        TargetKind::Tag
    } else {
        anyhow::bail!("target must be a full refs/heads ref, refs/tags ref, or commit SHA");
    };
    run(repo, "git", ["fetch", "--no-tags", remote, selector])?;
    let commit = capture(repo, "git", ["rev-parse", "FETCH_HEAD^{commit}"])?;
    let tag_object = if kind == TargetKind::Tag {
        let line = capture(repo, "git", ["ls-remote", "--exit-code", remote, selector])?;
        let object = line
            .split_whitespace()
            .next()
            .context("remote tag output has no object")?
            .to_string();
        (capture(repo, "git", ["cat-file", "-t", &object])? == "tag").then_some(object)
    } else {
        None
    };
    let target = BaseTarget {
        kind,
        selector: if kind == TargetKind::Commit {
            commit.clone()
        } else {
            selector.to_string()
        },
        commit,
        tag_object,
    };
    target.validate()?;
    Ok(target)
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("output path has no parent")?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file_mut().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

fn write_json_atomic(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

fn read_optional_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

fn remove_optional(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("remove {}", path.display())),
    }
}

fn relative_to(repo: &Path, path: &Path) -> Result<String> {
    Ok(path.strip_prefix(repo)?.to_string_lossy().into_owned())
}

fn git_private_path(repo: &Path, relative: &str) -> Result<PathBuf> {
    let path = PathBuf::from(capture(repo, "git", ["rev-parse", "--git-path", relative])?);
    Ok(if path.is_absolute() {
        path
    } else {
        repo.join(path)
    })
}

fn nonempty_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}
