mod support;

use serde_json::Value;
use std::fs;
use std::process::Command;
use support::{Fixture, git_capture, git_ok, stg_capture, stg_ok};

#[test]
fn initializes_and_reports_a_tooling_only_clone() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&["verify"]);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    assert_eq!(status["verification"]["ok"], true);
    assert_eq!(
        status["applied_patches"],
        serde_json::json!(["fork-tooling"])
    );
    assert_eq!(status["exports"], serde_json::json!([]));
    assert!(status["pending"].is_null());

    let head = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    fixture.forkctl_ok(&["init"]);
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), head);

    fs::write(fixture.repo.join("dirty.txt"), "dirty\n").unwrap();
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("worktree is not clean"));
    let dirty_status: Value =
        serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    assert_eq!(dirty_status["verification"]["ok"], false);
    assert!(!dirty_status["dirty"].as_array().unwrap().is_empty());
}

#[test]
fn creates_implements_and_publishes_with_an_exact_lease() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&[
        "new",
        "source-change",
        "--kind",
        "source",
        "--purpose",
        "Add downstream source behavior.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream adds the behavior.",
        "--path",
        "source.txt",
    ]);
    let incomplete = fixture.forkctl(&["verify"]);
    assert!(!incomplete.status.success());
    assert!(String::from_utf8_lossy(&incomplete.stderr).contains("patch source-change is empty"));

    fixture.implement_patch("source-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&["verify"]);
    let old_remote = git_capture(&fixture.repo, ["ls-remote", "origin", "refs/heads/main"]);
    fixture.forkctl_ok(&["publish"]);
    let new_remote = git_capture(&fixture.repo, ["ls-remote", "origin", "refs/heads/main"]);
    assert_ne!(old_remote, new_remote);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    assert!(status["pending"].is_null());
}

#[test]
fn finish_new_generates_exports_and_detects_drift() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&[
        "new",
        "source-change",
        "--kind",
        "source",
        "--purpose",
        "Add exported downstream source behavior.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream adds the behavior.",
        "--path",
        "source.txt",
        "--export",
        "patches/source-change.patch",
    ]);
    fixture.implement_patch("source-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&["new", "--finish"]);
    assert!(fixture.repo.join("patches/source-change.patch").is_file());
    fixture.forkctl_ok(&["verify"]);

    fs::write(
        fixture.repo.join("patches/source-change.patch"),
        "corrupt\n",
    )
    .unwrap();
    git_ok(&fixture.repo, ["add", "patches/source-change.patch"]);
    stg_ok(&fixture.repo, ["refresh", "--index"]);
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("export differs"));
}

#[test]
fn publish_rejects_a_concurrently_advanced_remote() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&[
        "new",
        "source-change",
        "--kind",
        "source",
        "--purpose",
        "Add downstream source behavior.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream adds the behavior.",
        "--path",
        "source.txt",
    ]);
    fixture.implement_patch("source-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&["verify"]);

    let other = fixture.repo.parent().unwrap().join("other");
    git_ok(
        fixture.repo.parent().unwrap(),
        [
            "clone",
            "--quiet",
            fixture.downstream_bare.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    git_ok(&other, ["config", "user.name", "Other Writer"]);
    git_ok(&other, ["config", "user.email", "other@example.com"]);
    fs::write(other.join("other.txt"), "advanced\n").unwrap();
    git_ok(&other, ["add", "other.txt"]);
    git_ok(&other, ["commit", "--quiet", "-m", "advance remote"]);
    git_ok(&other, ["push", "--quiet", "origin", "main"]);

    let before = git_capture(&fixture.repo, ["ls-remote", "origin", "refs/heads/main"]);
    let output = fixture.forkctl(&["publish"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("downstream advanced"));
    let after = git_capture(&fixture.repo, ["ls-remote", "origin", "refs/heads/main"]);
    assert_eq!(before, after);
}

#[test]
fn changed_and_no_op_rebases_preserve_review_evidence() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    let v2 = fixture.advance_upstream("v2", "upstream v2\n");
    fixture.forkctl_ok(&["rebase", "--onto", "refs/tags/v2"]);
    assert_eq!(stg_capture(&fixture.repo, ["id", "{base}"]), v2);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    let report = status["pending"]["report"].as_str().unwrap();
    let report_text = fs::read_to_string(report).unwrap();
    assert!(report_text.contains("## Range diff"));
    assert!(report_text.contains("Structural verification: passed"));
    fixture.forkctl_ok(&["publish"]);

    let head = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    fixture.forkctl_ok(&["rebase", "--onto", "refs/tags/v2"]);
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), head);
    fixture.forkctl_ok(&["publish"]);

    fixture.forkctl_ok(&["rebase", "--onto", "refs/heads/main"]);
    assert_eq!(stg_capture(&fixture.repo, ["id", "{base}"]), v2);
    fixture.forkctl_ok(&["publish"]);

    fixture.forkctl_ok(&["rebase", "--onto", &v2]);
    assert_eq!(stg_capture(&fixture.repo, ["id", "{base}"]), v2);
}

