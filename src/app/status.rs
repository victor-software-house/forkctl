use super::App;
use crate::process::capture;
use crate::protocol::{StatusResult, VerificationStatus};
use anyhow::Result;

impl App {
    pub fn status(&self) -> Result<StatusResult> {
        let verification = match self.verify_quiet() {
            Ok(_) => VerificationStatus {
                ok: true,
                error: None,
            },
            Err(error) => VerificationStatus {
                ok: false,
                error: Some(format!("{error:#}")),
            },
        };
        Ok(StatusResult {
            repository: self.repo.display().to_string(),
            current_branch: self.current_branch().ok(),
            declared_branch: self.manifest.downstream.branch.clone(),
            downstream_remote: self.manifest.downstream.remote.clone(),
            downstream_sha: self.downstream_sha().ok(),
            upstream_remote: self.manifest.upstream.remote.clone(),
            upstream_fetch_ref: self.manifest.upstream.fetch_ref.clone(),
            selected_target: self.manifest.base.target.selector.clone(),
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
        })
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
