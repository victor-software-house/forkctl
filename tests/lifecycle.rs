mod support;

use std::fs;
use support::{Fixture, git_ok, isolated_command};

#[test]
fn bootstrap_and_fresh_clone_hydration_are_idempotent() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&["check"]);
    assert_eq!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"]),
        "fork-tooling"
    );
    fixture.forkctl_ok(&["init"]);
    let manifest = read_manifest_value(&fixture.repo.join("patches/fork.yaml"));
    assert_eq!(
        manifest["contracts"]["required_text"][0]["path"],
        "base.txt"
    );
    assert_eq!(
        manifest["contracts"]["required_text"][0]["contains"],
        "base"
    );
    assert_eq!(manifest["commit_messages"]["source"]["type"], "feat");
    assert_eq!(manifest["commit_messages"]["tooling"]["scope"], "tooling");
    assert_eq!(
        patch_subject(&fixture.repo, "fork-tooling"),
        "chore(tooling): fork tooling"
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
fn patch_commit_subject_override_can_return_to_the_kind_default() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "source-fix",
        "--kind",
        "source",
        "--purpose",
        "Fix downstream source.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream provides equivalent behavior.",
        "--commit-subject",
        "fix(runtime): reject stale state",
        "--scope",
        "source.txt",
    ]);
    fs::write(fixture.repo.join("source.txt"), "fixed\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    assert_eq!(
        patch_subject(&fixture.repo, "source-fix"),
        "fix(runtime): reject stale state"
    );

    fixture.forkctl_ok(&["patch", "edit", "source-fix", "--default-commit"]);
    assert_eq!(
        patch_subject(&fixture.repo, "source-fix"),
        "feat: source fix"
    );
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn check_rejects_commit_subject_drift() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    rewrite_patch_subject(&fixture.repo, "source-change", "wrong subject");

    let check = fixture.forkctl(&["--format", "json", "check"]);
    assert!(!check.status.success());
    let check: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert!(
        check["error"]["message"]
            .as_str()
            .unwrap()
            .contains("expected \"feat: source change\"")
    );
}

#[test]
fn check_rejects_a_missing_explicit_commit_message_policy() {
    let fixture = Fixture::new();
    remove_commit_message_policy(&fixture);

    let check = fixture.forkctl(&["--format", "json", "check"]);
    assert!(!check.status.success());
    let check: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert!(
        check["error"]["message"]
            .as_str()
            .unwrap()
            .contains("manifest has no explicit commit_messages policy")
    );
}

#[test]
fn discover_restores_missing_mise_toml_from_workspace_snapshot() {
    let fixture = Fixture::new();
    let snap_dir = fixture.repo.join(".git/forkctl/workspace");
    fs::create_dir_all(&snap_dir).unwrap();
    fs::write(snap_dir.join("mise.toml"), "min_version = \"2026.7.7\"\n").unwrap();
    assert!(!fixture.repo.join("mise.toml").exists());
    fixture.forkctl_ok(&["status"]);
    assert_eq!(
        fs::read_to_string(fixture.repo.join("mise.toml")).unwrap(),
        "min_version = \"2026.7.7\"\n"
    );
}

#[test]
fn refresh_below_the_top_is_refused_without_rewrite_below() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "lower", "shared.txt", "lower\n");
    create_source_patch(&fixture, "upper", "other.txt", "upper\n");
    fixture.forkctl_ok(&["patch", "select", "lower"]);
    fs::write(fixture.repo.join("shared.txt"), "lower updated\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);

    let refresh = fixture.forkctl(&["--format", "json", "patch", "refresh"]);
    assert!(!refresh.status.success());
    let refresh: serde_json::Value = serde_json::from_slice(&refresh.stdout).unwrap();
    assert_eq!(refresh["error"]["code"], "operation_conflict");
    assert!(
        refresh["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--rewrite-below")
    );
    assert_eq!(
        refresh["error"]["suggested_command"],
        "mise run fork patch create NAME"
    );
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
}

#[test]
fn complete_check_rejects_unexpected_tracked_patch_export() {
    let fixture = Fixture::new();
    let stale = fixture.repo.join("patches/downstream/9999-stale.patch");
    fs::create_dir_all(stale.parent().unwrap()).unwrap();
    fs::write(&stale, "stale generated evidence\n").unwrap();
    git_ok(
        &fixture.repo,
        ["add", "patches/downstream/9999-stale.patch"],
    );
    let refresh = isolated_command(&fixture.repo, "stg")
        .args(["refresh", "--patch", "fork-tooling", "--index"])
        .output()
        .unwrap();
    assert!(
        refresh.status.success(),
        "{}",
        String::from_utf8_lossy(&refresh.stderr)
    );

    let check = fixture.forkctl(&["check"]);
    assert!(!check.status.success());
    assert!(
        String::from_utf8_lossy(&check.stderr)
            .contains("unexpected generated patch exports: patches/downstream/9999-stale.patch")
    );
}