#[test]
fn rebase_conflict_preserves_pending_state_and_recovery_tag() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&[
        "new",
        "source-change",
        "--kind",
        "source",
        "--purpose",
        "Change the shared base file.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream provides the downstream behavior.",
        "--path",
        "base.txt",
    ]);
    fixture.implement_patch("source-change", "base.txt", "downstream\n");
    fixture.forkctl_ok(&["verify"]);
    fixture.forkctl_ok(&["publish"]);

    fs::write(fixture.upstream_work.join("base.txt"), "upstream\n").unwrap();
    git_ok(&fixture.upstream_work, ["add", "base.txt"]);
    git_ok(
        &fixture.upstream_work,
        ["commit", "--quiet", "-m", "conflicting upstream"],
    );
    git_ok(
        &fixture.upstream_work,
        ["tag", "--annotate", "--message", "v2", "v2"],
    );
    git_ok(
        &fixture.upstream_work,
        ["push", "--quiet", "upstream-bare", "main", "refs/tags/v2"],
    );

    let output = fixture.forkctl(&["rebase", "--onto", "refs/tags/v2"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("rebase stopped"));
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    let backup = status["pending"]["backup_tag"].as_str().unwrap();
    assert_eq!(
        git_capture(&fixture.repo, ["tag", "--list", backup]),
        backup
    );
    let remote_tag = Command::new("git")
        .args(["ls-remote", "origin", &format!("refs/tags/{backup}")])
        .current_dir(&fixture.repo)
        .output()
        .unwrap();
    assert!(remote_tag.status.success());
    assert!(remote_tag.stdout.is_empty());

    fs::write(fixture.repo.join("base.txt"), "resolved\n").unwrap();
    git_ok(&fixture.repo, ["add", "base.txt"]);
    stg_ok(&fixture.repo, ["refresh"]);
    stg_ok(&fixture.repo, ["goto", "fork-tooling"]);
    fixture.forkctl_ok(&["rebase", "--onto", "refs/tags/v2"]);
    fixture.forkctl_ok(&["verify"]);
}

#[test]
fn undeclared_patch_path_fails_closed() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fs::write(fixture.repo.join("undeclared.txt"), "drift\n").unwrap();
    git_ok(&fixture.repo, ["add", "undeclared.txt"]);
    stg_ok(&fixture.repo, ["refresh", "--index"]);
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("undeclared path"));
}

#[test]
fn trailer_drift_fails_closed() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    stg_ok(
        &fixture.repo,
        [
            "edit",
            "--message",
            "fork-tooling\n\nDownstream-Reason: Wrong reason.\nUpstream-Status: inappropriate: downstream-only tooling\nDrop-When: The downstream fork is retired.",
            "fork-tooling",
        ],
    );
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Downstream-Reason"));
}

#[test]
fn ledger_and_patch_path_drift_fail_closed() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fs::write(fixture.repo.join("PATCHES.md"), "drift\n").unwrap();
    stg_ok(&fixture.repo, ["refresh"]);
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("generated ledger differs"));
}

#[test]
fn wrong_branch_fails_before_mutation() {
    let fixture = Fixture::tooling_only();
    git_ok(&fixture.repo, ["switch", "--quiet", "-c", "other"]);
    let output = fixture.forkctl(&["init"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("current branch is other"));
    assert!(
        Command::new("git")
            .args(["tag", "--list", "vsh/pre-sync-*"])
            .current_dir(&fixture.repo)
            .output()
            .unwrap()
            .stdout
            .is_empty()
    );
}
