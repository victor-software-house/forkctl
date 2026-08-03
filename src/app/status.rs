use super::App;
use crate::process::capture;
use crate::state::PendingState;
use anyhow::Result;
use serde::Serialize;
use std::env;
use std::io::{self, IsTerminal};

#[derive(Serialize)]
struct Status {
    repository: String,
    current_branch: Option<String>,
    declared_branch: String,
    downstream_remote: String,
    downstream_sha: Option<String>,
    upstream_remote: String,
    upstream_fetch_ref: String,
    base_label: String,
    canonical_base: String,
    stack_base: String,
    applied_patches: Vec<String>,
    unapplied_patches: Vec<String>,
    exports: Vec<String>,
    dirty: Vec<String>,
    pending: Option<PendingState>,
    verification: Check,
}

#[derive(Serialize)]
struct Check {
    ok: bool,
    error: Option<String>,
}

impl App {
    pub fn status(&self, json: bool) -> Result<()> {
        let verification = match self.verify_quiet() {
            Ok(_) => Check {
                ok: true,
                error: None,
            },
            Err(error) => Check {
                ok: false,
                error: Some(format!("{error:#}")),
            },
        };
        let status = Status {
            repository: self.repo.display().to_string(),
            current_branch: self.current_branch().ok(),
            declared_branch: self.manifest.downstream.branch.clone(),
            downstream_remote: self.manifest.downstream.remote.clone(),
            downstream_sha: self.downstream_sha().ok(),
            upstream_remote: self.manifest.upstream.remote.clone(),
            upstream_fetch_ref: self.manifest.upstream.fetch_ref.clone(),
            base_label: self.manifest.base.label.clone(),
            canonical_base: self.manifest.base.canonical.clone(),
            stack_base: self.manifest.base.stack.clone(),
            applied_patches: series(self, "--applied"),
            unapplied_patches: series(self, "--unapplied"),
            exports: self
                .manifest
                .exported_patches()
                .filter_map(|patch| patch.export.clone())
                .collect(),
            dirty: self.dirty_lines()?,
            pending: self.read_pending()?,
            verification,
        };
        if json {
            println!("{}", serde_json::to_string_pretty(&status)?);
        } else {
            print_human(&status);
        }
        Ok(())
    }
}

fn series(app: &App, state: &str) -> Vec<String> {
    capture(&app.repo, "stg", ["series", state, "--no-prefix"])
        .map(|value| {
            value
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn print_human(status: &Status) {
    let color = color_enabled(
        io::stdout().is_terminal(),
        env::var_os("NO_COLOR").is_some(),
    );
    let (bold, green, red, yellow, reset) = if color {
        ("\x1b[1m", "\x1b[32m", "\x1b[31m", "\x1b[33m", "\x1b[0m")
    } else {
        ("", "", "", "", "")
    };
    println!("{bold}forkctl status{reset}");
    println!("repository: {}", status.repository);
    println!(
        "branch: {} (declared {})",
        status.current_branch.as_deref().unwrap_or("detached"),
        status.declared_branch
    );
    println!(
        "downstream: {} {}",
        status.downstream_remote,
        status.downstream_sha.as_deref().unwrap_or("unavailable")
    );
    println!(
        "upstream: {} {}",
        status.upstream_remote, status.upstream_fetch_ref
    );
    println!("base: {} {}", status.base_label, status.stack_base);
    println!("canonical: {}", status.canonical_base);
    println!("applied: {}", display_list(&status.applied_patches));
    println!("unapplied: {}", display_list(&status.unapplied_patches));
    println!("exports: {}", display_list(&status.exports));
    if status.dirty.is_empty() {
        println!("worktree: {green}clean{reset}");
    } else {
        println!(
            "worktree: {yellow}dirty{reset} ({})",
            status.dirty.join(", ")
        );
    }
    if let Some(pending) = &status.pending {
        println!(
            "pending: {yellow}{:?}{reset} lease={} backup={}",
            pending.operation, pending.expected_remote_sha, pending.backup_tag
        );
        if let Some(report) = &pending.report {
            println!("report: {report}");
        }
    } else {
        println!("pending: none");
    }
    if status.verification.ok {
        println!("verification: {green}pass{reset}");
    } else {
        println!(
            "verification: {red}fail{reset}: {}",
            status
                .verification
                .error
                .as_deref()
                .unwrap_or("unknown error")
        );
    }
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn color_enabled(terminal: bool, no_color: bool) -> bool {
    terminal && !no_color
}

#[cfg(test)]
mod tests {
    #[test]
    fn color_policy_requires_terminal_without_no_color() {
        assert!(super::color_enabled(true, false));
        assert!(!super::color_enabled(false, false));
        assert!(!super::color_enabled(true, true));
    }
}