#[test]
fn yaml_manifest_preserves_disable_enable_and_remove_history() {
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
    let manifest = read_manifest_value(&fixture.repo.join("patches/fork.yaml"));
    assert_eq!(
        manifest["disabled_patches"][0]["patch"]["name"],
        "optional-feature"
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
    let manifest = read_manifest_value(&fixture.repo.join("patches/fork.yaml"));
    assert_eq!(
        manifest["disabled_patches"].as_array().unwrap().as_slice(),
        &[] as &[serde_json::Value]
    );
    assert!(
        manifest["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "patch_enabled")
    );
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
    let manifest_path = fixture.repo.join("patches/fork.yaml");
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
    let manifest = read_manifest_value(&manifest_path);
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
    let manifest = read_manifest_value(&fixture.repo.join("patches/fork.yaml"));
    assert!(
        manifest["patches"][1]["scope"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == ".github/workflows/release.yml")
    );
}

#[test]
fn patch_enable_continuation_adopts_the_current_commit_policy() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "optional-feature", "optional.txt", "enabled\n");
    fixture.forkctl_ok(&[
        "patch",
        "disable",
        "optional-feature",
        "--reason",
        "Not needed in this host",
    ]);
    fixture.forkctl_ok(&["publish"]);
    fixture.forkctl_ok(&[
        "contract",
        "migrate-commit-messages",
        "--source",
        "fix(runtime)",
    ]);
    fixture.forkctl_ok(&["publish"]);

    let sentinel = fixture.repo.join(".git/fail-current-policy-once");
    fs::write(&sentinel, "fail\n").unwrap();
    let hook = fixture.repo.join(".git/hooks/commit-msg");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nif test -f '{}' && grep -q '^fix(runtime):' \"$1\"; then rm '{}'; exit 1; fi\n",
            sentinel.display(),
            sentinel.display()
        ),
    )
    .unwrap();
    make_executable(&hook);

    let enable = fixture.forkctl(&["patch", "enable", "optional-feature"]);
    assert!(!enable.status.success());
    assert_operation_present(&fixture);
    fixture.forkctl_ok(&["operation", "continue"]);

    assert_eq!(
        patch_subject(&fixture.repo, "optional-feature"),
        "fix(runtime): optional feature"
    );
    assert!(
        stg_capture(&fixture.repo, ["series", "--all", "--no-prefix"])
            .lines()
            .any(|patch| patch == "optional-feature")
    );
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn lower_patch_refresh_conflict_can_continue_from_typed_journal() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "lower", "shared.txt", "lower\n");
    create_source_patch(&fixture, "upper", "shared.txt", "upper\n");
    fixture.forkctl_ok(&["patch", "select", "lower"]);
    fs::write(fixture.repo.join("shared.txt"), "lower updated\n").unwrap();
    git_ok(&fixture.repo, ["add", "shared.txt"]);

    let refresh = fixture.forkctl(&["--format", "json", "patch", "refresh", "--rewrite-below"]);
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
    assert!(
        !fixture
            .forkctl(&["patch", "refresh", "--rewrite-below"])
            .status
            .success()
    );

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
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
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
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nexec '{}' --manifest patches/fork.yaml check -s >/dev/null\n",
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
    let output = fixture.forkctl(&["--format", "json", "patch", "refresh", "--rewrite-below"]);
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
    let output = support::forkctl_command(&fixture.repo)
        .args(["--manifest", "patches/fork.yaml", "check"])
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
    assert_eq!(output.stderr, &[] as &[u8]);
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
    let output = support::forkctl_command(&clone)
        .args(["--manifest", "patches/fork.yaml", "init"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(
        git_capture_dynamic(&clone, &["tag", "--list", recovery]),
        ""
    );
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
    assert_eq!(
        git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{tag}")]
        ),
        ""
    );
    assert_operation_present(&fixture);
}

#[test]
fn publish_retry_verifies_completed_remote_refs_and_cleans_local_operation() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "optional-feature", "optional.txt", "enabled\n");
    create_source_patch(&fixture, "later-feature", "later.txt", "later\n");
    fixture.forkctl_ok(&["publish"]);
    let remote_before =
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"])
            .split_whitespace()
            .next()
            .unwrap()
            .to_string();
    let response = fixture.forkctl_ok(&[
        "--format",
        "json",
        "patch",
        "disable",
        "optional-feature",
        "--reason",
        "Not needed in this host",
    ]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let tag = response["result"]["recovery_tag"].as_str().unwrap();
    let tag_ref = format!("refs/tags/{tag}");
    let tag_object = git_capture_dynamic(&fixture.repo, &["rev-parse", &tag_ref]);
    let lease = format!("--force-with-lease=refs/heads/main:{remote_before}");
    let tag_refspec = format!("{tag_ref}:{tag_ref}");

    git_ok_dynamic(
        &fixture.repo,
        &[
            "push",
            "--atomic",
            &lease,
            "origin",
            &tag_refspec,
            "HEAD:refs/heads/main",
        ],
    );
    assert_operation_present(&fixture);

    let retry = fixture.forkctl_ok(&["--format", "json", "publish"]);
    let retry: serde_json::Value = serde_json::from_str(&retry).unwrap();
    assert_eq!(retry["result"]["already_published"], true);
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", &tag_ref])
            .split_whitespace()
            .next()
            .unwrap(),
        tag_object
    );
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
    let snapshot = git_capture_dynamic(
        &fixture.repo,
        &["rev-parse", "--git-path", "forkctl/manifest.json"],
    );
    let snapshot = std::path::PathBuf::from(snapshot);
    let snapshot = if snapshot.is_absolute() {
        snapshot
    } else {
        fixture.repo.join(snapshot)
    };
    assert!(!snapshot.exists());
}

#[test]
fn publish_repairs_branch_only_update_with_missing_recovery_evidence() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_upstream(&fixture.repo, "upstream v2\n");
    let remote_before =
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"])
            .split_whitespace()
            .next()
            .unwrap()
            .to_string();
    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    let tag = response["result"]["recovery_tag"].as_str().unwrap();
    let tag_ref = format!("refs/tags/{tag}");
    let lease = format!("--force-with-lease=refs/heads/main:{remote_before}");

    git_ok_dynamic(
        &fixture.repo,
        &["push", &lease, "origin", "HEAD:refs/heads/main"],
    );
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", &tag_ref]),
        ""
    );

    let repair = fixture.forkctl_ok(&["--format", "json", "publish"]);
    let repair: serde_json::Value = serde_json::from_str(&repair).unwrap();
    assert_eq!(repair["result"]["already_published"], false);
    let recoveries = repair["result"]["recovery_tags"].as_array().unwrap();
    assert_eq!(recoveries.len(), 2);
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", &tag_ref])
            .split_whitespace()
            .next()
            .unwrap(),
        response["result"]["recovery_tag_object"].as_str().unwrap()
    );
    assert!(recoveries.iter().any(|recovery| {
        let tag = recovery.as_str().unwrap().split(" -> ").next().unwrap();
        git_capture_dynamic(
            &fixture.repo,
            &["rev-parse", &format!("refs/tags/{tag}^{{commit}}")],
        ) == remote_before
    }));
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
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
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
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
    assert_eq!(
        git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{tag}")]
        ),
        ""
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
    assert_eq!(
        git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{tag}")]
        ),
        ""
    );
    assert_operation_present(&fixture);
}

