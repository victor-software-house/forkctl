mod support;

use std::fs;
use support::{Fixture, git_ok};

#[test]
fn bootstrap_and_fresh_clone_hydration_are_idempotent() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&["check"]);
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]),
        "fork-tooling"
    );
    fixture.forkctl_ok(&["init"]);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.repo.join("patches/fork.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["contracts"]["required_text"][0]["path"],
        "base.txt"
    );
    assert_eq!(
        manifest["contracts"]["required_text"][0]["contains"],
        "base"
    );
}

#[test]
fn explicit_patch_workflow_captures_staged_and_generates_evidence() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "source-change",
        "-k",
        "source",
        "-p",
        "Change downstream source.",
        "-u",
        "not-submitted",
        "-d",
        "Upstream provides equivalent behavior.",
        "-s",
        "source.txt",
    ]);
    fs::write(fixture.repo.join("source.txt"), "downstream\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["check", "-s"]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
    assert!(
        fixture
            .repo
            .join("patches/downstream/0001-source-change.patch")
            .is_file()
    );
    let series = stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]);
    assert_eq!(
        series.lines().collect::<Vec<_>>(),
        ["source-change", "fork-tooling"]
    );
}

#[test]
fn patch_disable_enable_and_remove_preserve_audited_recovery() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "optional-feature", "optional.txt", "enabled\n");
    create_source_patch(&fixture, "later-feature", "later.txt", "later\n");

    let dry_run = fixture.forkctl_ok(&[
        "--format",
        "json",
        "patch",
        "disable",
        "optional-feature",
        "--reason",
        "Not needed in this host",
        "--dry-run",
    ]);
    assert!(dry_run.contains("patch.disable"));
    assert!(fixture.repo.join("optional.txt").is_file());

    fixture.forkctl_ok(&[
        "patch",
        "disable",
        "optional-feature",
        "--reason",
        "Not needed in this host",
    ]);
    assert!(!fixture.repo.join("optional.txt").exists());
    assert!(fixture.repo.join("later.txt").is_file());
    let status = fixture.forkctl_ok(&["--format", "json", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(
        status["result"]["patches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|patch| patch["name"] == "optional-feature" && patch["state"] == "disabled")
    );
    fixture.forkctl_ok(&["publish"]);

    fixture.forkctl_ok(&["patch", "enable", "optional-feature"]);
    assert_eq!(
        fs::read_to_string(fixture.repo.join("optional.txt")).unwrap(),
        "enabled\n"
    );
    assert!(fixture.repo.join("later.txt").is_file());
    fixture.forkctl_ok(&["publish"]);
    fixture.forkctl_ok(&["check"]);

    fixture.forkctl_ok(&[
        "patch",
        "remove",
        "optional-feature",
        "--reason",
        "Feature retired",
    ]);
    assert!(!fixture.repo.join("optional.txt").exists());
    assert!(fixture.repo.join("later.txt").is_file());
    fixture.forkctl_ok(&["publish"]);
    fixture.forkctl_ok(&["check"]);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.repo.join("patches/fork.json")).unwrap()).unwrap();
    assert!(
        !manifest["patches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|patch| patch["name"] == "optional-feature")
    );
    assert!(
        manifest["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "patch_removed")
    );
}

#[test]
fn staged_check_rejects_out_of_scope_paths() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "source-change",
        "-k",
        "source",
        "-p",
        "Change source.",
        "-u",
        "not-submitted",
        "-d",
        "Upstream changes.",
        "-s",
        "source.txt",
    ]);
    fs::write(fixture.repo.join("README.md"), "outside\n").unwrap();
    git_ok(&fixture.repo, ["add", "README.md"]);
    let output = fixture.forkctl(&["check", "-s"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside patch"));
}

#[test]
fn contract_edit_preflights_and_refreshes_bookkeeping() {
    let fixture = Fixture::new();
    let manifest_path = fixture.repo.join("patches/fork.json");
    let before = fs::read(&manifest_path).unwrap();

    fixture.forkctl_ok(&[
        "contract",
        "edit",
        "--clear",
        "--allow-base",
        "vendor/**",
        "--required-text",
        "base.txt=base",
        "--dry-run",
    ]);
    assert_eq!(fs::read(&manifest_path).unwrap(), before);

    fixture.forkctl_ok(&[
        "contract",
        "edit",
        "--clear",
        "--allow-base",
        "vendor/**",
        "--required-text",
        "base.txt=base",
    ]);
    fixture.forkctl_ok(&["check"]);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["contracts"]["allow_base"][0], "vendor/**");

    let valid = fs::read(&manifest_path).unwrap();
    let rejected = fixture.forkctl(&[
        "--format",
        "json",
        "contract",
        "edit",
        "--required-text",
        "missing.txt=required",
    ]);
    assert!(!rejected.status.success());
    assert_eq!(fs::read(&manifest_path).unwrap(), valid);
}

