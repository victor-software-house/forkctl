mod support;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use support::{Fixture, isolated_command};

#[derive(Debug, Eq, PartialEq)]
struct RepoSnapshot {
    head: Vec<u8>,
    index_tree: Vec<u8>,
    refs: Vec<u8>,
    stg_series: Vec<u8>,
    worktree: Vec<(String, Vec<u8>)>,
    git_metadata: Vec<(String, Vec<u8>)>,
}

struct FailureCase {
    command: &'static str,
    arguments: serde_json::Value,
    expected_code: &'static str,
    prepare: fn(&Fixture),
}

#[test]
fn every_mutation_has_a_typed_non_mutating_failure_case() {
    let cases = failure_cases();
    let represented = cases
        .iter()
        .map(|case| case.command.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(represented.len(), cases.len(), "duplicate mutation case");

    let schema_commands = schema_commands();
    let read_only = BTreeSet::from([
        "check".to_string(),
        "instructions".to_string(),
        "operation.status".to_string(),
        "patch.list".to_string(),
        "patch.show".to_string(),
        "status".to_string(),
    ]);
    let expected_mutations = schema_commands
        .difference(&read_only)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        represented, expected_mutations,
        "every protocol mutation needs an expected-failure invariant"
    );

    for case in cases {
        let fixture = Fixture::new();
        (case.prepare)(&fixture);
        let before = snapshot(&fixture.repo);
        let output = fixture.api_call(
            "execute",
            &serde_json::json!({
                "command": case.command,
                "arguments": case.arguments,
            }),
        );
        assert!(
            !output.status.success(),
            "{} unexpectedly succeeded:\n{}",
            case.command,
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(output.stderr.is_empty(), "{} wrote stderr", case.command);
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            response["error"]["code"], case.expected_code,
            "{} returned the wrong client error: {response}",
            case.command
        );
        assert_ne!(response["error"]["code"], "internal_error");
        assert_eq!(
            before,
            snapshot(&fixture.repo),
            "{} mutated repository state on failure",
            case.command
        );
    }
}

fn failure_cases() -> Vec<FailureCase> {
    vec![
        case(
            "init",
            serde_json::json!({"upstream_remote":"unexpected"}),
            "invalid_request",
        ),
        case(
            "patch.create",
            serde_json::json!({
                "name":"fork-tooling",
                "kind":"tooling",
                "purpose":"duplicate",
                "upstream_status":"not-submitted",
                "drop_when":"never",
                "scope":["FORK.md"]
            }),
            "invalid_request",
        ),
        case(
            "patch.select",
            serde_json::json!({"patch":"missing"}),
            "patch_not_found",
        ),
        case(
            "patch.edit",
            serde_json::json!({"patch":"missing","purpose":"changed"}),
            "patch_not_found",
        ),
        case(
            "patch.refresh",
            serde_json::json!({"capture":{"source":"staged"}}),
            "active_patch_required",
        ),
        case(
            "patch.finish",
            serde_json::json!({}),
            "active_patch_required",
        ),
        case(
            "patch.remove",
            serde_json::json!({"patch":"missing","reason":"not present"}),
            "patch_not_found",
        ),
        case(
            "patch.disable",
            serde_json::json!({"patch":"missing","reason":"not present"}),
            "patch_not_found",
        ),
        case(
            "patch.enable",
            serde_json::json!({"patch":"missing"}),
            "invalid_request",
        ),
        case(
            "contract.edit",
            serde_json::json!({"clear":false,"allow_base":[],"required_text":[]}),
            "invalid_request",
        ),
        case("rebase", serde_json::json!({"onto":""}), "invalid_request"),
        FailureCase {
            command: "publish",
            arguments: serde_json::json!({}),
            expected_code: "active_patch_exists",
            prepare: create_active_patch,
        },
        case(
            "operation.continue",
            serde_json::json!({}),
            "invalid_request",
        ),
        case(
            "operation.abort",
            serde_json::json!({"confirmed":true}),
            "invalid_request",
        ),
    ]
}

fn case(
    command: &'static str,
    arguments: serde_json::Value,
    expected_code: &'static str,
) -> FailureCase {
    FailureCase {
        command,
        arguments,
        expected_code,
        prepare: |_| {},
    }
}

fn create_active_patch(fixture: &Fixture) {
    fixture.forkctl_ok(&[
        "patch",
        "create",
        "draft",
        "--kind",
        "source",
        "--purpose",
        "unpublished draft",
        "--upstream-status",
        "not-submitted",
        "--drop-when",
        "never",
        "--scope",
        "base.txt",
    ]);
}

fn schema_commands() -> BTreeSet<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_forkctl"))
        .env("FORKCTL_NO_UPDATE_CHECK", "1")
        .args(["api", "schema", "--kind", "invocation"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut commands = BTreeSet::new();
    collect_commands(&schema, &mut commands);
    commands
}

fn snapshot(repo: &Path) -> RepoSnapshot {
    let git_dir = PathBuf::from(capture_ok(repo, "git", &["rev-parse", "--git-dir"]));
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        repo.join(git_dir)
    };
    let mut worktree = Vec::new();
    snapshot_files(repo, repo, Some(".git"), &mut worktree);
    let mut git_metadata = Vec::new();
    snapshot_files(&git_dir, &git_dir, Some("objects"), &mut git_metadata);
    RepoSnapshot {
        head: capture_ok_bytes(repo, "git", &["rev-parse", "HEAD"]),
        index_tree: capture_ok_bytes(repo, "git", &["write-tree"]),
        refs: capture_ok_bytes(repo, "git", &["show-ref", "--head", "--dereference"]),
        stg_series: capture_ok_bytes(repo, "stg", &["series", "--all", "--no-prefix"]),
        worktree,
        git_metadata,
    }
}

fn capture_ok(dir: &Path, program: &str, args: &[&str]) -> String {
    String::from_utf8(capture_ok_bytes(dir, program, args))
        .unwrap()
        .trim()
        .to_string()
}

fn capture_ok_bytes(dir: &Path, program: &str, args: &[&str]) -> Vec<u8> {
    let output = isolated_command(dir, program).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{program} {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn snapshot_files(
    root: &Path,
    directory: &Path,
    excluded_entry: Option<&str>,
    files: &mut Vec<(String, Vec<u8>)>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut paths = entries
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if path.file_name().and_then(OsStr::to_str) == excluded_entry {
            continue;
        }
        if path.is_dir() {
            snapshot_files(root, &path, excluded_entry, files);
        } else {
            files.push((
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                fs::read(path).unwrap(),
            ));
        }
    }
}

fn collect_commands(value: &serde_json::Value, commands: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(command) = object
                .get("properties")
                .and_then(|properties| properties.get("command"))
                .and_then(|command| command.get("const"))
                .and_then(serde_json::Value::as_str)
            {
                commands.insert(command.to_string());
            }
            for value in object.values() {
                collect_commands(value, commands);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_commands(value, commands);
            }
        }
        _ => {}
    }
}
