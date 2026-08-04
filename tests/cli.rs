mod support;

use std::io::Write;
use std::process::{Command, Stdio};
use support::Fixture;

fn forkctl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_forkctl"))
}

fn complete_in(directory: &std::path::Path, words: &[&str], index: usize) -> std::process::Output {
    forkctl()
        .env("COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", index.to_string())
        .arg("--")
        .args(words)
        .current_dir(directory)
        .output()
        .unwrap()
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
fn instructions_work_outside_a_repository() {
    let directory = tempfile::tempdir().unwrap();
    let output = forkctl()
        .arg("instructions")
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("forkctl")
    );
}

#[test]
fn schema_bundle_exposes_all_contracts() {
    let output = forkctl()
        .args(["api", "schema", "--kind", "bundle"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    for key in [
        "manifest",
        "invocation",
        "response",
        "active_state",
        "operation",
    ] {
        assert!(schema["schemas"][key].is_object(), "missing {key}");
    }
}

#[test]
fn usage_spec_contains_full_mounted_grammar() {
    let output = forkctl().arg("--usage-spec=fork").output().unwrap();
    assert!(output.status.success());
    let spec = String::from_utf8(output.stdout).unwrap();
    assert!(spec.contains("name fork"));
    assert!(spec.contains("cmd patch"));
    assert!(spec.contains("cmd refresh"));
    assert!(spec.contains("flag \"-s --staged\""));
    assert!(spec.contains("complete patch run=\"mise run --quiet fork -- __candidates patch\""));
    assert!(spec.contains("complete onto run=\"mise run --quiet fork -- __candidates ref\""));
    assert!(
        spec.contains(
            "complete upstream_remote run=\"mise run --quiet fork -- __candidates remote\""
        )
    );
}

#[test]
fn completion_registration_covers_every_supported_shell() {
    for (shell, marker) in [
        ("bash", "COMPLETE=\"bash\""),
        ("elvish", "COMPLETE=\"elvish\""),
        ("fish", "COMPLETE=fish"),
        ("nu", "usage complete-word"),
        ("powershell", "$env:COMPLETE = \"powershell\""),
        ("zsh", "COMPLETE=\"zsh\""),
    ] {
        let output = forkctl().args(["completion", shell]).output().unwrap();
        assert!(output.status.success(), "completion failed for {shell}");
        let script = String::from_utf8(output.stdout).unwrap();
        assert!(
            script.contains(marker),
            "{shell} registration lacks {marker:?}"
        );
    }
}

#[test]
fn direct_completion_includes_live_patch_names() {
    let fixture = Fixture::new();
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "live-completion-patch",
        "--kind",
        "source",
        "--purpose",
        "completion proof",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "upstream accepts it",
        "--scope",
        "src/**",
    ]);
    let output = complete_in(&fixture.repo, &["forkctl", "patch", "show", "live"], 3);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "live-completion-patch"
    );
}

#[test]
fn direct_completion_includes_local_refs_and_remotes() {
    let fixture = Fixture::new();

    let refs = complete_in(
        &fixture.repo,
        &["forkctl", "rebase", "--onto", "refs/heads/m"],
        3,
    );
    assert!(refs.status.success());
    assert!(
        String::from_utf8(refs.stdout)
            .unwrap()
            .lines()
            .any(|value| value == "refs/heads/main")
    );

    let remotes = complete_in(
        &fixture.repo,
        &["forkctl", "init", "--upstream-remote", "up"],
        3,
    );
    assert!(remotes.status.success());
    assert!(
        String::from_utf8(remotes.stdout)
            .unwrap()
            .lines()
            .any(|value| value == "upstream")
    );
}

#[test]
fn api_domain_errors_are_typed_at_the_boundary() {
    let fixture = Fixture::new();

    let missing = fixture.api_call(
        "execute",
        &serde_json::json!({
            "command":"patch.select",
            "arguments":{"patch":"missing-patch"}
        }),
    );
    assert!(!missing.status.success());
    let missing: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(missing["error"]["code"], "patch_not_found");
    assert_eq!(missing["error"]["details"]["type"], "patch");
    assert_eq!(missing["error"]["details"]["requested"], "missing-patch");

    std::fs::write(fixture.repo.join("outside.txt"), "outside\n").unwrap();
    support::git_ok(&fixture.repo, ["add", "outside.txt"]);
    let staged = fixture.api_call(
        "execute",
        &serde_json::json!({
            "command":"check",
            "arguments":{"scope":"staged","patch":"fork-tooling"}
        }),
    );
    assert!(!staged.status.success());
    let staged: serde_json::Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(staged["error"]["code"], "staged_scope_violation");
    assert_eq!(staged["error"]["details"]["type"], "paths");
    assert_eq!(staged["error"]["details"]["patch"], "fork-tooling");
    assert_eq!(staged["error"]["details"]["paths"][0], "outside.txt");
}

#[test]
fn repository_discovery_failure_is_typed() {
    let directory = tempfile::tempdir().unwrap();
    let invocation = serde_json::json!({
        "protocol_version": 1,
        "mode": "execute",
        "request": {"command":"status","arguments":{}}
    });
    let mut child = forkctl()
        .args(["api", "call"])
        .current_dir(directory.path())
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
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["error"]["code"], "repository_not_found");
}

#[test]
fn dynamic_candidates_fail_silently_outside_a_repository() {
    let directory = tempfile::tempdir().unwrap();
    for kind in ["patch", "ref", "remote"] {
        let output = forkctl()
            .args(["__candidates", kind])
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "candidate lookup failed for {kind}"
        );
        assert!(
            output.stdout.is_empty(),
            "candidate lookup printed for {kind}"
        );
        assert!(
            output.stderr.is_empty(),
            "candidate lookup warned for {kind}"
        );
    }
}

#[test]
fn short_and_long_check_flags_have_api_parity() {
    let fixture = Fixture::new();
    let short = fixture.forkctl(&["--format", "json", "check", "-s"]);
    let long = fixture.forkctl(&["--format", "json", "check", "--staged"]);
    assert!(short.status.success());
    assert_eq!(short.stdout, long.stdout);
}

#[test]
fn cli_and_api_execute_the_same_status_handler() {
    let fixture = Fixture::new();
    let cli = fixture.forkctl(&["--format", "json", "status"]);
    assert!(cli.status.success());
    let api = fixture.api_call(
        "execute",
        &serde_json::json!({"command":"status","arguments":{}}),
    );
    assert!(api.status.success());
    assert!(api.stderr.is_empty());
    let cli: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
    let api: serde_json::Value = serde_json::from_slice(&api.stdout).unwrap();
    assert_eq!(cli["result"], api["result"]);
}

#[test]
fn domain_failure_is_one_json_error_with_empty_stderr() {
    let directory = tempfile::tempdir().unwrap();
    let invocation = serde_json::json!({
        "protocol_version": 1,
        "mode": "execute",
        "request": {"command":"check","arguments":{"scope":"repository"}}
    });
    let mut child = forkctl()
        .args(["api", "call"])
        .current_dir(directory.path())
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
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "error");
    assert_eq!(response["command"], "check");
}

#[test]
fn old_command_and_aliases_are_rejected() {
    for args in [vec!["verify"], vec!["new", "x"], vec!["--json", "status"]] {
        let output = forkctl().args(args).output().unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn read_only_command_rejects_plan_mode() {
    let fixture = Fixture::new();
    let output = fixture.api_call(
        "plan",
        &serde_json::json!({"command":"status","arguments":{}}),
    );
    assert!(!output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["error"]["code"], "invalid_request");
    assert_eq!(response["error"]["details"]["type"], "request");
}
