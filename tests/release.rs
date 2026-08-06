#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

const HEAD: &str = "1111111111111111111111111111111111111111";

#[test]
fn finalized_exact_release_can_be_resumed() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let fake_bin = root.path().join("bin");
    let state = root.path().join("state");
    fs::create_dir_all(repo.join("mise-tasks")).unwrap();
    fs::create_dir_all(repo.join("target/release")).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::create_dir_all(&state).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("mise-tasks/release.sh"),
        repo.join("mise-tasks/release.sh"),
    )
    .unwrap();

    write_executable(
        &repo.join("target/release/forkctl"),
        "#!/bin/sh\nprintf 'forkctl 0.0.19\\n'\n",
    );
    write_executable(
        &fake_bin.join("git"),
        &format!(
            "#!/bin/sh\ncase \"$1 $2\" in\n  'status --porcelain') exit 0 ;;\n  'branch --show-current') printf 'main\\n' ;;\n  'fetch origin') exit 0 ;;\n  'rev-parse HEAD'|'rev-parse origin/main') printf '{HEAD}\\n' ;;\n  *) exit 97 ;;\nesac\n"
        ),
    );
    write_executable(
        &fake_bin.join("cargo"),
        "#!/bin/sh\ncase \"$1\" in\n  build|info) exit 0 ;;\n  *) exit 97 ;;\nesac\n",
    );
    write_executable(
        &fake_bin.join("uname"),
        "#!/bin/sh\ncase \"$1\" in\n  -s) printf 'Darwin\\n' ;;\n  -m) printf 'arm64\\n' ;;\n  *) exit 97 ;;\nesac\n",
    );
    write_executable(
        &fake_bin.join("gh"),
        &format!(
            "#!/bin/sh\ncase \"$1 $2\" in\n  'repo view') printf 'victor-software-house/forkctl\\n' ;;\n  'release view')\n    case \" $* \" in\n      *' --json isDraft '*) printf 'false\\n' ;;\n      *) exit 0 ;;\n    esac ;;\n  'release upload') cp \"$4\" \"$STATE_DIR/asset\" ;;\n  'release download')\n    while [ \"$#\" -gt 0 ]; do\n      if [ \"$1\" = --dir ]; then directory=$2; break; fi\n      shift\n    done\n    cp \"$STATE_DIR/asset\" \"$directory/forkctl_0.0.19_macos_arm64.tar.gz\" ;;\n  'release edit') touch \"$STATE_DIR/unexpected-edit\"; exit 98 ;;\n  'api repos/victor-software-house/forkctl/commits/v0.0.19') printf '{HEAD}\\n' ;;\n  *) exit 97 ;;\nesac\n"
        ),
    );

    let path = format!("{}:{}", fake_bin.display(), std::env::var("PATH").unwrap());
    let output = Command::new("sh")
        .arg("mise-tasks/release.sh")
        .current_dir(&repo)
        .env("PATH", path)
        .env("STATE_DIR", &state)
        .env("TMPDIR", root.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!state.join("unexpected-edit").exists());
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}
