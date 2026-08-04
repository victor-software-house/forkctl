use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub struct Fixture {
    _directory: tempfile::TempDir,
    pub repo: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let upstream_bare = directory.path().join("upstream.git");
        let downstream_bare = directory.path().join("downstream.git");
        init_bare(directory.path(), &upstream_bare);
        let upstream_work = directory.path().join("upstream-work");
        create_upstream(directory.path(), &upstream_work, &upstream_bare);
        git_ok(
            directory.path(),
            [
                "clone",
                "--bare",
                "--quiet",
                upstream_bare.to_str().unwrap(),
                downstream_bare.to_str().unwrap(),
            ],
        );
        git_ok(
            directory.path(),
            [
                "--git-dir",
                downstream_bare.to_str().unwrap(),
                "symbolic-ref",
                "HEAD",
                "refs/heads/main",
            ],
        );
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
        git_ok(
            &repo,
            ["remote", "add", "upstream", upstream_bare.to_str().unwrap()],
        );
        let fixture = Self {
            _directory: directory,
            repo,
        };
        fixture.forkctl_ok(&[
            "init",
            "--upstream-remote",
            "upstream",
            "--upstream-url",
            upstream_bare.to_str().unwrap(),
            "--upstream-ref",
            "refs/heads/main",
            "--downstream-remote",
            "origin",
            "--downstream-branch",
            "main",
            "--base",
            "refs/heads/main",
            "--ledger",
            "PATCHES.md",
            "--exports",
            "patches/downstream",
            "--bookkeeping-patch",
            "fork-tooling",
            "--bookkeeping-path",
            "FORK.md",
            "--bookkeeping-path",
            "mise.toml",
            "--bookkeeping-path",
            "lefthook.yml",
            "--required-text",
            "base.txt=base",
        ]);
        fixture
    }

    pub fn forkctl(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_forkctl"))
            .arg("--manifest")
            .arg("patches/fork.json")
            .args(args)
            .current_dir(&self.repo)
            .output()
            .unwrap()
    }

    pub fn forkctl_ok(&self, args: &[&str]) -> String {
        let output = self.forkctl(args);
        assert!(
            output.status.success(),
            "forkctl {args:?} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn api_call(&self, mode: &str, request: &serde_json::Value) -> Output {
        let invocation = serde_json::json!({
            "protocol_version": 1,
            "manifest": "patches/fork.json",
            "mode": mode,
            "request": request,
        });
        let mut child = Command::new(env!("CARGO_BIN_EXE_forkctl"))
            .args(["api", "call"])
            .current_dir(&self.repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_string(&invocation).unwrap().as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
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

fn create_upstream(directory: &Path, work: &Path, bare: &Path) {
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
    git_ok(work, ["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(work, ["push", "--quiet", "-u", "origin", "main"]);
}

fn isolated_command(program: &str) -> Command {
    let mut command = Command::new(program);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
}

pub fn git_ok<const N: usize>(dir: &Path, args: [&str; N]) {
    let output = isolated_command("git")
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

fn configure_identity(repo: &Path) {
    git_ok(repo, ["config", "user.name", "Forkctl Test"]);
    git_ok(repo, ["config", "user.email", "forkctl@example.com"]);
}