#[test]
fn pretty_publish_streams_pre_push_output_and_json_does_not() {
    let fixture = Fixture::new();
    let hook = fixture.repo.join(".git/hooks/pre-push");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(&hook, "#!/bin/sh\necho FORKCTL_HOOK_STREAM\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook, permissions).unwrap();
    }

    let pretty = fixture.forkctl(&["publish"]);
    assert!(
        pretty.status.success(),
        "pretty publish failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&pretty.stdout),
        String::from_utf8_lossy(&pretty.stderr)
    );
    assert!(
        String::from_utf8_lossy(&pretty.stderr).contains("FORKCTL_HOOK_STREAM"),
        "pretty publish must stream hook output to stderr: {}",
        String::from_utf8_lossy(&pretty.stderr)
    );

    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    let json = fixture.forkctl(&["--format", "json", "publish"]);
    assert!(
        json.status.success(),
        "json publish failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&json.stdout),
        String::from_utf8_lossy(&json.stderr)
    );
    assert!(
        json.stderr.is_empty(),
        "json publish must keep stderr empty, got: {}",
        String::from_utf8_lossy(&json.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&json.stdout).contains("FORKCTL_HOOK_STREAM"),
        "json stdout must not contain hook output"
    );
}

#[test]
fn patch_work_publishes_under_an_exact_lease_with_recovery() {
    let fixture = Fixture::new();
    let bootstrap = fixture.forkctl_ok(&["--format", "json", "publish"]);
    let bootstrap: serde_json::Value = serde_json::from_str(&bootstrap).unwrap();
    assert_eq!(bootstrap["result"]["fast_forward"], true);
    assert_eq!(bootstrap["result"]["already_published"], false);
    assert_eq!(
        bootstrap["result"]["recovery_tags"]
            .as_array()
            .unwrap()
            .as_slice(),
        &[] as &[serde_json::Value]
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
fn patch_work_append_refuses_an_unfetched_remote_advance() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&["publish"]);
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    advance_downstream(&fixture.repo, "remote advanced\n");
    let remote_before =
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let publish = fixture.forkctl(&["--format", "json", "publish", "--append"]);
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
    assert!(
        !fixture
            .forkctl(&["patch", "refresh", "--rewrite-below"])
            .status
            .success()
    );

    fs::create_dir_all(fixture.repo.join("patches")).unwrap();
    fs::write(
        fixture.repo.join("patches/fork.yaml"),
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
    fs::write(fixture.repo.join("patches/fork.yaml"), "not: [valid\n").unwrap();

    let init = fixture.forkctl(&["--format", "json", "init"]);
    assert!(!init.status.success());
    let init: serde_json::Value = serde_json::from_slice(&init.stdout).unwrap();
    assert_eq!(init["error"]["code"], "manifest_invalid");

    git_ok(&fixture.repo, ["add", "patches/fork.yaml"]);
    git_ok(&fixture.repo, ["stash"]);
    fs::write(fixture.repo.join("patches/fork.yaml"), "not: [valid\n").unwrap();
    git_ok(&fixture.repo, ["add", "patches/fork.yaml"]);
    stg_ok_dynamic(&fixture.repo, &["refresh", "--index"]);
    let check = fixture.forkctl(&["--format", "json", "check"]);
    assert!(!check.status.success());
    let check: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(check["error"]["code"], "manifest_invalid");
}

#[test]
fn commit_message_migration_rewrites_the_local_stack_without_publishing() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    make_subjects_legacy(&fixture);
    let old_tip = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    let remote_tip = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);

    let check = fixture.forkctl(&["--format", "json", "check"]);
    assert!(!check.status.success());
    assert!(String::from_utf8_lossy(&check.stdout).contains("migrate-commit-messages"));

    let preview = fixture.forkctl_ok(&[
        "--format",
        "json",
        "contract",
        "migrate-commit-messages",
        "--dry-run",
    ]);
    let preview: serde_json::Value = serde_json::from_str(&preview).unwrap();
    assert_eq!(
        preview["result"]["command"],
        "contract.migrate_commit_messages"
    );
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), old_tip);
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());

    let migrated = fixture.forkctl_ok(&[
        "--format",
        "json",
        "contract",
        "migrate-commit-messages",
        "--source",
        "fix(runtime)",
        "--tooling",
        "chore(fork)",
    ]);
    let migrated: serde_json::Value = serde_json::from_str(&migrated).unwrap();
    assert_eq!(migrated["result"]["old_tip"], old_tip);
    assert_eq!(
        migrated["result"]["rewritten_patches"],
        serde_json::json!(["source-change", "fork-tooling"])
    );
    assert_eq!(
        patch_subject(&fixture.repo, "source-change"),
        "fix(runtime): source change"
    );
    assert_eq!(
        patch_subject(&fixture.repo, "fork-tooling"),
        "chore(fork): fork tooling"
    );
    assert_eq!(migrated["result"]["policy"]["source"]["type"], "fix");
    assert_eq!(migrated["result"]["policy"]["tooling"]["scope"], "fork");
    assert_eq!(
        git_capture(&fixture.repo, ["rev-parse", "origin/main"]),
        remote_tip
    );
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["operation"]["phase"], "ready_to_publish");
    fixture.forkctl_ok(&["check"]);

    let recovery_tag = migrated["result"]["recovery_tag"].as_str().unwrap();
    fixture.forkctl_ok(&["publish"]);
    assert!(
        !git_capture_dynamic(
            &fixture.repo,
            &["ls-remote", "origin", &format!("refs/tags/{recovery_tag}")],
        )
        .is_empty()
    );
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());

    let remigrated =
        fixture.forkctl_ok(&["--format", "json", "contract", "migrate-commit-messages"]);
    let remigrated: serde_json::Value = serde_json::from_str(&remigrated).unwrap();
    assert_eq!(remigrated["result"]["policy"]["source"]["type"], "fix");
    assert_eq!(remigrated["result"]["policy"]["source"]["scope"], "runtime");
    assert_eq!(remigrated["result"]["policy"]["tooling"]["type"], "chore");
    assert_eq!(remigrated["result"]["policy"]["tooling"]["scope"], "fork");
    assert_eq!(
        remigrated["result"]["rewritten_patches"],
        serde_json::json!([])
    );
}

