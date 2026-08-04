use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const SANDBOX_DIR: &str = ".forkctl-test-env";
const FIXED_GIT_DATE: &str = "2001-02-03T04:05:06Z";

pub struct Fixture {
    _directory: tempfile::TempDir,
    pub repo: PathBuf,
    manifest: String,
}

impl Fixture {
    pub fn new() -> Self {
        Self::new_with_manifest("patches/fork.yaml")
    }

    pub fn new_with_manifest(manifest: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        prepare_sandbox(directory.path());
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
        git_ok(
            &repo,
            ["remote", "add", "upstream", upstream_bare.to_str().unwrap()],
        );
        let fixture = Self {
            _directory: directory,
            repo,
            manifest: manifest.to_string(),
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
        forkctl_command(&self.repo)
            .arg("--manifest")
            .arg(&self.manifest)
            .args(args)
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
            "manifest": self.manifest,
            "mode": mode,
            "request": request,
        });
        let mut child = forkctl_command(&self.repo)
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
    fs::write(work.join("base.txt"), "base\n").unwrap();
    git_ok(work, ["add", "base.txt"]);
    git_ok(work, ["commit", "--quiet", "-m", "base"]);
    git_ok(work, ["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(work, ["push", "--quiet", "-u", "origin", "main"]);
}

pub fn forkctl_command(dir: &Path) -> Command {
    sandboxed_command(dir, env!("CARGO_BIN_EXE_forkctl"), false)
}

pub fn isolated_command(dir: &Path, program: impl AsRef<OsStr>) -> Command {
    sandboxed_command(dir, program, true)
}

fn sandboxed_command(dir: &Path, program: impl AsRef<OsStr>, clear_git_context: bool) -> Command {
    let sandbox = sandbox_path(dir);
    let mise_data_dir = std::env::var_os("MISE_DATA_DIR").map_or_else(
        || {
            PathBuf::from(std::env::var_os("HOME").expect("test runner HOME is required"))
                .join(".local/share/mise")
        },
        PathBuf::from,
    );
    let mut command = Command::new(program);
    if clear_git_context {
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
    }
    command
        .current_dir(dir)
        .env("HOME", sandbox.join("home"))
        .env("XDG_CONFIG_HOME", sandbox.join("xdg-config"))
        .env("XDG_CACHE_HOME", sandbox.join("xdg-cache"))
        .env("XDG_DATA_HOME", sandbox.join("xdg-data"))
        .env("MISE_DATA_DIR", mise_data_dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", sandbox.join("git-global-config"))
        .env("GIT_CONFIG_SYSTEM", sandbox.join("git-system-config"))
        .env("GIT_TEMPLATE_DIR", sandbox.join("git-template"))
        .env("GIT_AUTHOR_NAME", "Forkctl Test")
        .env("GIT_AUTHOR_EMAIL", "forkctl@example.com")
        .env("GIT_COMMITTER_NAME", "Forkctl Test")
        .env("GIT_COMMITTER_EMAIL", "forkctl@example.com")
        .env("GIT_AUTHOR_DATE", FIXED_GIT_DATE)
        .env("GIT_COMMITTER_DATE", FIXED_GIT_DATE)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("TZ", "UTC")
        .env("NO_COLOR", "1");
    command
}

fn prepare_sandbox(root: &Path) {
    let sandbox = root.join(SANDBOX_DIR);
    for directory in [
        "home",
        "xdg-config",
        "xdg-cache",
        "xdg-data",
        "git-template",
    ] {
        fs::create_dir_all(sandbox.join(directory)).unwrap();
    }
    fs::write(sandbox.join("git-global-config"), "").unwrap();
    fs::write(sandbox.join("git-system-config"), "").unwrap();
}

fn sandbox_path(dir: &Path) -> PathBuf {
    dir.ancestors()
        .map(|ancestor| ancestor.join(SANDBOX_DIR))
        .find(|candidate| candidate.is_dir())
        .unwrap_or_else(|| panic!("no forkctl test sandbox above {}", dir.display()))
}

pub fn git_ok<const N: usize>(dir: &Path, args: [&str; N]) {
    let output = isolated_command(dir, "git")
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
