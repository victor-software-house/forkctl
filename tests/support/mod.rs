use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub struct Fixture {
    _directory: tempfile::TempDir,
    pub upstream_work: PathBuf,
    pub downstream_bare: PathBuf,
    pub repo: PathBuf,
}

impl Fixture {
    pub fn tooling_only() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let upstream_bare = directory.path().join("upstream.git");
        let downstream_bare = directory.path().join("downstream.git");
        init_bare(directory.path(), &upstream_bare);
        init_bare(directory.path(), &downstream_bare);

        let upstream_work = directory.path().join("upstream-work");
        let base = create_upstream(directory.path(), &upstream_work, &upstream_bare);
        create_tooling_commit(&upstream_work, &upstream_bare, &downstream_bare, &base);

        let repo = directory.path().join("consumer");
        git_ok(
            directory.path(),
            [
                "clone",
                "--quiet",
                downstream_bare.to_str().unwrap(),
                repo.to_str().unwrap(),
            ],
        );
        configure_identity(&repo);

        Self {
            _directory: directory,
            upstream_work,
            downstream_bare,
            repo,
        }
    }

    pub fn forkctl(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_forkctl"))
            .arg("--manifest")
            .arg("fork.json")
            .args(args)
            .current_dir(&self.repo)
            .output()
            .unwrap()
    }

    pub fn forkctl_ok(&self, args: &[&str]) -> String {
        let output = self.forkctl(args);
        assert!(
            output.status.success(),
            "{} {args:?} failed:\nstdout: {}\nstderr: {}",
            env!("CARGO_BIN_EXE_forkctl"),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn advance_upstream(&self, tag: &str, contents: &str) -> String {
        fs::write(self.upstream_work.join("upstream.txt"), contents).unwrap();
        git_ok(&self.upstream_work, ["add", "upstream.txt"]);
        git_ok(&self.upstream_work, ["commit", "--quiet", "-m", tag]);
        git_ok(
            &self.upstream_work,
            ["tag", "--annotate", "--message", tag, tag],
        );
        git_ok(
            &self.upstream_work,
            [
                "push",
                "--quiet",
                "upstream-bare",
                "main",
                &format!("refs/tags/{tag}"),
            ],
        );
        git_capture(&self.upstream_work, ["rev-parse", "HEAD"])
    }

    pub fn implement_patch(&self, name: &str, path: &str, contents: &str) {
        stg_ok(&self.repo, ["goto", name]);
        fs::write(self.repo.join(path), contents).unwrap();
        git_ok(&self.repo, ["add", path]);
        stg_ok(&self.repo, ["refresh", "--index"]);
        stg_ok(&self.repo, ["push", "--all"]);
    }
}

fn init_bare(directory: &Path, path: &Path) {
    git_ok(
        directory,
        ["init", "--bare", "--quiet", path.to_str().unwrap()],
    );
    git_ok(
        directory,
        [
            "--git-dir",
            path.to_str().unwrap(),
            "symbolic-ref",
            "HEAD",
            "refs/heads/main",
        ],
    );
}

fn create_upstream(directory: &Path, work: &Path, bare: &Path) -> String {
    git_ok(
        directory,
        [
            "init",
            "--quiet",
            "--initial-branch=main",
            work.to_str().unwrap(),
        ],
    );
    configure_identity(work);
    fs::write(work.join("base.txt"), "base\n").unwrap();
    git_ok(work, ["add", "base.txt"]);
    git_ok(work, ["commit", "--quiet", "-m", "base"]);
    git_ok(work, ["tag", "--annotate", "--message", "v1", "v1"]);
    git_ok(
        work,
        ["remote", "add", "upstream-bare", bare.to_str().unwrap()],
    );
    git_ok(
        work,
        ["push", "--quiet", "upstream-bare", "main", "refs/tags/v1"],
    );
    git_capture(work, ["rev-parse", "HEAD"])
}

fn create_tooling_commit(work: &Path, upstream: &Path, downstream: &Path, base: &str) {
    fs::write(
        work.join("FORK.md"),
        "Run `mise run fork:verify` before publication.\n",
    )
    .unwrap();
    let manifest = json!({
        "schema": 1,
        "downstream": {
            "remote": "origin",
            "branch": "main",
            "backup_tag_prefix": "vsh/pre-sync"
        },
        "upstream": {
            "remote": "upstream",
            "url": upstream.to_string_lossy(),
            "fetch_ref": "refs/heads/main"
        },
        "base": {
            "label": "refs/tags/v1",
            "canonical": base,
            "stack": base
        },
        "ledger": "PATCHES.md",
        "bookkeeping_patch": "fork-tooling",
        "patches": [{
            "name": "fork-tooling",
            "kind": "tooling",
            "purpose": "Own downstream fork policy and bookkeeping.",
            "upstream_status": "inappropriate: downstream-only tooling",
            "drop_when": "The downstream fork is retired.",
            "paths": ["fork.json", "PATCHES.md", "FORK.md", "patches/*"]
        }],
        "allow": {"base": []},
        "required": [{"path": "FORK.md", "contains": "mise run fork:verify"}]
    });
    fs::write(
        work.join("fork.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();
    fs::write(
        work.join("PATCHES.md"),
        ledger(
            "refs/tags/v1",
            base,
            &[LedgerPatch {
                name: "fork-tooling",
                kind: "tooling",
                purpose: "Own downstream fork policy and bookkeeping.",
                upstream_status: "inappropriate: downstream-only tooling",
                drop_when: "The downstream fork is retired.",
            }],
        ),
    )
    .unwrap();
    git_ok(work, ["add", "FORK.md", "fork.json", "PATCHES.md"]);
    git_ok(
        work,
        [
            "commit",
            "--quiet",
            "-m",
            "fork-tooling\n\nDownstream-Reason: Own downstream fork policy and bookkeeping.\nUpstream-Status: inappropriate: downstream-only tooling\nDrop-When: The downstream fork is retired.",
        ],
    );
    git_ok(
        work,
        ["remote", "add", "downstream", downstream.to_str().unwrap()],
    );
    git_ok(work, ["push", "--quiet", "downstream", "HEAD:main"]);
    git_ok(work, ["reset", "--hard", "--quiet", base]);
}

pub fn git_ok<const N: usize>(dir: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn stg_ok<const N: usize>(dir: &Path, args: [&str; N]) {
    let output = Command::new("stg")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stg failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn stg_capture<const N: usize>(dir: &Path, args: [&str; N]) -> String {
    let output = Command::new("stg")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

pub fn git_capture<const N: usize>(dir: &Path, args: [&str; N]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn configure_identity(repo: &Path) {
    git_ok(repo, ["config", "user.name", "Forkctl Test"]);
    git_ok(repo, ["config", "user.email", "forkctl@example.com"]);
}

struct LedgerPatch<'a> {
    name: &'a str,
    kind: &'a str,
    purpose: &'a str,
    upstream_status: &'a str,
    drop_when: &'a str,
}

fn ledger(label: &str, base: &str, patches: &[LedgerPatch<'_>]) -> String {
    let mut output = format!(
        "# Downstream Patches\n\n> Generated by `forkctl`; edit the manifest, not this file.\n\nBase: `{label}` (`{base}`)\n\n| Order | Patch | Kind | Purpose | Upstream status | Drop condition |\n|--:|:--|:--|:--|:--|:--|\n"
    );
    for (index, patch) in patches.iter().enumerate() {
        output.push_str("| ");
        output.push_str(&(index + 1).to_string());
        output.push_str(" | `");
        output.push_str(patch.name);
        output.push_str("` | ");
        output.push_str(patch.kind);
        output.push_str(" | ");
        output.push_str(patch.purpose);
        output.push_str(" | ");
        output.push_str(patch.upstream_status);
        output.push_str(" | ");
        output.push_str(patch.drop_when);
        output.push_str(" |\n");
    }
    output
}