#[test]
fn commit_message_migration_continues_after_a_hook_failure() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    make_subjects_legacy(&fixture);
    let sentinel = fixture.repo.join(".git/fail-commit-message-once");
    fs::write(&sentinel, "fail\n").unwrap();
    let hook = fixture.repo.join(".git/hooks/commit-msg");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nif test -f '{}'; then rm '{}'; exit 1; fi\n",
            sentinel.display(),
            sentinel.display()
        ),
    )
    .unwrap();
    make_executable(&hook);

    let migration = fixture.forkctl(&["contract", "migrate-commit-messages"]);
    assert!(!migration.status.success());
    assert_operation_present(&fixture);

    fixture.forkctl_ok(&["operation", "continue"]);
    assert_eq!(
        patch_subject(&fixture.repo, "source-change"),
        "feat: source change"
    );
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn commit_message_migration_abort_restores_the_exact_old_stack() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    make_subjects_legacy(&fixture);
    let old_tip = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    let old_source = patch_subject(&fixture.repo, "source-change");
    let old_tooling = patch_subject(&fixture.repo, "fork-tooling");
    let hook = fixture.repo.join(".git/hooks/commit-msg");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        "#!/bin/sh\nif grep -q '^chore(tooling):' \"$1\"; then exit 1; fi\n",
    )
    .unwrap();
    make_executable(&hook);

    let migration = fixture.forkctl(&["contract", "migrate-commit-messages"]);
    assert!(!migration.status.success());
    assert_eq!(
        patch_subject(&fixture.repo, "source-change"),
        "feat: source change"
    );
    assert_operation_present(&fixture);
    fixture.forkctl_ok(&["operation", "abort", "--yes"]);

    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), old_tip);
    assert_eq!(patch_subject(&fixture.repo, "source-change"), old_source);
    assert_eq!(patch_subject(&fixture.repo, "fork-tooling"), old_tooling);
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
}

fn make_subjects_legacy(fixture: &Fixture) {
    remove_commit_message_policy(fixture);

    let export_path = fixture
        .repo
        .join("patches/downstream/0001-source-change.patch");
    let export = fs::read_to_string(&export_path).unwrap();
    let (_, body) = export.split_once('\n').unwrap();
    fs::write(&export_path, format!("source-change\n{body}")).unwrap();
    git_ok(
        &fixture.repo,
        [
            "add",
            "patches/fork.yaml",
            "patches/downstream/0001-source-change.patch",
        ],
    );
    stg_ok_dynamic(
        &fixture.repo,
        &["refresh", "--patch", "fork-tooling", "--index"],
    );
    rewrite_patch_subject(&fixture.repo, "source-change", "source-change");
    rewrite_patch_subject(&fixture.repo, "fork-tooling", "fork-tooling");
}