#[test]
fn patch_kind_change_reorders_series_and_exports() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "first-source", "first.txt", "first\n");
    create_source_patch(&fixture, "second-source", "second.txt", "second\n");

    fixture.forkctl_ok(&["patch", "edit", "first-source", "--kind", "tooling"]);
    fixture.forkctl_ok(&["check"]);

    let series = stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]);
    assert_eq!(
        series.lines().collect::<Vec<_>>(),
        ["second-source", "first-source", "fork-tooling"]
    );
    assert!(
        fixture
            .repo
            .join("patches/downstream/0001-second-source.patch")
            .is_file()
    );
    assert!(
        !fixture
            .repo
            .join("patches/downstream/0001-first-source.patch")
            .exists()
    );
    assert!(
        !fixture
            .repo
            .join("patches/downstream/0002-second-source.patch")
            .exists()
    );
    let operation = git_capture_dynamic(
        &fixture.repo,
        &["rev-parse", "--git-path", "forkctl/operation.json"],
    );
    assert!(!fixture.repo.join(operation).exists());
}

#[test]
fn bookkeeping_patch_edit_remains_final_after_source_patches() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "source\n");
    fixture.forkctl_ok(&["patch", "select", "fork-tooling"]);

    fixture.forkctl_ok(&[
        "patch",
        "edit",
        "fork-tooling",
        "--add-scope",
        ".github/workflows/release.yml",
    ]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);

    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"])
            .lines()
            .collect::<Vec<_>>(),
        ["source-change", "fork-tooling"]
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.repo.join("patches/fork.json")).unwrap()).unwrap();
    assert!(
        manifest["patches"][1]["scope"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == ".github/workflows/release.yml")
    );
}

