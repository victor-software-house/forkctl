mod init;
mod rebase;
mod verify;

use crate::manifest::Manifest;
use crate::pattern;
use crate::process::{capture, run};
use anyhow::{Context, Result, ensure};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

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
        let bytes = fs::read(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?;
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
        ensure!(
            capture(&self.repo, "git", ["status", "--porcelain"])?.is_empty(),
            "worktree is not clean"
        );
        Ok(())
    }

    fn fetch_upstream(&self, quiet: bool) -> Result<()> {
        let upstream = &self.manifest.upstream;
        let branch = upstream
            .git_ref
            .strip_prefix(&format!("{}/", upstream.remote))
            .with_context(|| {
                format!(
                    "upstream ref {} must start with {}/",
                    upstream.git_ref, upstream.remote
                )
            })?;
        let mut args = vec!["fetch"];
        if quiet {
            args.push("--quiet");
        }
        args.extend([upstream.remote.as_str(), branch]);
        run(&self.repo, "git", args)
    }

    fn stg_series(&self) -> Result<Vec<String>> {
        Ok(nonempty_lines(&capture(
            &self.repo,
            "stg",
            ["series", "--all", "--no-prefix"],
        )?))
    }

    fn verify_allowed_diff(
        &self,
        from: &str,
        to: &str,
        allow: &[String],
        label: &str,
    ) -> Result<()> {
        for candidate in nonempty_lines(&capture(
            &self.repo,
            "git",
            ["diff", "--name-only", from, to],
        )?) {
            ensure!(
                allow
                    .iter()
                    .any(|allowed| pattern::matches(allowed, &candidate)),
                "undeclared {label}: {candidate}"
            );
        }
        Ok(())
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
                &self.manifest.bases.stack,
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

    fn write_manifest(&self) -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(&self.manifest)?;
        bytes.push(b'\n');
        write_atomic(&self.manifest_path, &bytes)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
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
