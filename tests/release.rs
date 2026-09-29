#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const HEAD: &str = "1111111111111111111111111111111111111111";
const OTHER: &str = "2222222222222222222222222222222222222222";

#[test]
fn release_pushes_the_version_tag_at_the_pushed_main_commit() {
    let fixture = ReleaseFixture::new("");
    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(fixture.state.join("tag")).unwrap(),
        "tag --annotate v0.0.19 --message forkctl 0.0.19 1111111111111111111111111111111111111111\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.state.join("push")).unwrap(),
        "push origin refs/tags/v0.0.19\n"
    );
}

#[test]
fn release_resumes_when_the_tag_already_marks_the_same_commit() {
    let fixture = ReleaseFixture::new(HEAD);
    let output = fixture.run();
    assert_success(&output);
    assert!(!fixture.state.join("tag").exists());
    assert!(!fixture.state.join("push").exists());
}

#[test]
fn release_refuses_a_tag_on_another_commit() {
    let fixture = ReleaseFixture::new(OTHER);
    let output = fixture.run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already tags another commit"));
    assert!(!fixture.state.join("push").exists());
}

struct ReleaseFixture {
    _root: tempfile::TempDir,
    repo: PathBuf,
    fake_bin: PathBuf,
    state: PathBuf,
}

impl ReleaseFixture {
    /// `remote_tag` is the commit the remote tag already peels to, or empty when absent.
    fn new(remote_tag: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        let fake_bin = root.path().join("bin");
        let state = root.path().join("state");
        fs::create_dir_all(repo.join("mise-tasks")).unwrap();
        fs::create_dir_all(&fake_bin).unwrap();
        fs::create_dir_all(&state).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("mise-tasks/release.sh"),
            repo.join("mise-tasks/release.sh"),
        )
        .unwrap();
        fs::write(
            repo.join("Cargo.toml"),
            "[workspace.package]\nversion = \"0.0.19\"\n",
        )
        .unwrap();
        let remote_line = if remote_tag.is_empty() {
            String::new()
        } else {
            format!("printf '{remote_tag}\\trefs/tags/v0.0.19^{{}}\\n'")
        };
        write_executable(
            &fake_bin.join("git"),
            &format!(
                "#!/bin/sh\ncase \"$1 $2\" in\n  'status --porcelain') exit 0 ;;\n  'branch --show-current') printf 'main\\n' ;;\n  'fetch origin') exit 0 ;;\n  'rev-parse HEAD'|'rev-parse origin/main') printf '{HEAD}\\n' ;;\n  'ls-remote origin') {remote_line} ;;\n  'tag --annotate') echo \"$*\" > \"$STATE_DIR/tag\" ;;\n  'push origin') echo \"$*\" > \"$STATE_DIR/push\" ;;\n  *) exit 97 ;;\nesac\n"
            ),
        );
        Self {
            _root: root,
            repo,
            fake_bin,
            state,
        }
    }

    fn run(&self) -> Output {
        let path = format!(
            "{}:{}",
            self.fake_bin.display(),
            std::env::var("PATH").unwrap()
        );
        Command::new("sh")
            .arg("mise-tasks/release.sh")
            .current_dir(&self.repo)
            .env("PATH", path)
            .env("STATE_DIR", &self.state)
            .output()
            .unwrap()
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}
