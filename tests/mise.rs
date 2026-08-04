#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;
use support::Fixture;

#[test]
fn mounted_mise_proxy_preserves_help_output_exit_and_cwd() {
    let fixture = Fixture::new();
    let catalog = MountedCatalog::new(&fixture.repo);

    let mounted_help = catalog.mise(&["run", "--quiet", "fork", "--help"]);
    assert!(mounted_help.status.success());
    assert!(String::from_utf8_lossy(&mounted_help.stdout).contains("Commands"));
    assert!(String::from_utf8_lossy(&mounted_help.stdout).contains("patch"));

    let direct = fixture.api_call(
        "execute",
        &serde_json::json!({"command":"status","arguments":{}}),
    );
    let mounted = catalog.mise(&[
        "run",
        "--quiet",
        "fork",
        "--manifest",
        "patches/fork.yaml",
        "--format",
        "json",
        "status",
    ]);
    assert_eq!(mounted.status.code(), direct.status.code());
    assert_eq!(mounted.stdout, direct.stdout);
    assert_eq!(mounted.stderr, direct.stderr);
}

#[test]
fn mounted_mise_completion_covers_supported_shells_and_live_values() {
    let fixture = Fixture::new();
    let catalog = MountedCatalog::new(&fixture.repo);
    let spec = catalog.mise(&["usage"]);
    assert!(
        spec.status.success(),
        "mise usage failed: {}",
        String::from_utf8_lossy(&spec.stderr)
    );
    let spec_path = fixture.repo.join("mise-mounted.usage.kdl");
    fs::write(&spec_path, &spec.stdout).unwrap();

    for shell in ["bash", "fish", "nu", "powershell", "zsh"] {
        let commands =
            catalog.complete(&spec_path, shell, 4, &["mise", "run", "fork", "patch", ""]);
        assert!(
            commands.lines().any(|line| line.starts_with("refresh")),
            "{shell} omitted patch refresh: {commands:?}"
        );

        let patches = catalog.complete(
            &spec_path,
            shell,
            5,
            &["mise", "run", "fork", "patch", "show", "fork"],
        );
        assert!(
            patches.lines().any(|line| line.starts_with("fork-tooling")),
            "{shell} omitted live patch name: {patches:?}"
        );

        let refs = catalog.complete(
            &spec_path,
            shell,
            5,
            &["mise", "run", "fork", "rebase", "--onto", "refs/heads/m"],
        );
        assert!(
            refs.lines().any(|line| line.starts_with("refs/heads/main")),
            "{shell} omitted local ref: {refs:?}"
        );
    }
}

#[test]
fn lefthook_composes_with_mounted_read_only_checks() {
    let fixture = Fixture::new();
    let catalog = MountedCatalog::new(&fixture.repo);
    fs::write(
        fixture.repo.join("lefthook.yml"),
        "pre-commit:\n  commands:\n    forkctl-staged:\n      run: mise run --quiet fork check -s\npre-push:\n  commands:\n    forkctl-check:\n      run: mise run --quiet fork check -q\n",
    )
    .unwrap();
    support::git_ok(&fixture.repo, ["add", "mise.toml", "lefthook.yml"]);
    let refresh = support::isolated_command(&fixture.repo, "stg")
        .args(["refresh", "--patch", "fork-tooling", "--index"])
        .output()
        .unwrap();
    assert!(
        refresh.status.success(),
        "bookkeeping refresh failed: {}",
        String::from_utf8_lossy(&refresh.stderr)
    );

    let direct_check = fixture.forkctl(&["--format", "json", "check"]);
    assert!(
        direct_check.status.success(),
        "direct check failed before mounted hook:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&direct_check.stdout),
        String::from_utf8_lossy(&direct_check.stderr)
    );

    let pre_push = catalog.lefthook(&["run", "pre-push"]);
    assert!(
        pre_push.status.success(),
        "pre-push failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&pre_push.stdout),
        String::from_utf8_lossy(&pre_push.stderr)
    );

    fs::write(fixture.repo.join("outside.txt"), "outside\n").unwrap();
    support::git_ok(&fixture.repo, ["add", "outside.txt"]);
    let pre_commit = catalog.lefthook(&["run", "pre-commit"]);
    assert!(!pre_commit.status.success());
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&pre_commit.stdout),
        String::from_utf8_lossy(&pre_commit.stderr)
    );
    assert!(
        diagnostics.contains("active patch") || diagnostics.contains("outside patch"),
        "unexpected pre-commit diagnostics: {diagnostics}"
    );
}

struct MountedCatalog {
    repo: PathBuf,
    _tasks: tempfile::TempDir,
}

impl MountedCatalog {
    fn new(repo: &Path) -> Self {
        let tasks = tempfile::tempdir().unwrap();
        let task_path = tasks.path().join("fork");
        let source =
            fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tasks/fork/fork")).unwrap();
        let source = source
            .replace(
                &format!(
                    ", \"github:victor-software-house/forkctl\" = \"{}\"",
                    env!("CARGO_PKG_VERSION")
                ),
                "",
            )
            .replace(
                "exec \"$(mise where github:victor-software-house/forkctl)/forkctl\" \"$@\"",
                &format!("exec {} \"$@\"", env!("CARGO_BIN_EXE_forkctl")),
            );
        assert!(
            !source.contains("mise where github:victor-software-house/forkctl"),
            "mounted test task still resolves the published forkctl binary"
        );
        fs::write(&task_path, source).unwrap();
        let mut permissions = fs::metadata(&task_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&task_path, permissions).unwrap();
        let include = serde_json::to_string(&tasks.path().display().to_string()).unwrap();
        fs::write(
            repo.join("mise.toml"),
            format!("[settings]\nexperimental = true\n\n[task_config]\nincludes = [{include}]\n"),
        )
        .unwrap();
        Self {
            repo: repo.to_owned(),
            _tasks: tasks,
        }
    }

    fn mise(&self, args: &[&str]) -> Output {
        support::isolated_command(&self.repo, "mise")
            .args(args)
            .env("MISE_TRUSTED_CONFIG_PATHS", &self.repo)
            .current_dir(&self.repo)
            .output()
            .unwrap()
    }

    fn complete(&self, spec: &Path, shell: &str, cword: usize, words: &[&str]) -> String {
        let output = support::isolated_command(&self.repo, "usage")
            .args(["complete-word", "--file"])
            .arg(spec)
            .args(["--shell", shell, "--cword", &cword.to_string(), "--"])
            .args(words)
            .env("MISE_TRUSTED_CONFIG_PATHS", &self.repo)
            .current_dir(&self.repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "usage completion failed for {shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "usage completion warned for {shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn lefthook(&self, args: &[&str]) -> Output {
        support::isolated_command(&self.repo, "lefthook")
            .args(args)
            .env("MISE_TRUSTED_CONFIG_PATHS", &self.repo)
            .current_dir(&self.repo)
            .output()
            .unwrap()
    }
}
