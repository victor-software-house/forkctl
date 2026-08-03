use std::fs;
use std::process::Command;

fn forkctl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_forkctl"))
}

#[test]
fn instructions_work_outside_a_repository() {
    let directory = tempfile::tempdir().unwrap();
    let output = forkctl()
        .arg("instructions")
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("mise run fork:init"));
    assert!(stdout.contains("Do not put generic executable fork lifecycle logic"));
}

#[test]
fn version_reports_package_version() {
    let output = forkctl().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!("forkctl {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn invalid_manifest_fails_before_patch_commands() {
    let directory = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(directory.path())
            .status()
            .unwrap()
            .success()
    );
    let manifest = directory.path().join("fork.json");
    let invalid =
        include_str!("../examples/fork.json").replacen("\"schema\": 1", "\"schema\": 2", 1);
    fs::write(&manifest, invalid).unwrap();

    let output = forkctl()
        .args(["--manifest", manifest.to_str().unwrap(), "verify"])
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported manifest schema"), "{stderr}");
}