#[test]
fn lower_patch_refresh_conflict_can_continue_from_typed_journal() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "lower", "shared.txt", "lower\n");
    create_source_patch(&fixture, "upper", "shared.txt", "upper\n");
    fixture.forkctl_ok(&["patch", "select", "lower"]);
    fs::write(fixture.repo.join("shared.txt"), "lower updated\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);

    let refresh = fixture.forkctl(&["--format", "json", "patch", "refresh"]);
    assert!(!refresh.status.success());
    let refresh: serde_json::Value = serde_json::from_slice(&refresh.stdout).unwrap();
    assert_eq!(refresh["error"]["code"], "subprocess_failed");

    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["operation"]["kind"], "patch_refresh");
    assert_eq!(status["result"]["operation"]["phase"], "conflict");

    fs::write(fixture.repo.join("shared.txt"), "lower updated\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);
    let first_continue = fixture.forkctl(&["operation", "continue"]);
    assert!(!first_continue.status.success());
    fs::write(fixture.repo.join("shared.txt"), "lower updated\nupper\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);
    fixture.forkctl_ok(&["operation", "continue"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn patch_kind_reorder_conflict_can_continue_from_typed_journal() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "lower", "shared.txt", "lower\n");
    create_source_patch(&fixture, "upper", "shared.txt", "upper\n");

    let edit = fixture.forkctl(&[
        "--format", "json", "patch", "edit", "lower", "--kind", "tooling",
    ]);
    assert!(!edit.status.success());
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["operation"]["kind"], "patch_edit");
    assert_eq!(status["result"]["operation"]["phase"], "conflict");

    fs::write(fixture.repo.join("shared.txt"), "upper\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);
    let first_continue = fixture.forkctl(&["operation", "continue"]);
    assert!(!first_continue.status.success());
    fs::write(fixture.repo.join("shared.txt"), "lower\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);
    fixture.forkctl_ok(&["operation", "continue"]);
    fixture.forkctl_ok(&["check"]);
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"])
            .lines()
            .collect::<Vec<_>>(),
        ["upper", "lower", "fork-tooling"]
    );
}

#[test]
fn patch_refresh_abort_restores_stack_and_active_patch() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "lower", "shared.txt", "lower\n");
    create_source_patch(&fixture, "upper", "shared.txt", "upper\n");
    fixture.forkctl_ok(&["patch", "select", "lower"]);
    let old_tip = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    let old_series = stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]);
    fs::write(fixture.repo.join("shared.txt"), "lower updated\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);
    assert!(!fixture.forkctl(&["patch", "refresh"]).status.success());

    fixture.forkctl_ok(&["operation", "abort", "--yes"]);
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), old_tip);
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]),
        old_series
    );
    let status = fixture.forkctl_ok(&["--format", "json", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["active_patch"]["patch"], "lower");
    assert!(status["result"]["operation"].is_null());
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn refresh_consumes_pre_commit_hook_modified_index() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "hook-format",
        "--kind",
        "source",
        "--purpose",
        "format through the existing hook",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "upstream adopts formatting",
        "--scope",
        "hook.txt",
    ]);
    let hook = fixture.repo.join(".git/hooks/pre-commit");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf 'formatted\\n' > hook.txt\ngit add hook.txt\n",
    )
    .unwrap();
    make_executable(&hook);
    fs::write(fixture.repo.join("hook.txt"), "raw\n").unwrap();
    git_ok(&fixture.repo, ["add", "hook.txt"]);

    fixture.forkctl_ok(&["patch", "refresh"]);
    assert_eq!(
        fs::read_to_string(fixture.repo.join("hook.txt")).unwrap(),
        "formatted\n"
    );
    let commit = stg_capture(&fixture.repo, ["id", "hook-format", "--"]);
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["show", &format!("{commit}:hook.txt")]),
        "formatted"
    );
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn documented_staged_check_hook_accepts_source_and_bookkeeping_refreshes() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "hook-check",
        "--kind",
        "source",
        "--purpose",
        "validate source and generated bookkeeping scopes",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "upstream provides equivalent behavior",
        "--scope",
        "hook-check.txt",
    ]);
    let hook = fixture.repo.join(".git/hooks/pre-commit");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nexec '{}' --manifest patches/fork.json check -s >/dev/null\n",
            env!("CARGO_BIN_EXE_forkctl")
        ),
    )
    .unwrap();
    make_executable(&hook);
    fs::write(fixture.repo.join("hook-check.txt"), "downstream\n").unwrap();
    git_ok(&fixture.repo, ["add", "hook-check.txt"]);

    fixture.forkctl_ok(&["patch", "refresh"]);
    let status = fixture.forkctl_ok(&["--format", "json", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["active_patch"]["patch"], "hook-check");
    assert!(status["result"]["operation"].is_null());
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn no_op_refresh_does_not_create_an_operation() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&["patch", "select", "source-change"]);
    let output = fixture.forkctl(&["--format", "json", "patch", "refresh"]);
    assert!(!output.status.success());
    let output: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["error"]["code"], "capture_conflict");
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
}

#[test]
fn refresh_dry_run_is_non_mutating() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "source-change",
        "-k",
        "source",
        "-p",
        "Change source.",
        "-u",
        "not-submitted",
        "-d",
        "Upstream changes.",
        "-s",
        "source.txt",
    ]);
    fs::write(fixture.repo.join("source.txt"), "downstream\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    let head = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    fixture.forkctl_ok(&["patch", "refresh", "--dry-run"]);
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), head);
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]),
        "fork-tooling"
    );
}

