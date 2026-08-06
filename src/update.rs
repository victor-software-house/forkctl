use crate::process;
use semver::Version;
use serde::Deserialize;
use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const CHECK_INTERVAL: Duration = Duration::from_hours(24);
const REGISTRY_URL: &str = "https://crates.io/api/v1/crates/forkctl";

#[derive(Deserialize)]
struct RegistryResponse {
    #[serde(rename = "crate")]
    package: RegistryPackage,
}

#[derive(Deserialize)]
struct RegistryPackage {
    newest_version: String,
}

pub fn available_notice() -> Option<String> {
    if !io::stderr().is_terminal()
        || env::var_os("FORKCTL_NO_UPDATE_CHECK").is_some()
        || !check_is_due()
    {
        return None;
    }

    let current = current_version()?;
    latest_version()
        .filter(|latest| latest > &current)
        .map(|version| update_message(&version.to_string()))
}

fn latest_version() -> Option<Version> {
    let output = process::capture(
        Path::new("."),
        "curl",
        [
            "--fail",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "1",
            "--max-time",
            "1",
            "--user-agent",
            concat!("forkctl/", env!("CARGO_PKG_VERSION")),
            REGISTRY_URL,
        ],
    )
    .ok()?;
    let response: RegistryResponse = serde_json::from_str(&output).ok()?;
    Version::parse(&response.package.newest_version).ok()
}

fn current_version() -> Option<Version> {
    Version::parse(env!("CARGO_PKG_VERSION")).ok()
}

fn check_is_due() -> bool {
    let path = cache_path();
    if fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < CHECK_INTERVAL)
    {
        return false;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, []);
    true
}

fn cache_path() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(env::temp_dir)
        .join("forkctl/update-check")
}

fn update_message(version: &str) -> String {
    format!(
        "forkctl {version} is available; run `cargo install forkctl --locked` or update the pinned mise catalog"
    )
}

#[cfg(test)]
mod tests {
    use super::update_message;

    #[test]
    fn update_notice_is_short_and_actionable() {
        assert_eq!(
            update_message("1.2.3"),
            "forkctl 1.2.3 is available; run `cargo install forkctl --locked` or update the pinned mise catalog"
        );
    }
}
