use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

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
    assert!(stdout.contains("Do not put generic executable fork lifecycle or presentation logic"));
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
fn api_schema_describes_versioned_invocation_and_response() {
    let output = forkctl().args(["api", "schema"]).output().unwrap();
    assert!(output.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema["protocol_version"], 1);
    assert_eq!(
        schema["invocation"]["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["response"]["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn clap_and_json_adapters_return_the_same_typed_result() {
    let cli = forkctl()
        .args(["instructions", "--output", "json"])
        .output()
        .unwrap();
    assert!(cli.status.success());

    let invocation = serde_json::json!({
        "protocol_version": 1,
        "request": {"command": "instructions"}
    });
    let mut child = forkctl()
        .args(["api", "call"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&invocation).unwrap().as_bytes())
        .unwrap();
    let api = child.wait_with_output().unwrap();
    assert!(api.status.success());

    let cli: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
    let api: serde_json::Value = serde_json::from_slice(&api.stdout).unwrap();
    assert_eq!(cli["result"], api["result"]);
}

#[test]
fn json_errors_are_schema_shaped_and_do_not_contaminate_stderr() {
    let invocation = serde_json::json!({
        "protocol_version": 2,
        "request": {"command": "instructions"}
    });
    let mut child = forkctl()
        .args(["api", "call"])
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
    assert_eq!(response["protocol_version"], 1);
    assert_eq!(response["error"]["code"], "unsupported_protocol");
}

#[test]
fn json_alias_remains_compatible() {
    let output = forkctl().args(["instructions", "--json"]).output().unwrap();
    assert!(output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "success");
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