#[test]
fn hook_exported_git_environment_does_not_contaminate_nested_clone() {
    let fixture = Fixture::new();
    let git_dir = git_capture(&fixture.repo, ["rev-parse", "--git-dir"]);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_forkctl"))
        .args(["--manifest", "patches/fork.json", "check"])
        .current_dir(&fixture.repo)
        .env("GIT_DIR", fixture.repo.join(git_dir))
        .env("GIT_WORK_TREE", &fixture.repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn json_status_is_one_clean_envelope() {
    let fixture = Fixture::new();
    let output = fixture.api_call(
        "execute",
        &serde_json::json!({"command":"status","arguments":{}}),
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "success");
    assert_eq!(response["command"], "status");
}

#[test]
fn rebase_publish_and_fresh_clone_hydrate_exact_recovery() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_upstream(&fixture.repo, "upstream v2\n");
    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let recovery = response["result"]["recovery_tag"].as_str().unwrap();
    fixture.forkctl_ok(&["publish"]);
    let remote = git_capture_dynamic(&fixture.repo, &["remote", "get-url", "origin"]);
    let clone = fixture.repo.parent().unwrap().join("fresh-clone");
    git_ok(
        fixture.repo.parent().unwrap(),
        ["clone", "--quiet", remote.as_str(), clone.to_str().unwrap()],
    );
    git_ok(&clone, ["config", "user.name", "Forkctl Test"]);
    git_ok(&clone, ["config", "user.email", "forkctl@example.com"]);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_forkctl"))
        .args(["--manifest", "patches/fork.json", "init"])
        .current_dir(&clone)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!git_capture_dynamic(&clone, &["tag", "--list", recovery]).is_empty());
}

#[test]
fn operation_abort_restores_exact_old_stack_before_clearing_journal() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    let old_tip = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    let old_series = stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]);
    advance_upstream(&fixture.repo, "upstream v2\n");
    fixture.forkctl_ok(&["rebase", "--onto", "refs/heads/main"]);
    assert_ne!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), old_tip);

    fixture.forkctl_ok(&["operation", "abort", "--dry-run"]);
    assert_ne!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), old_tip);
    let unconfirmed = fixture.forkctl(&["--format", "json", "operation", "abort"]);
    assert!(!unconfirmed.status.success());
    let unconfirmed: serde_json::Value = serde_json::from_slice(&unconfirmed.stdout).unwrap();
    assert_eq!(unconfirmed["error"]["code"], "invalid_request");

    fixture.forkctl_ok(&["operation", "abort", "--yes"]);
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), old_tip);
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]),
        old_series
    );
    fixture.forkctl_ok(&["check"]);
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
}

#[test]
fn active_patch_commands_and_abort_fail_before_mutating_an_operation() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    let old_tip = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    advance_upstream(&fixture.repo, "upstream v2\n");
    fixture.forkctl_ok(&["rebase", "--onto", "refs/heads/main"]);

    let new_tip = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    let operation_relative = git_capture_dynamic(
        &fixture.repo,
        &["rev-parse", "--git-path", "forkctl/operation.json"],
    );
    let operation_path = fixture.repo.join(operation_relative);
    let operation_before = fs::read(&operation_path).unwrap();
    let active_relative = git_capture_dynamic(
        &fixture.repo,
        &["rev-parse", "--git-path", "forkctl/active.json"],
    );
    let active_path = fixture.repo.join(active_relative);

    let create = fixture.forkctl(&[
        "--format",
        "json",
        "patch",
        "create",
        "blocked",
        "--kind",
        "tooling",
        "--purpose",
        "Must not be created during an operation.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "The operation completes.",
        "--scope",
        "blocked.txt",
    ]);
    assert!(!create.status.success());
    let create: serde_json::Value = serde_json::from_slice(&create.stdout).unwrap();
    assert_eq!(create["error"]["code"], "operation_in_progress");

    let select = fixture.forkctl(&["--format", "json", "patch", "select", "fork-tooling"]);
    assert!(!select.status.success());
    let select: serde_json::Value = serde_json::from_slice(&select.stdout).unwrap();
    assert_eq!(select["error"]["code"], "operation_in_progress");
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), new_tip);
    assert_eq!(fs::read(&operation_path).unwrap(), operation_before);
    assert!(!active_path.exists());

    fs::write(
        &active_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "mode": "existing",
            "patch": "fork-tooling"
        }))
        .unwrap(),
    )
    .unwrap();

    let abort = fixture.forkctl(&["--format", "json", "operation", "abort", "--yes"]);
    assert!(!abort.status.success());
    let abort: serde_json::Value = serde_json::from_slice(&abort.stdout).unwrap();
    assert_eq!(abort["error"]["code"], "active_patch_exists");
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), new_tip);
    assert_eq!(fs::read(&operation_path).unwrap(), operation_before);
    assert!(active_path.is_file());

    fs::remove_file(active_path).unwrap();
    fixture.forkctl_ok(&["operation", "abort", "--yes"]);
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), old_tip);
    assert!(!operation_path.exists());
}

