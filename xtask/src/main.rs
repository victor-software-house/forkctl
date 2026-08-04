use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const FORKCTL_TOOL_MARKER: &str = "\"github:victor-software-house/forkctl\" = \"";
const FORKCTL_REF_MARKER: &str = "forkctl.git//tasks/fork?ref=v";
const MIN_MISE_MARKER: &str = "min_version = \"";
const RUST_TOOL_MARKER: &str = "rust = \"";
const STGIT_TOOL_MARKER: &str = "\"cargo:stgit\" = { version = \"";
const LEFTHOOK_TOOL_MARKER: &str = "lefthook = \"";
const USAGE_TOOL_MARKER: &str = "usage = \"";
const GH_TOOL_MARKER: &str = "gh = \"";
const AST_GREP_TOOL_MARKER: &str = "ast-grep = \"";
const RUST_VERSION_MARKER: &str = "rust-version = \"";

struct ToolVersions {
    minimum_mise: String,
    rust: String,
    stgit: String,
    lefthook: String,
    usage: String,
    gh: String,
    ast_grep: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("version-sync: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let check = match env::args().nth(1).as_deref() {
        Some("--check") => true,
        None => false,
        Some(argument) => return Err(format!("unknown argument: {argument}")),
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest has no parent".to_string())?
        .to_owned();
    let version = env!("CARGO_PKG_VERSION");
    let tool_versions = read_tool_versions(&root.join("mise.toml"))?;

    sync_file(&root.join("Cargo.toml"), check, |contents| {
        replace_values(contents, RUST_VERSION_MARKER, &tool_versions.rust, 1)
    })?;
    sync_file(&root.join("tasks/fork/fork"), check, |contents| {
        let contents = replace_values(contents, FORKCTL_TOOL_MARKER, version, 1)?;
        let contents = replace_values(&contents, RUST_TOOL_MARKER, &tool_versions.rust, 1)?;
        replace_values(&contents, STGIT_TOOL_MARKER, &tool_versions.stgit, 1)
    })?;
    for path in [
        root.join("tasks/fork/hooks/install"),
        root.join("tasks/fork/hooks/validate"),
    ] {
        sync_file(&path, check, |contents| {
            replace_values(contents, LEFTHOOK_TOOL_MARKER, &tool_versions.lefthook, 1)
        })?;
    }
    sync_file(&root.join("examples/mise.toml"), check, |contents| {
        let contents = replace_values(contents, MIN_MISE_MARKER, &tool_versions.minimum_mise, 1)?;
        replace_values(&contents, FORKCTL_REF_MARKER, version, 1)
    })?;
    sync_file(&root.join("README.md"), check, |contents| {
        replace_values(contents, MIN_MISE_MARKER, &tool_versions.minimum_mise, 1)
    })?;
    check_lock(&root.join("mise.lock"), &tool_versions)?;
    Ok(())
}

fn read_tool_versions(path: &Path) -> Result<ToolVersions, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    Ok(ToolVersions {
        minimum_mise: read_value(&contents, MIN_MISE_MARKER)?,
        rust: read_value(&contents, RUST_TOOL_MARKER)?,
        stgit: read_value(&contents, STGIT_TOOL_MARKER)?,
        lefthook: read_value(&contents, LEFTHOOK_TOOL_MARKER)?,
        usage: read_value(&contents, USAGE_TOOL_MARKER)?,
        gh: read_value(&contents, GH_TOOL_MARKER)?,
        ast_grep: read_value(&contents, AST_GREP_TOOL_MARKER)?,
    })
}

fn read_value(contents: &str, marker: &str) -> Result<String, String> {
    let matches = contents
        .lines()
        .filter_map(|line| {
            let start = line.find(marker)? + marker.len();
            let end = line[start..].find('"')? + start;
            Some(line[start..end].to_string())
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [value] if !value.is_empty() => Ok(value.clone()),
        _ => Err(format!(
            "marker {marker:?} yielded {} values, expected exactly one",
            matches.len()
        )),
    }
}

fn check_lock(path: &Path, versions: &ToolVersions) -> Result<(), String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    for (tool, expected) in [
        ("ast-grep", versions.ast_grep.as_str()),
        ("cargo:stgit", versions.stgit.as_str()),
        ("gh", versions.gh.as_str()),
        ("lefthook", versions.lefthook.as_str()),
        ("usage", versions.usage.as_str()),
        ("rust", versions.rust.as_str()),
    ] {
        let section = if tool.contains(':') {
            format!("[[tools.\"{tool}\"]]")
        } else {
            format!("[[tools.{tool}]]")
        };
        let start = contents
            .find(&section)
            .ok_or_else(|| format!("{} is missing {section}", path.display()))?;
        let remaining = &contents[start + section.len()..];
        let end = remaining.find("\n[[tools.").unwrap_or(remaining.len());
        let actual = read_value(&remaining[..end], "version = \"")?;
        if actual != expected {
            return Err(format!(
                "{} locks {tool} at {actual}, expected {expected}; run `mise lock`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn sync_file(
    path: &Path,
    check: bool,
    transform: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), String> {
    let original =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let updated = transform(&original)?;
    if updated == original {
        return Ok(());
    }
    if check {
        return Err(format!("{} does not match package version", path.display()));
    }
    fs::write(path, updated).map_err(|error| format!("write {}: {error}", path.display()))
}

fn replace_values(
    contents: &str,
    marker: &str,
    version: &str,
    expected_count: usize,
) -> Result<String, String> {
    let mut count = 0;
    let mut output = String::with_capacity(contents.len());
    for line in contents.split_inclusive('\n') {
        if let Some(start) = line.find(marker) {
            let value_start = start + marker.len();
            let value_end = line[value_start..]
                .find('"')
                .map(|offset| value_start + offset)
                .ok_or_else(|| format!("unterminated version after marker: {marker}"))?;
            output.push_str(&line[..value_start]);
            output.push_str(version);
            output.push_str(&line[value_end..]);
            count += 1;
        } else {
            output.push_str(line);
        }
    }
    if count != expected_count {
        return Err(format!(
            "marker {marker:?} occurred {count} times, expected {expected_count}"
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_the_declared_value() {
        let source = "tool = \"old\"\n";
        assert_eq!(
            replace_values(source, "tool = \"", "0.0.2", 1).unwrap(),
            "tool = \"0.0.2\"\n"
        );
    }

    #[test]
    fn rejects_missing_markers() {
        assert!(replace_values("other = \"x\"\n", "tool = \"", "0.0.2", 1).is_err());
    }

    #[test]
    fn reads_exactly_one_declared_value() {
        assert_eq!(
            read_value("rust = \"expected\"\n", RUST_TOOL_MARKER).unwrap(),
            "expected"
        );
        assert!(read_value("", RUST_TOOL_MARKER).is_err());
        assert!(read_value("rust = \"a\"\nrust = \"b\"\n", RUST_TOOL_MARKER).is_err());
    }
}
