mod init;
mod new;
mod publish;
mod rebase;
mod status;
mod verify;

use crate::ledger;
use crate::manifest::{BaseTarget, Manifest, Patch, TargetKind};
use crate::pattern;
use crate::process::{capture, output, run, succeeds};
use crate::state::{PatchCommitEvidence, PendingOperation, PendingState};
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
    repo: PathBuf,
    manifest_path: PathBuf,
    manifest: Manifest,
}

impl App {
    pub fn load(manifest_arg: &Path) -> Result<Self> {
        let cwd = env::current_dir().context("read current directory")?;
        let repo = PathBuf::from(capture(&cwd, "git", ["rev-parse", "--show-toplevel"])?);
        let manifest_path = if manifest_arg.is_absolute() {
            manifest_arg.to_owned()
        } else {
            repo.join(manifest_arg)
        };
        let bytes = match fs::read(&manifest_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let snapshot = git_private_path(&repo, "forkctl/manifest.json")?;
                fs::read(&snapshot).with_context(|| {
                    format!(
                        "read {} or pending snapshot {}",
                        manifest_path.display(),
                        snapshot.display()
                    )
                })?
            }
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", manifest_path.display()));
            }
        };
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse {}", manifest_path.display()))?;
        manifest.validate(&repo, &manifest_path)?;
        Ok(Self {
            repo,
            manifest_path,
            manifest,
        })
    }

    fn require_clean(&self) -> Result<()> {
        ensure!(self.dirty_lines()?.is_empty(), "worktree is not clean");
        Ok(())
    }

    fn require_declared_branch(&self) -> Result<()> {
        let actual = self.current_branch()?;
        ensure!(
            actual == self.manifest.downstream.branch,
            "current branch is {actual}, expected {}",
            self.manifest.downstream.branch
        );
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
            self.manifest.downstream.remote, self.manifest.downstream.branch
        );
        ensure!(
            tracking == expected,
            "branch tracks {tracking}, expected {expected}"
        );
        Ok(())
    }

    fn current_branch(&self) -> Result<String> {
        capture(
            &self.repo,
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
        )
        .context("repository is in detached HEAD state")
    }

    fn dirty_lines(&self) -> Result<Vec<String>> {
        Ok(nonempty_lines(&capture(
            &self.repo,
            "git",
            ["status", "--porcelain=v1"],
        )?))
    }

    fn upstream_tracking_ref(&self) -> String {
        let branch = self
            .manifest
            .upstream
            .fetch_ref
            .strip_prefix("refs/heads/")
            .expect("validated fetch ref");
        format!("refs/remotes/{}/{}", self.manifest.upstream.remote, branch)
    }

    fn fetch_upstream(&self, quiet: bool) -> Result<()> {
        let upstream = &self.manifest.upstream;
        let destination = self.upstream_tracking_ref();
        let refspec = format!("+{}:{destination}", upstream.fetch_ref);
        let mut args = vec!["fetch"];
        if quiet {
            args.push("--quiet");
        }
        args.extend(["--no-tags", upstream.remote.as_str(), refspec.as_str()]);
        run(&self.repo, "git", args)
    }

    fn fetch_base_target(&self, quiet: bool) -> Result<()> {
        let target = &self.manifest.base.target;
        let selector = if target.kind == TargetKind::Tag && target.tag_object.is_some() {
            target.selector.as_str()
        } else {
            target.commit.as_str()
        };
        let mut args = vec!["fetch"];
        if quiet {
            args.push("--quiet");
        }
        args.extend([
            "--no-tags",
            self.manifest.upstream.remote.as_str(),
            selector,
        ]);
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

    fn fetch_recovery_tags(&self, quiet: bool) -> Result<()> {
        let pattern = format!(
            "refs/tags/{}-*:refs/tags/{}-*",
            self.manifest.downstream.backup_tag_prefix, self.manifest.downstream.backup_tag_prefix
        );
        let mut args = vec!["fetch"];
        if quiet {
            args.push("--quiet");
        }
        args.extend([
            "--no-tags",
            self.manifest.downstream.remote.as_str(),
            pattern.as_str(),
        ]);
        run(&self.repo, "git", args)
    }

    fn resolve_target(&self, selector: &str) -> Result<BaseTarget> {
        ensure!(!selector.trim().is_empty(), "rebase target is required");
        let kind = if is_full_sha(selector) {
            TargetKind::Commit
        } else if selector.starts_with("refs/heads/") {
            TargetKind::Branch
        } else if selector.starts_with("refs/tags/") {
            TargetKind::Tag
        } else {
            anyhow::bail!("target must be a full refs/heads ref, refs/tags ref, or commit SHA");
        };
        run(
            &self.repo,
            "git",
            [
                "fetch",
                "--no-tags",
                self.manifest.upstream.remote.as_str(),
                selector,
            ],
        )?;
        let commit = capture(&self.repo, "git", ["rev-parse", "FETCH_HEAD^{commit}"])?;
        let tag_object = if kind == TargetKind::Tag {
            let object = self.remote_ref_sha(&self.manifest.upstream.remote, selector)?;
            match capture(&self.repo, "git", ["cat-file", "-t", &object])?.as_str() {
                "tag" => Some(object),
                "commit" => {
                    ensure!(
                        object == commit,
                        "lightweight tag object differs from target commit"
                    );
                    None
                }
                object_type => anyhow::bail!("tag points to unsupported object type {object_type}"),
            }
        } else {
            None
        };
        let target = BaseTarget {
            kind,
            selector: selector.to_string(),
            commit,
            tag_object,
        };
        target.validate()?;
        self.verify_target_evidence(&target)?;
        Ok(target)
    }

    fn verify_target_evidence(&self, target: &BaseTarget) -> Result<()> {
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
            let tag_name = capture(&self.repo, "git", ["cat-file", "tag", object])?
                .lines()
                .find_map(|line| line.strip_prefix("tag "))
                .context("annotated tag object has no tag name")?
                .to_string();
            ensure!(
                target.selector == format!("refs/tags/{tag_name}"),
                "tag selector does not match annotated tag name"
            );
        }
        Ok(())
    }

    fn stg_series(&self) -> Result<Vec<String>> {
        Ok(nonempty_lines(&capture(
            &self.repo,
            "stg",
            ["series", "--all", "--no-prefix"],
        )?))
    }

    fn patch_commit(&self, patch: &str) -> Result<String> {
        capture(&self.repo, "stg", ["id", patch])
    }

    fn patch_paths(&self, commit: &str) -> Result<Vec<String>> {
        Ok(nonempty_lines(&capture(
            &self.repo,
            "git",
            ["diff-tree", "--no-commit-id", "--name-only", "-r", commit],
        )?))
    }

    fn verify_allowed_paths(candidates: &[String], allow: &[String], label: &str) -> Result<()> {
        for candidate in candidates {
            ensure!(
                allow
                    .iter()
                    .any(|allowed| pattern::matches(allowed, candidate)),
                "undeclared {label}: {candidate}"
            );
        }
        Ok(())
    }

    fn verify_allowed_diff(
        &self,
        from: &str,
        to: &str,
        allow: &[String],
        label: &str,
    ) -> Result<()> {
        let candidates = nonempty_lines(&capture(
            &self.repo,
            "git",
            ["diff", "--name-only", from, to],
        )?);
        Self::verify_allowed_paths(&candidates, allow, label)
    }

    fn export_patch(&self, patch: &Patch) -> Result<Vec<u8>> {
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

    fn write_exports(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for patch in self.manifest.exported_patches() {
            let path = self
                .repo
                .join(patch.export.as_ref().expect("exported patch"));
            write_atomic(&path, &self.export_patch(patch)?)?;
            paths.push(path);
        }
        Ok(paths)
    }

    fn reconstruct_tree(&self) -> Result<String> {
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
                "verify-stack",
                &self.manifest.base.stack,
            ],
        )?;
        run(&clone, "stg", ["init"])?;
        for patch in self.manifest.exported_patches() {
            let path = self
                .repo
                .join(patch.export.as_ref().expect("exported patch"));
            run(
                &clone,
                "stg",
                [OsStr::new("import"), OsStr::new("--3way"), path.as_os_str()],
            )?;
        }
        capture(&clone, "git", ["rev-parse", "HEAD^{tree}"])
    }

    fn expected_reconstructed_tree(&self) -> Result<String> {
        if let Some(patch) = self.manifest.exported_patches().last() {
            let commit = self.patch_commit(&patch.name)?;
            capture(
                &self.repo,
                "git",
                ["rev-parse", &format!("{commit}^{{tree}}")],
            )
        } else {
            capture(
                &self.repo,
                "git",
                [
                    "rev-parse",
                    &format!("{}^{{tree}}", self.manifest.base.stack),
                ],
            )
        }
    }

    fn write_manifest(&self) -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(&self.manifest)?;
        bytes.push(b'\n');
        write_atomic(&self.manifest_path, &bytes)
    }

    fn write_ledger(&self) -> Result<PathBuf> {
        let path = self.repo.join(&self.manifest.ledger);
        write_atomic(&path, ledger::render(&self.manifest)?.as_bytes())?;
        Ok(path)
    }

    fn stage_and_refresh_bookkeeping(&self, paths: &[PathBuf]) -> Result<()> {
        let mut relative = paths
            .iter()
            .map(|path| relative_to(&self.repo, path))
            .collect::<Result<Vec<_>>>()?;
        relative.sort();
        relative.dedup();
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
            let top = capture(&self.repo, "stg", ["top"])?;
            ensure!(
                top == self.manifest.bookkeeping_patch,
                "top patch is {top}, expected bookkeeping patch {}",
                self.manifest.bookkeeping_patch
            );
            run(&self.repo, "stg", ["refresh", "--index"])?;
        }
        Ok(())
    }

    fn downstream_ref(&self) -> String {
        format!("refs/heads/{}", self.manifest.downstream.branch)
    }

    fn remote_ref_sha(&self, remote: &str, git_ref: &str) -> Result<String> {
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

    fn downstream_sha(&self) -> Result<String> {
        self.remote_ref_sha(&self.manifest.downstream.remote, &self.downstream_ref())
    }

    fn git_private_path(&self, relative: &str) -> Result<PathBuf> {
        git_private_path(&self.repo, relative)
    }

    fn pending_path(&self) -> Result<PathBuf> {
        self.git_private_path("forkctl/pending.json")
    }

    fn file_object_id(&self, path: &Path) -> Result<String> {
        capture(
            &self.repo,
            "git",
            [OsStr::new("hash-object"), path.as_os_str()],
        )
    }

    fn read_pending(&self) -> Result<Option<PendingState>> {
        let path = self.pending_path()?;
        match fs::read(&path) {
            Ok(bytes) => {
                let state: PendingState = serde_json::from_slice(&bytes)
                    .with_context(|| format!("parse {}", path.display()))?;
                ensure!(
                    state.schema == 1,
                    "unsupported pending-state schema: {}",
                    state.schema
                );
                Ok(Some(state))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
        }
    }

    fn write_pending(&self, state: &PendingState) -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(state)?;
        bytes.push(b'\n');
        write_atomic(&self.pending_path()?, &bytes)
    }

    fn clear_pending(&self) -> Result<()> {
        for path in [
            self.pending_path()?,
            self.git_private_path("forkctl/manifest.json")?,
        ] {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| format!("remove {}", path.display()));
                }
            }
        }
        Ok(())
    }

    fn create_recovery(&self, operation: PendingOperation) -> Result<PendingState> {
        ensure!(
            self.read_pending()?.is_none(),
            "a forkctl operation is already pending"
        );
        let expected_remote_sha = self.downstream_sha()?;
        let old_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        let old_tip = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        let old_patches = self
            .manifest
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
            .context("system clock is before Unix epoch")?
            .as_nanos();
        let short = old_tip.get(..12).context("old tip is not a full SHA")?;
        let backup_tag = format!(
            "{}-{epoch}-{short}",
            self.manifest.downstream.backup_tag_prefix
        );
        run(
            &self.repo,
            "git",
            [
                "tag",
                "--annotate",
                "--message",
                &format!("forkctl recovery before {operation:?}"),
                &backup_tag,
                &old_tip,
            ],
        )?;
        let mut manifest = serde_json::to_vec_pretty(&self.manifest)?;
        manifest.push(b'\n');
        write_atomic(&self.git_private_path("forkctl/manifest.json")?, &manifest)?;
        Ok(PendingState::new(
            operation,
            expected_remote_sha,
            old_base,
            old_tip,
            old_patches,
            backup_tag,
        ))
    }
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn git_private_path(repo: &Path, relative: &str) -> Result<PathBuf> {
    let path = PathBuf::from(capture(repo, "git", ["rev-parse", "--git-path", relative])?);
    Ok(if path.is_absolute() {
        path
    } else {
        repo.join(path)
    })
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

fn relative_to(repo: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(repo)
        .with_context(|| format!("{} is outside {}", path.display(), repo.display()))?
        .to_string_lossy()
        .into_owned())
}

fn nonempty_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}