#[test]
fn operation_rejects_deleted_and_substituted_recovery_tags() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_upstream(&fixture.repo, "upstream v2\n");
    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let tag = response["result"]["recovery_tag"].as_str().unwrap();
    let tag_object = response["result"]["recovery_tag_object"].as_str().unwrap();
    let old_tip = response["result"]["old_tip"].as_str().unwrap();
    let tag_ref = format!("refs/tags/{tag}");

    git_ok_dynamic(&fixture.repo, &["update-ref", "-d", &tag_ref]);
    assert!(!fixture.forkctl(&["check"]).status.success());

    git_ok_dynamic(&fixture.repo, &["update-ref", &tag_ref, old_tip]);
    assert!(!fixture.forkctl(&["check"]).status.success());

    git_ok_dynamic(&fixture.repo, &["update-ref", "-d", &tag_ref]);
    git_ok_dynamic(
        &fixture.repo,
        &["tag", "-a", "-m", "wrong recovery", tag, "HEAD"],
    );
    assert!(!fixture.forkctl(&["check"]).status.success());

    git_ok_dynamic(&fixture.repo, &["update-ref", &tag_ref, tag_object]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn publish_rejects_stale_lease_without_partial_refs() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_upstream(&fixture.repo, "upstream v2\n");
    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let tag = response["result"]["recovery_tag"].as_str().unwrap();
    advance_downstream(&fixture.repo, "remote advanced\n");
    let remote_before =
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let publish = fixture.forkctl(&["--format", "json", "publish"]);
    assert!(!publish.status.success());
    let publish: serde_json::Value = serde_json::from_slice(&publish.stdout).unwrap();
    assert_eq!(publish["error"]["code"], "remote_advanced");
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]),
        remote_before
    );
    assert!(
        git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{tag}")]
        )
        .is_empty()
    );
    assert_operation_present(&fixture);
}

#[test]
fn publish_rejects_conflicting_remote_recovery_tag() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_upstream(&fixture.repo, "upstream v2\n");
    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let tag = response["result"]["recovery_tag"].as_str().unwrap();
    git_ok_dynamic(
        &fixture.repo,
        &["push", "origin", &format!("HEAD:refs/tags/{tag}")],
    );

    let publish = fixture.forkctl(&["--format", "json", "publish"]);
    assert!(!publish.status.success());
    let publish: serde_json::Value = serde_json::from_slice(&publish.stdout).unwrap();
    assert_eq!(publish["error"]["code"], "publication_rejected");
    assert_operation_present(&fixture);
}

#[test]
fn publish_preserves_refs_when_remote_policy_rejects_atomic_push() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_upstream(&fixture.repo, "upstream v2\n");
    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let tag = response["result"]["recovery_tag"].as_str().unwrap();
    let remote = git_capture_dynamic(&fixture.repo, &["remote", "get-url", "origin"]);
    let hook = std::path::Path::new(&remote).join("hooks/pre-receive");
    fs::write(
        &hook,
        "#!/bin/sh\necho 'protected branch policy' >&2\nexit 1\n",
    )
    .unwrap();
    make_executable(&hook);
    let branch_before =
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let publish = fixture.forkctl(&["--format", "json", "publish"]);
    assert!(!publish.status.success());
    let publish: serde_json::Value = serde_json::from_slice(&publish.stdout).unwrap();
    assert_eq!(publish["error"]["code"], "publication_rejected");
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]),
        branch_before
    );
    assert!(
        git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{tag}")]
        )
        .is_empty()
    );
    assert_operation_present(&fixture);
}

