mod support;

use serde_json::Value;
use std::fs;
use support::{Fixture, git_capture, git_ok, isolated_command, stg_capture, stg_ok};

fn command_data(response: &Value) -> &Value {
    &response["result"]["data"]
}

#[test]
fn initializes_and_reports_a_tooling_only_clone() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&["verify"]);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    assert_eq!(command_data(&status)["verification"]["ok"], true);
    assert_eq!(
        command_data(&status)["applied_patches"],
        serde_json::json!(["fork-tooling"])
    );
    assert_eq!(command_data(&status)["exports"], serde_json::json!([]));
    assert!(command_data(&status)["pending"].is_null());

    let head = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    fixture.forkctl_ok(&["init"]);
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), head);

    fs::write(fixture.repo.join("dirty.txt"), "dirty\n").unwrap();
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("worktree is not clean"));
    let dirty_status: Value =
        serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    assert_eq!(command_data(&dirty_status)["verification"]["ok"], false);
    assert!(
        !command_data(&dirty_status)["dirty"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn json_api_executes_real_status_without_terminal_output_leaks() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    let output = fixture.api_call(&serde_json::json!({"command": "status"}));
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "success");
    assert_eq!(response["result"]["command"], "status");
    assert_eq!(command_data(&response)["verification"]["ok"], true);
    assert_eq!(
        command_data(&response)["applied_patches"],
        serde_json::json!(["fork-tooling"])
    );
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
    fixture.forkctl_ok(&["new", "--finish"]);
    fixture.forkctl_ok(&["verify"]);
    let old_remote = git_capture(&fixture.repo, ["ls-remote", "origin", "refs/heads/main"]);
    fixture.forkctl_ok(&["publish"]);
    let new_remote = git_capture(&fixture.repo, ["ls-remote", "origin", "refs/heads/main"]);
    assert_ne!(old_remote, new_remote);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    assert!(command_data(&status)["pending"].is_null());
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
    fixture.forkctl_ok(&["new", "--finish"]);
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
    let report = command_data(&status)["pending"]["report"]["path"]
        .as_str()
        .unwrap();
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
    fixture.forkctl_ok(&["new", "--finish"]);
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
    let backup = command_data(&status)["pending"]["backup_tag"]
        .as_str()
        .unwrap();
    assert_eq!(
        git_capture(&fixture.repo, ["tag", "--list", backup]),
        backup
    );
    let remote_tag = isolated_command("git")
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
fn retargeted_recovery_tag_is_rejected() {
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
    fixture.forkctl_ok(&["new", "--finish"]);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    let backup = command_data(&status)["pending"]["backup_tag"]
        .as_str()
        .unwrap();
    git_ok(
        &fixture.repo,
        [
            "tag",
            "--force",
            "--annotate",
            "--message",
            "retargeted",
            backup,
            "HEAD",
        ],
    );
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("backup tag resolves"));
}

#[test]
fn modified_rebase_report_is_rejected() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.advance_upstream("v2", "upstream v2\n");
    fixture.forkctl_ok(&["rebase", "--onto", "refs/tags/v2"]);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    let report = command_data(&status)["pending"]["report"]["path"]
        .as_str()
        .unwrap();
    fs::write(report, "modified after review\n").unwrap();
    let output = fixture.forkctl(&["publish"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("rebase report differs"));
}

#[test]
fn invalid_target_fails_before_recovery_state() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    let output = fixture.forkctl(&["rebase", "--onto", "v2"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("target must be a full"));
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    assert!(command_data(&status)["pending"].is_null());
    assert!(git_capture(&fixture.repo, ["tag", "--list", "vsh/pre-sync-*"]).is_empty());
}

#[test]
fn upstream_merged_patch_is_dropped_into_history() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&[
        "new",
        "merged-change",
        "--kind",
        "source",
        "--purpose",
        "Add behavior later provided upstream.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream adds the behavior.",
        "--path",
        "merged.txt",
    ]);
    fixture.implement_patch("merged-change", "merged.txt", "merged\n");
    fixture.forkctl_ok(&["new", "--finish"]);
    fixture.forkctl_ok(&["publish"]);

    fs::write(fixture.upstream_work.join("merged.txt"), "merged\n").unwrap();
    git_ok(&fixture.upstream_work, ["add", "merged.txt"]);
    git_ok(
        &fixture.upstream_work,
        ["commit", "--quiet", "-m", "merge downstream behavior"],
    );
    git_ok(
        &fixture.upstream_work,
        ["tag", "--annotate", "--message", "v2", "v2"],
    );
    git_ok(
        &fixture.upstream_work,
        ["push", "--quiet", "upstream-bare", "main", "refs/tags/v2"],
    );

    let output = fixture.forkctl_ok(&["rebase", "--onto", "refs/tags/v2"]);
    assert!(output.contains("dropped upstream-merged patch merged-change"));
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]),
        "fork-tooling"
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(fixture.repo.join("fork.json")).unwrap()).unwrap();
    assert_eq!(manifest["history"][0]["patch"]["name"], "merged-change");
    assert_eq!(manifest["history"][0]["kind"], "upstream-merged");
    let history_commit = manifest["history"][0]["commit"]
        .as_str()
        .unwrap()
        .to_string();
    let ledger = fs::read_to_string(fixture.repo.join("PATCHES.md")).unwrap();
    assert!(ledger.contains("upstream merged"));
    assert!(ledger.contains("merged-change"));
    fixture.forkctl_ok(&["verify"]);
    fixture.forkctl_ok(&["publish"]);

    let clone = fixture.repo.parent().unwrap().join("history-reader");
    git_ok(
        fixture.repo.parent().unwrap(),
        [
            "clone",
            "--quiet",
            fixture.downstream_bare.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let init = isolated_command(env!("CARGO_BIN_EXE_forkctl"))
        .args(["--manifest", "fork.json", "init"])
        .current_dir(&clone)
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "fresh-clone init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    git_ok(
        &clone,
        ["cat-file", "-e", &format!("{history_commit}^{{commit}}")],
    );
}