fn remove_commit_message_policy(fixture: &Fixture) {
    let manifest_path = fixture.repo.join("patches/fork.yaml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let policy = "commit_messages:\n  source:\n    type: feat\n  tooling:\n    type: chore\n    scope: tooling\n";
    assert!(manifest.contains(policy));
    fs::write(&manifest_path, manifest.replace(policy, "")).unwrap();
    git_ok(&fixture.repo, ["add", "patches/fork.yaml"]);
    stg_ok_dynamic(
        &fixture.repo,
        &["refresh", "--patch", "fork-tooling", "--index"],
    );
}

fn rewrite_patch_subject(repo: &std::path::Path, patch: &str, subject: &str) {
    let commit = capture(repo, "stg", &["id", patch]);
    let message = capture(repo, "git", &["log", "-1", "--format=%B", &commit]);
    let (_, body) = message.split_once('\n').unwrap();
    let before = capture(repo, "stg", &["series", "--all", "--no-prefix"])
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let position = before
        .iter()
        .position(|candidate| candidate == patch)
        .unwrap();
    stg_ok_dynamic(
        repo,
        &["edit", patch, "--message", &format!("{subject}\n{body}")],
    );
    let after = capture(repo, "stg", &["series", "--all", "--no-prefix"])
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if after[position] != patch {
        stg_ok_dynamic(repo, &["rename", &after[position], patch]);
    }
}

fn patch_subject(repo: &std::path::Path, patch: &str) -> String {
    let commit = capture(repo, "stg", &["id", patch]);
    capture(repo, "git", &["log", "-1", "--format=%s", &commit])
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
    let output = support::isolated_command(repo, "git")
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
    let output = support::isolated_command(repo, program)
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

fn stg_top_commit(repo: &std::path::Path) -> String {
    let top = capture(repo, "stg", &["top"]);
    capture(repo, "stg", &["id", &top])
}

fn stg_ok_dynamic(repo: &std::path::Path, args: &[&str]) {
    let output = support::isolated_command(repo, "stg")
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

fn read_manifest_value(path: &std::path::Path) -> serde_json::Value {
    let contents = fs::read_to_string(path).unwrap();
    serde_saphyr::from_str(&contents).unwrap()
}

fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn rebase_records_replay_path_changes_without_claiming_cause() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "two-file-change",
        "--kind",
        "source",
        "--purpose",
        "Change downstream source.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream provides equivalent behavior.",
        "--scope",
        "kept.txt",
        "--scope",
        "shared.txt",
    ]);
    fs::write(fixture.repo.join("kept.txt"), "downstream only\n").unwrap();
    fs::write(fixture.repo.join("shared.txt"), "shared content\n").unwrap();
    git_ok(&fixture.repo, ["add", "kept.txt", "shared.txt"]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);

    let upstream = git_capture_dynamic(&fixture.repo, &["remote", "get-url", "upstream"]);
    let work = fixture.repo.parent().unwrap().join("absorb-upstream");
    git_ok(
        fixture.repo.parent().unwrap(),
        [
            "clone",
            "--quiet",
            upstream.as_str(),
            work.to_str().unwrap(),
        ],
    );
    git_ok(&work, ["config", "user.name", "Forkctl Test"]);
    git_ok(&work, ["config", "user.email", "forkctl@example.com"]);
    fs::write(work.join("shared.txt"), "shared content\n").unwrap();
    git_ok(&work, ["add", "shared.txt"]);
    git_ok(&work, ["commit", "--quiet", "-m", "upstream adopts file"]);
    git_ok(&work, ["push", "--quiet", "origin", "main"]);

    let response = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(
        response["result"]["path_changed_patches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["two-file-change"]
    );
    let ledger = fs::read_to_string(fixture.repo.join("PATCHES.md")).unwrap();
    assert!(
        ledger.contains("replay path change") && ledger.contains("shared.txt"),
        "ledger lacks replay path-change history:\n{ledger}"
    );
    let manifest = read_manifest_value(&fixture.repo.join("patches/fork.yaml"));
    let path_change = &manifest["history"][0]["path_changes"][0];
    assert_eq!(path_change["patch"], "two-file-change");
    assert_eq!(
        path_change["lost_paths"].as_array().unwrap(),
        &vec![serde_json::Value::from("shared.txt")]
    );
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn staged_check_reports_both_sides_of_a_rename() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "moving-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&[
        "patch",
        "edit",
        "moving-change",
        "--add-scope",
        "renamed.txt",
    ]);
    fixture.forkctl_ok(&["patch", "select", "moving-change"]);
    git_ok(&fixture.repo, ["mv", "source.txt", "renamed.txt"]);
    let staged = fixture.forkctl_ok(&["--format", "json", "check", "--staged"]);
    let staged: serde_json::Value = serde_json::from_str(&staged).unwrap();
    let checked = staged["result"]["checked_paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(checked, vec!["renamed.txt", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh", "--rewrite-below"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn declared_check_catches_an_upstream_case_the_patch_never_covered() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "guarded-spawn",
        "--kind",
        "source",
        "--purpose",
        "Route every terminal spawn through the downstream guard.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream guards terminal spawning.",
        "--scope",
        "crates/terminal/**",
        "--scope",
        "fork-rules/**",
        "--check",
        "unguarded-spawn=ast-grep scan --rule fork-rules/unguarded-spawn.yml {files}",
        "--check-glob",
        "unguarded-spawn=crates/**/*.rs",
    ]);
    fs::create_dir_all(fixture.repo.join("crates/terminal/src")).unwrap();
    fs::create_dir_all(fixture.repo.join("fork-rules")).unwrap();
    fs::write(
        fixture.repo.join("fork-rules/unguarded-spawn.yml"),
        concat!(
            "id: unguarded-spawn\n",
            "language: rust\n",
            "severity: error\n",
            "message: spawn_terminal bypasses the downstream guard\n",
            "rule:\n",
            "  all:\n",
            "    - pattern: spawn_terminal($$$ARGS)\n",
            "    - not:\n",
            "        inside:\n",
            "          kind: function_item\n",
            "          has:\n",
            "            field: name\n",
            "            regex: ^spawn_terminal_guarded$\n",
            "          stopBy: end\n",
        ),
    )
    .unwrap();
    fs::write(
        fixture.repo.join("crates/terminal/src/guard.rs"),
        "pub fn spawn_terminal_guarded(cmd: &str) -> u32 {\n    spawn_terminal(cmd)\n}\n\nfn spawn_terminal(cmd: &str) -> u32 {\n    cmd.len() as u32\n}\n",
    )
    .unwrap();
    git_ok(&fixture.repo, ["add", "crates", "fork-rules"]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    let check = fixture.forkctl_ok(&["--format", "json", "check"]);
    let check: serde_json::Value = serde_json::from_str(&check).unwrap();
    assert_eq!(check["result"]["declared_checks"], 1);

    // Upstream later adds a caller in a crate the patch does not own and that did not exist
    // when the patch was written. The declared check must still notice.
    let upstream = git_capture_dynamic(&fixture.repo, &["remote", "get-url", "upstream"]);
    let work = fixture.repo.parent().unwrap().join("upstream-new-case");
    git_ok(
        fixture.repo.parent().unwrap(),
        [
            "clone",
            "--quiet",
            upstream.as_str(),
            work.to_str().unwrap(),
        ],
    );
    git_ok(&work, ["config", "user.name", "Forkctl Test"]);
    git_ok(&work, ["config", "user.email", "forkctl@example.com"]);
    fs::create_dir_all(work.join("crates/panes/src")).unwrap();
    fs::write(
        work.join("crates/panes/src/split.rs"),
        "pub fn open_split() -> u32 {\n    spawn_terminal(\"zsh\")\n}\n",
    )
    .unwrap();
    git_ok(&work, ["add", "crates"]);
    git_ok(&work, ["commit", "--quiet", "-m", "upstream adds a caller"]);
    git_ok(&work, ["push", "--quiet", "origin", "main"]);

    let failed = fixture.forkctl(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    assert!(!failed.status.success());
    let failed: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(failed["error"]["code"], "check_failed");
    let findings = failed["error"]["details"]["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["code"], "check_command_failed");
    assert_eq!(findings[0]["subject"], "guarded-spawn: unguarded-spawn");
    assert!(
        findings[0]["message"]
            .as_str()
            .unwrap()
            .contains("exited 1"),
        "unexpected finding: {findings:?}"
    );
}

#[test]
fn declared_check_defaults_to_the_declaring_patch_scope() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "scoped-default",
        "--kind",
        "source",
        "--purpose",
        "Check only the files this patch owns.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream owns the files.",
        "--scope",
        "owned/**",
        "--check",
        "owned-only=test \"$(echo {files})\" = owned/kept.txt",
        "--check",
        "wider=test \"$(echo {files} | wc -w)\" -eq 2",
        "--check-glob",
        "wider=**/*.txt",
    ]);
    fs::create_dir_all(fixture.repo.join("owned")).unwrap();
    fs::write(fixture.repo.join("owned/kept.txt"), "downstream\n").unwrap();
    git_ok(&fixture.repo, ["add", "owned"]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    let check = fixture.forkctl_ok(&["--format", "json", "check"]);
    let check: serde_json::Value = serde_json::from_str(&check).unwrap();
    assert_eq!(check["result"]["declared_checks"], 2);
    let ledger = fs::read_to_string(fixture.repo.join("PATCHES.md")).unwrap();
    assert!(
        ledger.contains("| `scoped-default` | `owned-only` | stack | `owned/**` |"),
        "ledger lacks the default-scope check row:\n{ledger}"
    );
}