#[test]
fn publish_has_no_fallback_when_remote_lacks_atomic_push() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_upstream(&fixture.repo, "upstream v2\n");
    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let tag = response["result"]["recovery_tag"].as_str().unwrap();
    let remote = git_capture_dynamic(&fixture.repo, &["remote", "get-url", "origin"]);
    git_ok_dynamic(
        &fixture.repo,
        &[
            "--git-dir",
            &remote,
            "config",
            "receive.advertiseAtomic",
            "false",
        ],
    );
    let branch_before =
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let publish = fixture.forkctl(&["--format", "json", "publish"]);
    assert!(!publish.status.success());
    let publish: serde_json::Value = serde_json::from_slice(&publish.stdout).unwrap();
    assert_eq!(publish["error"]["code"], "publication_rejected");
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]),
        branch_before
    );
    assert!(
        git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{tag}")]
        )
        .is_empty()
    );
    assert_operation_present(&fixture);
}

#[test]
fn patch_work_publishes_under_an_exact_lease_with_recovery() {
    let fixture = Fixture::new();
    let bootstrap = fixture.forkctl_ok(&["--format", "json", "publish"]);
    let bootstrap: serde_json::Value = serde_json::from_str(&bootstrap).unwrap();
    assert_eq!(bootstrap["result"]["fast_forward"], true);
    assert_eq!(bootstrap["result"]["already_published"], false);
    assert!(
        bootstrap["result"]["recovery_tags"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let published_base = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    let head = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    assert_ne!(head, published_base);

    let publish = fixture.forkctl_ok(&["--format", "json", "publish"]);
    let publish: serde_json::Value = serde_json::from_str(&publish).unwrap();
    assert_eq!(publish["result"]["already_published"], false);
    assert_eq!(publish["result"]["fast_forward"], false);
    assert_eq!(publish["result"]["expected_lease"], published_base);
    let recovery = publish["result"]["recovery_tags"].as_array().unwrap();
    assert_eq!(recovery.len(), 1);
    let tag = recovery[0]
        .as_str()
        .unwrap()
        .split(" -> ")
        .next()
        .unwrap()
        .to_string();

    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"])
            .split_whitespace()
            .next()
            .unwrap(),
        head
    );
    assert_eq!(
        git_capture_dynamic(
            &fixture.repo,
            &["rev-parse", &format!("refs/tags/{tag}^{{commit}}")]
        ),
        published_base,
        "publication recovery tag must preserve the overwritten remote tip"
    );
    assert!(
        !git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{tag}")]
        )
        .is_empty(),
        "publication recovery tag must be published"
    );

    let again = fixture.forkctl_ok(&["--format", "json", "publish"]);
    let again: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(again["result"]["already_published"], true);
}

#[test]
fn patch_work_rewrite_refuses_an_unfetched_remote_advance() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&["publish"]);
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_downstream(&fixture.repo, "remote advanced\n");
    let remote_before =
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let publish = fixture.forkctl(&["--format", "json", "publish"]);
    assert!(!publish.status.success());
    let publish: serde_json::Value = serde_json::from_slice(&publish.stdout).unwrap();
    assert_eq!(publish["error"]["code"], "remote_advanced");
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]),
        remote_before
    );
}

