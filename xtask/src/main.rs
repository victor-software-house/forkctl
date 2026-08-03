use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const TOOL_MARKER: &str = "\"github:victor-software-house/forkctl\" = \"";
const REF_MARKER: &str = "forkctl.git//tasks/fork.toml?ref=v";

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

    sync_file(&root.join("tasks/fork.toml"), check, |contents| {
        replace_values(contents, TOOL_MARKER, version, 6)
    })?;
    sync_file(&root.join("examples/mise.toml"), check, |contents| {
        replace_values(contents, REF_MARKER, version, 1)
    })?;
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
}