#[test]
fn declared_checks_never_run_in_the_operator_worktree() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "isolated-checks",
        "--kind",
        "source",
        "--purpose",
        "Prove declared commands are isolated.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Checks are removed.",
        "--scope",
        "owned.txt",
        "--check",
        "stack-isolated=touch stack-leak.txt",
        "--check",
        "patch-isolated=touch patch-leak.txt",
        "--check-at",
        "patch-isolated=patch",
    ]);
    fs::write(fixture.repo.join("owned.txt"), "downstream\n").unwrap();
    git_ok(&fixture.repo, ["add", "owned.txt"]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);

    assert!(!fixture.repo.join("stack-leak.txt").exists());
    assert!(!fixture.repo.join("patch-leak.txt").exists());
}

#[test]
fn layer_check_observes_only_its_own_patch_commit() {
    let fixture = Fixture::new();
    // The lower patch asserts that the higher patch's file is absent, which is only true at its
    // own layer. The same command would fail against the fully applied stack.
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "lower",
        "--kind",
        "source",
        "--purpose",
        "Own the lower file.",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "Upstream owns it.",
        "--scope",
        "lower.txt",
        "--check",
        "higher-absent=test ! -f higher.txt",
        "--check-at",
        "higher-absent=patch",
    ]);
    fs::write(fixture.repo.join("lower.txt"), "lower\n").unwrap();
    git_ok(&fixture.repo, ["add", "lower.txt"]);
    fixture.forkctl_ok(&["patch", "refresh"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    create_source_patch(&fixture, "higher", "higher.txt", "higher\n");
    fixture.forkctl_ok(&["check"]);

    // The same command evaluated against the applied stack fails, and because `patch edit`
    // runs the full check, the invalid declaration is rejected at declaration time.
    let failed = fixture.forkctl(&[
        "--format",
        "json",
        "patch",
        "edit",
        "lower",
        "--clear-checks",
        "--check",
        "higher-absent=test ! -f higher.txt",
    ]);
    assert!(!failed.status.success());
    let failed: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(failed["error"]["code"], "check_failed");
    let findings = failed["error"]["details"]["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["code"], "check_command_failed");
    assert_eq!(findings[0]["subject"], "lower: higher-absent");
    fixture.forkctl_ok(&["operation", "abort", "--yes"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn declared_check_over_no_tracked_file_fails_as_stale() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    let stale = fixture.forkctl(&[
        "--format",
        "json",
        "patch",
        "edit",
        "source-change",
        "--check",
        "vanished=grep -q downstream {files}",
        "--check-glob",
        "vanished=vendor/**/*.rs",
    ]);
    assert!(!stale.status.success());
    let stale: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    let findings = stale["error"]["details"]["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["code"], "check_glob_stale");
    assert!(
        findings[0]["message"]
            .as_str()
            .unwrap()
            .contains("would pass over nothing"),
        "unexpected finding: {findings:?}"
    );
    fixture.forkctl_ok(&["operation", "abort", "--yes"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn check_glob_naming_an_undeclared_check_is_rejected() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    let rejected = fixture.forkctl(&[
        "patch",
        "edit",
        "source-change",
        "--check",
        "present=grep -q downstream {files}",
        "--check-glob",
        "typo=source.txt",
    ]);
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("undeclared check: typo"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn fixture_processes_use_private_home_git_config_and_templates() {
    let fixture = Fixture::new();
    let sandbox = fixture.repo.parent().unwrap().join(".forkctl-test-env");

    let environment = support::isolated_command(&fixture.repo, "sh")
        .args([
            "-c",
            "printf '%s\n%s\n%s\n%s\n' \"$HOME\" \"$XDG_CONFIG_HOME\" \"$LC_ALL\" \"$TZ\"",
        ])
        .output()
        .unwrap();
    assert!(environment.status.success());
    let values = String::from_utf8(environment.stdout).unwrap();
    let values = values.lines().collect::<Vec<_>>();
    assert_eq!(values[0], sandbox.join("home").to_str().unwrap());
    assert_eq!(values[1], sandbox.join("xdg-config").to_str().unwrap());
    assert_eq!(values[2], "C");
    assert_eq!(values[3], "UTC");

    let global = support::isolated_command(&fixture.repo, "git")
        .args(["config", "--global", "--list"])
        .output()
        .unwrap();
    assert!(global.status.success());
    assert!(global.stdout.is_empty(), "global Git config leaked");

    let hooks = fixture.repo.join(".git/hooks");
    assert!(
        !hooks.exists() || fs::read_dir(&hooks).unwrap().next().is_none(),
        "Git's host template directory populated fixture hooks"
    );
}

#[test]
fn json_manifest_is_a_first_class_lifecycle_codec() {
    let fixture = Fixture::new_with_manifest("patches/fork.json");
    let path = fixture.repo.join("patches/fork.json");
    let initial: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(initial["schema"], 1);

    fixture.forkctl_ok(&["contract", "edit", "--allow-base", "vendor/**"]);
    let rewritten = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();
    assert_eq!(value["contracts"]["allow_base"][0], "vendor/**");
    assert!(
        rewritten.starts_with(b"{"),
        "JSON manifest was not preserved"
    );

    let api = fixture.api_call(
        "execute",
        &serde_json::json!({"command":"status","arguments":{}}),
    );
    assert!(api.status.success());
    assert_eq!(api.stderr, &[] as &[u8]);
    let response: serde_json::Value = serde_json::from_slice(&api.stdout).unwrap();
    assert_eq!(response["status"], "success");
    fixture.forkctl_ok(&["check"]);

    fixture.forkctl_ok(&["contract", "migrate-commit-messages"]);
    let rewritten = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();
    assert_eq!(value["commit_messages"]["source"]["type"], "feat");
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn repository_publish_mode_defaults_to_rewrite_and_can_be_changed() {
    let fixture = Fixture::new();
    let status = fixture.forkctl_ok(&["--format", "json", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["publish_mode"], "rewrite");

    fixture.forkctl_ok(&["contract", "edit", "--publish-mode", "append"]);
    let status = fixture.forkctl_ok(&["--format", "json", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["result"]["publish_mode"], "append");
}

#[test]
fn publish_append_keeps_the_previous_tip_as_an_ancestor() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "first\n");
    fixture.forkctl_ok(&["publish"]);
    let first = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);

    fixture.forkctl_ok(&["patch", "select", "source-change"]);
    fs::write(fixture.repo.join("source.txt"), "second\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh", "--rewrite-below"]);
    fixture.forkctl_ok(&["patch", "finish"]);

    let published = fixture.forkctl_ok(&["--format", "json", "publish", "--append"]);
    let published: serde_json::Value = serde_json::from_str(&published).unwrap();
    assert_eq!(published["result"]["mode"], "append");
    assert_eq!(published["result"]["fast_forward"], true);
    assert_eq!(
        published["result"]["recovery_tags"]
            .as_array()
            .unwrap()
            .as_slice(),
        &[] as &[serde_json::Value]
    );
    assert_eq!(
        git_capture(&fixture.repo, ["rev-parse", "HEAD"]),
        stg_top_commit(&fixture.repo)
    );

    let second = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);
    assert_ne!(first, second);
    git_ok(
        &fixture.repo,
        ["merge-base", "--is-ancestor", &first, &second],
    );
    let repeated = fixture.forkctl_ok(&["--format", "json", "publish", "--append"]);
    let repeated: serde_json::Value = serde_json::from_str(&repeated).unwrap();
    assert_eq!(repeated["result"]["already_published"], true);
    assert_eq!(
        git_capture(&fixture.repo, ["rev-parse", "origin/main"]),
        second
    );
}

#[test]
fn publish_append_fresh_clone_hydrates_declared_stack() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "first\n");
    fixture.forkctl_ok(&["publish"]);

    fixture.forkctl_ok(&["patch", "select", "source-change"]);
    fs::write(fixture.repo.join("source.txt"), "second\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh", "--rewrite-below"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["publish", "--append"]);

    let remote = git_capture_dynamic(&fixture.repo, &["remote", "get-url", "origin"]);
    let clone = fixture.repo.parent().unwrap().join("append-fresh-clone");
    git_ok(
        fixture.repo.parent().unwrap(),
        ["clone", "--quiet", remote.as_str(), clone.to_str().unwrap()],
    );
    git_ok(&clone, ["config", "user.name", "Forkctl Test"]);
    git_ok(&clone, ["config", "user.email", "forkctl@example.com"]);

    let output = support::forkctl_command(&clone)
        .args(["--manifest", "patches/fork.yaml", "init"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        stg_capture(&clone, ["series", "--all", "--no-prefix"])
            .lines()
            .collect::<Vec<_>>(),
        ["source-change", "fork-tooling"]
    );
    assert_eq!(
        git_capture(&clone, ["rev-parse", "HEAD"]),
        stg_top_commit(&clone)
    );
}

#[test]
fn publish_append_allows_followup_patch_refresh() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "first\n");
    fixture.forkctl_ok(&["publish"]);

    fixture.forkctl_ok(&["patch", "select", "source-change"]);
    fs::write(fixture.repo.join("source.txt"), "second\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh", "--rewrite-below"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["publish", "--append"]);

    fixture.forkctl_ok(&["patch", "select", "source-change"]);
    fs::write(fixture.repo.join("source.txt"), "third\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh", "--rewrite-below"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn publish_append_restores_stack_after_remote_rejection() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "first\n");
    fixture.forkctl_ok(&["publish"]);

    fixture.forkctl_ok(&["patch", "select", "source-change"]);
    fs::write(fixture.repo.join("source.txt"), "second\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh", "--rewrite-below"]);
    fixture.forkctl_ok(&["patch", "finish"]);
    let stack_tip = stg_top_commit(&fixture.repo);
    let remote = git_capture_dynamic(&fixture.repo, &["remote", "get-url", "origin"]);
    let hook = std::path::Path::new(&remote).join("hooks/pre-receive");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    make_executable(&hook);
    let remote_before = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);

    let publish = fixture.forkctl(&["--format", "json", "publish", "--append"]);
    assert!(!publish.status.success());
    let publish: serde_json::Value = serde_json::from_slice(&publish.stdout).unwrap();
    assert_eq!(publish["error"]["code"], "publication_rejected");
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), stack_tip);
    assert_eq!(
        git_capture(&fixture.repo, ["rev-parse", "HEAD"]),
        stg_top_commit(&fixture.repo)
    );
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"])
            .split_whitespace()
            .next()
            .unwrap(),
        remote_before
    );

    let real_git = isolated_command(&fixture.repo, "sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(real_git.status.success());
    let real_git = String::from_utf8(real_git.stdout).unwrap();
    let wrapper_dir = fixture.repo.parent().unwrap().join("failing-reset-bin");
    fs::create_dir_all(&wrapper_dir).unwrap();
    let wrapper = wrapper_dir.join("git");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nif [ \"$1\" = reset ] && [ \"$2\" = --soft ]; then\n  echo simulated-append-reset-failure >&2\n  exit 1\nfi\nexec \"{}\" \"$@\"\n",
            real_git.trim()
        ),
    )
    .unwrap();
    make_executable(&wrapper);
    let mut command = support::forkctl_command(&fixture.repo);
    command
        .args([
            "--manifest",
            "patches/fork.yaml",
            "--format",
            "json",
            "publish",
            "--append",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                wrapper_dir.display(),
                std::env::var("PATH").unwrap()
            ),
        );
    let failed_restore = command.output().unwrap();
    assert!(!failed_restore.status.success());
    let failed_restore = String::from_utf8(failed_restore.stdout).unwrap();
    assert!(failed_restore.contains("publication_rejected"));
    assert!(failed_restore.contains("simulated-append-reset-failure"));
    assert!(failed_restore.contains("also failed to restore local stack"));
}

