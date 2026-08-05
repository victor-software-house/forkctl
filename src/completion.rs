use crate::process::{capture, command};
use clap::ValueEnum;
use clap_complete::{CompletionCandidate, engine::ArgValueCompleter};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub fn patch_completer() -> ArgValueCompleter {
    ArgValueCompleter::new(patch_candidates)
}

pub fn ref_completer() -> ArgValueCompleter {
    ArgValueCompleter::new(ref_candidates)
}

pub fn remote_completer() -> ArgValueCompleter {
    ArgValueCompleter::new(remote_candidates)
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CandidateKind {
    Patch,
    Ref,
    Remote,
}

pub fn candidate_lines(kind: CandidateKind) -> Vec<String> {
    let values = match kind {
        CandidateKind::Patch => patch_candidates(OsStr::new("")),
        CandidateKind::Ref => ref_candidates(OsStr::new("")),
        CandidateKind::Remote => remote_candidates(OsStr::new("")),
    };
    values
        .into_iter()
        .map(|candidate| {
            let value = candidate.get_value().to_string_lossy().into_owned();
            candidate
                .get_help()
                .map_or_else(|| value.clone(), |help| format!("{value}\t{help}"))
        })
        .collect()
}

fn patch_candidates(current: &OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_string_lossy();
    let Some((repo, manifest_path)) = repository_and_manifest() else {
        return Vec::new();
    };
    let Ok(bytes) = std::fs::read(&manifest_path) else {
        return Vec::new();
    };
    let Ok(format) = crate::manifest_codec::ManifestFormat::from_path(&manifest_path) else {
        return Vec::new();
    };
    let Ok(manifest) = format.parse(&bytes, &manifest_path) else {
        return Vec::new();
    };
    let mut values = manifest.patch_names();
    values.extend(
        manifest
            .disabled_patches
            .iter()
            .map(|record| record.patch.name.clone()),
    );
    let active = git_private(&repo, "forkctl/active.json")
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<crate::state::ActivePatchState>(&bytes).ok())
        .map(|active| active.name().to_string());
    if let Some(active) = active
        && !values.contains(&active)
    {
        values.push(active);
    }
    candidates(values, &prefix)
}

fn ref_candidates(current: &OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_string_lossy();
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    let Ok(output) = capture(
        &cwd,
        "git",
        [
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/tags",
        ],
    ) else {
        return Vec::new();
    };
    candidates(output.lines().map(str::to_string), &prefix)
}

fn remote_candidates(current: &OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_string_lossy();
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    let Ok(output) = capture(&cwd, "git", ["remote"]) else {
        return Vec::new();
    };
    candidates(output.lines().map(str::to_string), &prefix)
}

fn candidates(values: impl IntoIterator<Item = String>, prefix: &str) -> Vec<CompletionCandidate> {
    values
        .into_iter()
        .filter(|value| value.starts_with(prefix))
        .map(CompletionCandidate::new)
        .collect()
}

fn repository_and_manifest() -> Option<(PathBuf, PathBuf)> {
    let cwd = std::env::current_dir().ok()?;
    let repo = PathBuf::from(capture(&cwd, "git", ["rev-parse", "--show-toplevel"]).ok()?);
    let manifest = std::env::var_os("FORK_MANIFEST")
        .map_or_else(|| PathBuf::from("patches/fork.yaml"), PathBuf::from);
    let manifest = if manifest.is_absolute() {
        manifest
    } else {
        repo.join(manifest)
    };
    Some((repo, manifest))
}

fn git_private(repo: &Path, relative: &str) -> Option<PathBuf> {
    let output = command(repo, "git")
        .args(["rev-parse", "--git-path", relative])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(value.trim());
    Some(if path.is_absolute() {
        path
    } else {
        repo.join(path)
    })
}