#[test]
fn pending_old_stack_boundary_mismatch_is_rejected() {
    let fixture = finished_new_fixture();
    mutate_pending(&fixture.repo, |pending| {
        pending["old_base"] = Value::String("0".repeat(40));
    });
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pending old stack"));
}

#[test]
fn pending_old_patch_name_mismatch_is_rejected() {
    let fixture = finished_new_fixture();
    mutate_pending(&fixture.repo, |pending| {
        pending["old_patches"][0]["name"] = Value::String("different-patch".into());
    });
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("old patch names"));
}

#[test]
fn pending_new_base_mismatch_is_rejected() {
    let fixture = finished_new_fixture();
    mutate_pending(&fixture.repo, |pending| {
        pending["new_base"] = Value::String("0".repeat(40));
    });
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pending new base"));
}

#[test]
fn pending_rebase_target_mismatch_is_rejected() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.advance_upstream("v2", "upstream v2\n");
    fixture.forkctl_ok(&["rebase", "--onto", "refs/tags/v2"]);
    mutate_pending(&fixture.repo, |pending| {
        pending["target"]["selector"] = Value::String("refs/tags/different".into());
    });
    let output = fixture.forkctl(&["verify"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pending target"));
}

fn finished_new_fixture() -> Fixture {
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
    fixture.forkctl_ok(&["new", "--finish"]);
    fixture
}

fn mutate_pending(repo: &std::path::Path, mutate: impl FnOnce(&mut Value)) {
    let relative = git_capture(repo, ["rev-parse", "--git-path", "forkctl/pending.json"]);
    let candidate = std::path::PathBuf::from(relative);
    let path = if candidate.is_absolute() {
        candidate
    } else {
        repo.join(candidate)
    };
    let mut pending: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    mutate(&mut pending);
    fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&pending).unwrap()),
    )
    .unwrap();
}

#[test]
fn conflicting_remote_recovery_tag_blocks_publish() {
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
    fixture.forkctl_ok(&["new", "--finish"]);
    let status: Value = serde_json::from_str(&fixture.forkctl_ok(&["status", "--json"])).unwrap();
    let backup = command_data(&status)["pending"]["backup_tag"]
        .as_str()
        .unwrap();

    let other = fixture.repo.parent().unwrap().join("tag-writer");
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
    git_ok(
        &other,
        [
            "tag",
            "--annotate",
            "--message",
            "conflicting",
            backup,
            "HEAD",
        ],
    );
    git_ok(
        &other,
        ["push", "--quiet", "origin", &format!("refs/tags/{backup}")],
    );
    let output = fixture.forkctl(&["publish"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("remote backup tag"));
}

#[test]
fn tooling_patch_is_inserted_before_bookkeeping_with_exact_trailers() {
    let fixture = Fixture::tooling_only();
    fixture.forkctl_ok(&["init"]);
    fixture.forkctl_ok(&[
        "new",
        "build-tooling",
        "--kind",
        "tooling",
        "--purpose",
        "Add downstream build tooling.",
        "--upstream-status",
        "inappropriate: downstream-only tooling",
        "--drop-when",
        "The downstream build is retired.",
        "--path",
        "BUILD.md",
    ]);
    fixture.implement_patch("build-tooling", "BUILD.md", "build\n");
    fixture.forkctl_ok(&["new", "--finish"]);
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]),
        "build-tooling\nfork-tooling"
    );
    let commit = stg_capture(&fixture.repo, ["id", "build-tooling"]);
    let message = git_capture(&fixture.repo, ["log", "-1", "--format=%B", &commit]);
    assert!(message.contains("Downstream-Reason: Add downstream build tooling."));
    assert!(message.contains("Upstream-Status: inappropriate: downstream-only tooling"));
    assert!(message.contains("Drop-When: The downstream build is retired."));
    fixture.forkctl_ok(&["verify"]);
}

#[test]
fn post_finish_change_requires_finish_again() {
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
    fixture.implement_patch("source-change", "source.txt", "first\n");
    fixture.forkctl_ok(&["new", "--finish"]);
    fixture.implement_patch("source-change", "source.txt", "second\n");
    let output = fixture.forkctl(&["publish"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("current tip"));
    fixture.forkctl_ok(&["new", "--finish"]);
    fixture.forkctl_ok(&["verify"]);
}

#[test]
fn wrong_branch_fails_before_mutation() {
    let fixture = Fixture::tooling_only();
    git_ok(&fixture.repo, ["switch", "--quiet", "-c", "other"]);
    let output = fixture.forkctl(&["init"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("current branch is other"));
    assert!(
        isolated_command("git")
            .args(["tag", "--list", "vsh/pre-sync-*"])
            .current_dir(&fixture.repo)
            .output()
            .unwrap()
            .stdout
            .is_empty()
    );
}
