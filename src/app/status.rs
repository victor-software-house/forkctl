use super::App;
use crate::protocol::{CheckSummary, PatchSummary, StatusResult};
use anyhow::Result;

impl App {
    pub fn status(&self) -> Result<StatusResult> {
        let manifest = self.manifest()?;
        let check = match self.check_repository(false) {
            Ok(_) => CheckSummary {
                ok: true,
                message: None,
            },
            Err(error) => CheckSummary {
                ok: false,
                message: Some(format!("{error:#}")),
            },
        };
        let inventory = self.worktree_inventory()?;
        let active = self.read_active()?;
        let applied = series(self, "--applied");
        let unapplied = series(self, "--unapplied");
        let patches = manifest
            .patches
            .iter()
            .map(|patch| PatchSummary {
                name: patch.name.clone(),
                kind: patch.kind,
                state: if applied.contains(&patch.name) {
                    "applied".into()
                } else if unapplied.contains(&patch.name) {
                    "unapplied".into()
                } else {
                    "missing".into()
                },
                commit: self.patch_commit(&patch.name).ok(),
                active: active
                    .as_ref()
                    .is_some_and(|value| value.name() == patch.name),
            })
            .collect();
        Ok(StatusResult {
            repository: self.repo.display().to_string(),
            manifest: self.manifest_path.display().to_string(),
            current_branch: self.current_branch().ok(),
            declared_branch: manifest.downstream.branch.clone(),
            downstream_remote: manifest.downstream.remote.clone(),
            downstream_sha: self.downstream_sha().ok(),
            upstream_remote: manifest.upstream.remote.clone(),
            upstream_fetch_ref: manifest.upstream.fetch_ref.clone(),
            selected_target: manifest.base.target.selector.clone(),
            canonical_base: manifest.base.canonical.clone(),
            stack_base: manifest.base.stack.clone(),
            patches,
            active_patch: active,
            staged: inventory.staged,
            unstaged: inventory.unstaged,
            untracked: inventory.untracked,
            operation: self.read_operation()?,
            check,
        })
    }
}

fn series(app: &App, state: &str) -> Vec<String> {
    crate::process::capture(&app.repo, "stg", ["series", state, "--no-prefix"])
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