#[test]
fn recovery_commands_survive_an_unreadable_tracked_manifest() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "lower", "shared.txt", "lower\n");
    create_source_patch(&fixture, "upper", "shared.txt", "upper\n");
    fixture.forkctl_ok(&["patch", "select", "lower"]);
    fs::write(fixture.repo.join("shared.txt"), "lower updated\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);
    assert!(!fixture.forkctl(&["patch", "refresh"]).status.success());

    fs::create_dir_all(fixture.repo.join("patches")).unwrap();
    fs::write(
        fixture.repo.join("patches/fork.json"),
        "<<<<<<< HEAD\nconflict\n=======\nmarkers\n>>>>>>> theirs\n",
    )
    .unwrap();

    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["operation"]["kind"], "patch_refresh");
    fixture.forkctl_ok(&["--format", "json", "status"]);
    fixture.forkctl_ok(&["operation", "abort", "--yes"]);
    let status = fixture.forkctl_ok(&["--format", "json", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
    assert_eq!(status["result"]["active_patch"]["patch"], "lower");
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn invalid_tracked_manifest_never_bootstraps_a_replacement() {
    let fixture = Fixture::new();
    fs::write(fixture.repo.join("patches/fork.json"), "not json\n").unwrap();

    let init = fixture.forkctl(&["--format", "json", "init"]);
    assert!(!init.status.success());
    let init: serde_json::Value = serde_json::from_slice(&init.stdout).unwrap();
    assert_eq!(init["error"]["code"], "manifest_invalid");

    git_ok(&fixture.repo, ["add", "patches/fork.json"]);
    git_ok(&fixture.repo, ["stash"]);
    fs::write(fixture.repo.join("patches/fork.json"), "not json\n").unwrap();
    git_ok(&fixture.repo, ["add", "patches/fork.json"]);
    stg_ok_dynamic(&fixture.repo, &["refresh", "--index"]);
    let check = fixture.forkctl(&["--format", "json", "check"]);
    assert!(!check.status.success());
    let check: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(check["error"]["code"], "manifest_invalid");
}

fn assert_operation_present(fixture: &Fixture) {
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_object());
}

fn create_source_patch(fixture: &Fixture, name: &str, path: &str, contents: &str) {
    fixture.forkctl_ok(&[
        "patch",
        "create",
        name,
        "--kind",
        "source",
        "--purpose",
        "Change downstream source.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream provides equivalent behavior.",
        "--scope",
        path,
    ]);
    std::fs::write(fixture.repo.join(path), contents).unwrap();
    git_ok(&fixture.repo, ["add", path]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);
}

fn advance_upstream(repo: &std::path::Path, contents: &str) {
    let upstream = git_capture_dynamic(repo, &["remote", "get-url", "upstream"]);
    let work = repo.parent().unwrap().join("advance-upstream");
    git_ok(
        repo.parent().unwrap(),
        [
            "clone",
            "--quiet",
            upstream.as_str(),
            work.to_str().unwrap(),
        ],
    );
    git_ok(&work, ["config", "user.name", "Forkctl Test"]);
    git_ok(&work, ["config", "user.email", "forkctl@example.com"]);
    std::fs::write(work.join("upstream.txt"), contents).unwrap();
    git_ok(&work, ["add", "upstream.txt"]);
    git_ok(&work, ["commit", "--quiet", "-m", "upstream v2"]);
    git_ok(&work, ["push", "--quiet", "origin", "main"]);
}

fn advance_downstream(repo: &std::path::Path, contents: &str) {
    let downstream = git_capture_dynamic(repo, &["remote", "get-url", "origin"]);
    let work = repo.parent().unwrap().join("advance-downstream");
    git_ok(
        repo.parent().unwrap(),
        [
            "clone",
            "--quiet",
            downstream.as_str(),
            work.to_str().unwrap(),
        ],
    );
    git_ok(&work, ["config", "user.name", "Forkctl Test"]);
    git_ok(&work, ["config", "user.email", "forkctl@example.com"]);
    fs::write(work.join("remote.txt"), contents).unwrap();
    git_ok(&work, ["add", "remote.txt"]);
    git_ok(&work, ["commit", "--quiet", "-m", "remote advance"]);
    git_ok(&work, ["push", "--quiet", "origin", "main"]);
}

fn git_capture_dynamic(repo: &std::path::Path, args: &[&str]) -> String {
    capture(repo, "git", args)
}

fn git_ok_dynamic(repo: &std::path::Path, args: &[&str]) {
    let output = support::isolated_command("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn capture(repo: &std::path::Path, program: &str, args: &[&str]) -> String {
    let output = support::isolated_command(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn stg_capture(repo: &std::path::Path, args: [&str; 3]) -> String {
    capture(repo, "stg", &args)
}

fn stg_ok_dynamic(repo: &std::path::Path, args: &[&str]) {
    let output = support::isolated_command("stg")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stg {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_capture(repo: &std::path::Path, args: [&str; 2]) -> String {
    capture(repo, "git", &args)
}

fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}