#[test]
fn publish_append_retry_completes_an_already_pushed_operation() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&["publish"]);
    let previous_remote = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);

    advance_upstream(&fixture.repo, "upstream v2\n");
    let rebased = fixture.forkctl_ok(&["--format", "json", "rebase", "--onto", "refs/heads/main"]);
    let rebased: serde_json::Value = serde_json::from_str(&rebased).unwrap();
    let recovery_tag = rebased["result"]["recovery_tag"].as_str().unwrap();
    let recovery_ref = format!("refs/tags/{recovery_tag}");
    let stack_tip = stg_top_commit(&fixture.repo);
    let short = &previous_remote[..previous_remote.len().min(12)];
    git_ok_dynamic(
        &fixture.repo,
        &[
            "merge",
            "-s",
            "ours",
            "--no-ff",
            "-m",
            &format!("forkctl: keep published history {short}"),
            &previous_remote,
        ],
    );
    let bridge = git_capture(&fixture.repo, ["rev-parse", "HEAD"]);
    git_ok_dynamic(
        &fixture.repo,
        &[
            "push",
            "--atomic",
            &format!("--force-with-lease=refs/heads/main:{previous_remote}"),
            "origin",
            &format!("{recovery_ref}:{recovery_ref}"),
            "HEAD:refs/heads/main",
        ],
    );
    git_ok_dynamic(&fixture.repo, &["reset", "--soft", &stack_tip]);

    let retry = fixture.forkctl_ok(&["--format", "json", "publish", "--append"]);
    let retry: serde_json::Value = serde_json::from_str(&retry).unwrap();
    assert_eq!(retry["result"]["already_published"], true);
    assert_eq!(retry["result"]["head"], bridge);
    assert_eq!(
        git_capture_dynamic(&fixture.repo, &["ls-remote", "origin", "refs/heads/main"])
            .split_whitespace()
            .next()
            .unwrap(),
        bridge
    );
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), stack_tip);
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
}

