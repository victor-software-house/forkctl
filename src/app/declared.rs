use super::App;
use crate::error::DomainError;
use crate::manifest::{Check, CheckStage, Patch, scope_matches};
use crate::process::{capture, command, run};
use crate::protocol::CheckFinding;
use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Longest command line forkctl will hand to the shell after expanding `{files}`.
const MAX_COMMAND_BYTES: usize = 96 * 1024;

const FILES_TEMPLATE: &str = "{files}";

impl App {
    /// Runs every declared patch check against the tree it observes.
    ///
    /// Stack checks run in the repository with the complete series applied. Patch checks run in a
    /// disposable clone checked out at the declaring patch's own commit, so an invariant can be
    /// verified for that layer alone. Checks never touch the operator's worktree.
    pub(super) fn run_declared_checks(&self) -> Result<Vec<CheckFinding>> {
        let manifest = self.manifest()?;
        if manifest.check_count() == 0 {
            return Ok(Vec::new());
        }
        let mut findings = Vec::new();
        let mut stack_layer = None;
        for patch in &manifest.patches {
            let mut patch_layer = None;
            for check in &patch.checks {
                let prepared = match check.at {
                    CheckStage::Stack => {
                        if let Some(prepared) = &stack_layer {
                            prepared
                        } else {
                            let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
                            stack_layer.insert(self.tree_layer(&head)?)
                        }
                    }
                    CheckStage::Patch => {
                        if let Some(prepared) = &patch_layer {
                            prepared
                        } else {
                            let commit = self.patch_commit(&patch.name)?;
                            patch_layer.insert(self.tree_layer(&commit)?)
                        }
                    }
                };
                let globs = patch.check_globs(check);
                let files = prepared
                    .files
                    .iter()
                    .filter(|path| globs.iter().any(|glob| scope_matches(glob, path)))
                    .cloned()
                    .collect::<Vec<_>>();
                if files.is_empty() {
                    findings.push(stale_finding(patch, check, globs));
                    continue;
                }
                if let Some(finding) = run_check(patch, check, &prepared.root, &files)? {
                    findings.push(finding);
                }
            }
        }
        Ok(findings)
    }

    /// Materializes one observed tree in a disposable clone.
    fn tree_layer(&self, commit: &str) -> Result<CheckLayer> {
        let directory = tempfile::tempdir().context("create check layer directory")?;
        let root = directory.path().join("repo");
        run(
            &self.repo,
            "git",
            [
                OsStr::new("clone"),
                OsStr::new("--shared"),
                OsStr::new("--quiet"),
                OsStr::new("--no-checkout"),
                self.repo.as_os_str(),
                root.as_os_str(),
            ],
        )?;
        run(&root, "git", ["checkout", "--quiet", "--detach", commit])?;
        run(&root, "git", ["remote", "remove", "origin"])?;
        let files = capture(&root, "git", ["ls-files", "--cached"])?
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        Ok(CheckLayer {
            _directory: directory,
            root,
            files,
        })
    }
}

fn run_check(
    patch: &Patch,
    check: &Check,
    root: &Path,
    files: &[String],
) -> Result<Option<CheckFinding>> {
    let expanded = if check.run.contains(FILES_TEMPLATE) {
        let quoted = shell_words::join(files);
        check.run.replace(FILES_TEMPLATE, &quoted)
    } else {
        check.run.clone()
    };
    if expanded.len() > MAX_COMMAND_BYTES {
        return Err(DomainError::check_failed(format!(
                "check {} of patch {} expands to {} bytes over {} files; narrow its glob or pass patterns to the tool instead of {FILES_TEMPLATE}",
                check.name,
                patch.name,
                expanded.len(),
                files.len(),
            ))
            .into());
    }
    let output = command(root, "sh")
        .args([OsStr::new("-c"), OsStr::new(expanded.as_str())])
        .output()
        .with_context(|| format!("run check {} of patch {}", check.name, patch.name))?;
    if output.status.success() {
        return Ok(None);
    }
    let mut diagnostics = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if diagnostics.is_empty() {
        diagnostics = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }
    let status = output
        .status
        .code()
        .map_or_else(|| "signal".to_string(), |code| code.to_string());
    Ok(Some(CheckFinding {
        code: "check_command_failed".into(),
        subject: format!("{}: {}", patch.name, check.name),
        message: format!(
            "`{}` exited {status} over {} file(s){}",
            check.run,
            files.len(),
            diagnostics_suffix(&diagnostics)
        ),
    }))
}

struct CheckLayer {
    _directory: tempfile::TempDir,
    root: PathBuf,
    files: Vec<String>,
}

fn diagnostics_suffix(diagnostics: &str) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }
    let line = diagnostics.lines().next().unwrap_or_default();
    format!(": {line}")
}

fn stale_finding(patch: &Patch, check: &Check, globs: &[String]) -> CheckFinding {
    CheckFinding {
        code: "check_glob_stale".into(),
        subject: format!("{}: {}", patch.name, check.name),
        message: format!(
            "{} matches no tracked file, so the check would pass over nothing; its subject moved or was removed",
            globs.join(", ")
        ),
    }
}