#[test]
fn publish_append_retry_rejects_a_bridge_with_the_wrong_previous_tip() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&["publish"]);
    let previous_remote = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);

    advance_upstream(&fixture.repo, "upstream v2\n");
    fixture.forkctl_ok(&["rebase", "--onto", "refs/heads/main"]);
    let stack_tip = stg_top_commit(&fixture.repo);
    let tree = git_capture(&fixture.repo, ["rev-parse", "HEAD^{tree}"]);
    let unrelated_previous = git_capture_dynamic(
        &fixture.repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &previous_remote,
            "-m",
            "unrelated published history",
        ],
    );
    let short = &previous_remote[..previous_remote.len().min(12)];
    let malformed_bridge = git_capture_dynamic(
        &fixture.repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &stack_tip,
            "-p",
            &unrelated_previous,
            "-m",
            &format!("forkctl: keep published history {short}"),
        ],
    );
    git_ok_dynamic(
        &fixture.repo,
        &[
            "push",
            &format!("--force-with-lease=refs/heads/main:{previous_remote}"),
            "origin",
            &format!("{malformed_bridge}:refs/heads/main"),
        ],
    );

    let retry = fixture.forkctl(&["--format", "json", "publish", "--append"]);
    assert!(!retry.status.success());
    let retry: serde_json::Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(retry["error"]["code"], "remote_advanced");
    assert_eq!(git_capture(&fixture.repo, ["rev-parse", "HEAD"]), stack_tip);
    assert_operation_present(&fixture);
}

#[test]
fn publish_append_completes_ready_rebase_operation() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "downstream\n");
    fixture.forkctl_ok(&["publish"]);
    let previous_remote = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);

    advance_upstream(&fixture.repo, "upstream v2\n");
    fixture.forkctl_ok(&["rebase", "--onto", "refs/heads/main"]);
    let published = fixture.forkctl_ok(&["--format", "json", "publish", "--append"]);
    let published: serde_json::Value = serde_json::from_str(&published).unwrap();
    assert_eq!(published["result"]["mode"], "append");
    assert_eq!(published["result"]["fast_forward"], true);
    assert_eq!(
        published["result"]["recovery_tags"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        git_capture(&fixture.repo, ["rev-parse", "HEAD"]),
        stg_top_commit(&fixture.repo)
    );
    let current_remote = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);
    git_ok(
        &fixture.repo,
        [
            "merge-base",
            "--is-ancestor",
            &previous_remote,
            &current_remote,
        ],
    );
    let status = fixture.forkctl_ok(&["--format", "json", "operation", "status"]);
    let status: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert!(status["result"]["operation"].is_null());
    fixture.forkctl_ok(&["check"]);
}

#[test]
fn publish_propose_then_promote_moves_the_lease() {
    let fixture = Fixture::new();
    create_source_patch(&fixture, "source-change", "source.txt", "first\n");
    fixture.forkctl_ok(&["publish"]);
    let first = git_capture(&fixture.repo, ["rev-parse", "origin/main"]);

    fixture.forkctl_ok(&["patch", "select", "source-change"]);
    fs::write(fixture.repo.join("source.txt"), "proposed\n").unwrap();
    git_ok(&fixture.repo, ["add", "source.txt"]);
    fixture.forkctl_ok(&["patch", "refresh", "--rewrite-below"]);
    fixture.forkctl_ok(&["patch", "finish"]);

    let proposed = fixture.forkctl_ok(&["--format", "json", "publish", "--propose"]);
    let proposed: serde_json::Value = serde_json::from_str(&proposed).unwrap();
    assert_eq!(proposed["result"]["mode"], "propose");
    assert_eq!(
        proposed["result"]["proposal_branch"],
        "forkctl/proposal/main"
    );
    assert_eq!(
        git_capture(&fixture.repo, ["rev-parse", "origin/main"]),
        first
    );
    let review = proposed["result"]["head"].as_str().unwrap();
    let review_tree =
        git_capture_dynamic(&fixture.repo, &["rev-parse", &format!("{review}^{{tree}}")]);
    let head_tree = git_capture_dynamic(&fixture.repo, &["rev-parse", "HEAD^{tree}"]);
    assert_eq!(review_tree, head_tree);

    let promoted = fixture.forkctl_ok(&["--format", "json", "publish", "--promote"]);
    let promoted: serde_json::Value = serde_json::from_str(&promoted).unwrap();
    assert_eq!(promoted["result"]["mode"], "rewrite");
    assert_eq!(
        git_capture(&fixture.repo, ["rev-parse", "origin/main"]),
        git_capture(&fixture.repo, ["rev-parse", "HEAD"])
    );
}
