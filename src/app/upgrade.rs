use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde_json::Value;

use super::{App, write_atomic};
use crate::error::{AppError, AppResult as Result, DomainError, InternalResultExt as _};
use crate::manifest::SCHEMA;
use crate::manifest_upgrade::{self, EARLIER_SCHEMA};
use crate::protocol::{CommandResult, ExecutionMode, MutationPlan, UpgradeResult};
use crate::state::ActivePatchState;

impl App {
    /// Rewrite a schema 1 manifest in place as the current schema, then
    /// regenerate every generated document and refresh the bookkeeping patch,
    /// as `contract edit` does. A draft active patch is rewritten too. An
    /// operation in progress is refused: its journal belongs to the forkctl
    /// that started it. A schema 2 manifest is refused while the worktree has
    /// uncommitted changes, because an upgrade whose bookkeeping refresh
    /// failed leaves its rewrite uncommitted.
    pub(crate) fn upgrade(&mut self, mode: ExecutionMode) -> Result<CommandResult> {
        let text = fs::read_to_string(&self.manifest_path).map_err(|error| {
            AppError::from(DomainError::manifest_invalid(format!(
                "read {}: {error}",
                self.manifest_path.display()
            )))
        })?;
        let schema =
            manifest_upgrade::declared_schema(self.manifest_format, &text, &self.manifest_path)?;
        if schema == u64::from(SCHEMA) {
            self.manifest()?;
            self.require_clean()?;
            return Ok(CommandResult::Upgrade(UpgradeResult {
                from_schema: SCHEMA,
                to_schema: SCHEMA,
                generated_paths: Vec::new(),
                check: None,
            }));
        }
        let upgraded = manifest_upgrade::upgrade(self.manifest_format, &text, &self.manifest_path)?;
        let manifest = self
            .manifest_format
            .parse(upgraded.as_bytes(), &self.manifest_path)
            .map_err(|error| {
                error.context(format!(
                    "{} does not read as manifest schema {SCHEMA} after the upgrade; an anchor or alias may hide a `kind` key, so expand it by hand",
                    self.manifest_path.display()
                ))
            })?;
        manifest
            .validate(&self.repo, &self.manifest_path)
            .map_err(|error| DomainError::manifest_invalid(error.to_string()))?;
        if self.operation_path()?.exists() {
            return Err(DomainError::operation_conflict(
                "an operation is in progress; finish it with operation continue or operation abort --yes, using the forkctl release that started it, then run mise run fork upgrade",
                None,
            )
            .into());
        }
        self.manifest = Some(manifest);
        self.manifest_error = None;
        self.manifest_text = Some(upgraded);
        self.manifest_edits.clear();
        self.require_clean()?;
        self.require_declared_branch()?;
        let active = self.upgraded_active()?;

        let manifest = self.manifest()?;
        let plan = MutationPlan {
            command: "upgrade".into(),
            reads: vec![self.manifest_path.display().to_string()],
            writes: vec![
                self.manifest_path.display().to_string(),
                manifest.documents.ledger.clone(),
                manifest.documents.exports.clone(),
            ],
            hooks: vec!["pre-commit via bookkeeping refresh".into()],
            ref_updates: Vec::new(),
            paths: Vec::new(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }

        let generated = self.write_upgrade(active.as_deref())?;
        let check = self.check_repository(self.read_active()?.is_some())?;
        Ok(CommandResult::Upgrade(UpgradeResult {
            from_schema: EARLIER_SCHEMA,
            to_schema: SCHEMA,
            generated_paths: generated
                .iter()
                .map(|path| {
                    path.strip_prefix(&self.repo)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            check: Some(check),
        }))
    }

    /// Write the upgraded files and commit them to the bookkeeping patch.
    /// The draft active file is written after the tracked files, so a failed
    /// write leaves only files `git restore` reaches, and before the refresh,
    /// which reads it. `write_atomic` renames into place, so a failed draft
    /// write keeps the old draft whole, and the rerun after the restore
    /// rewrites it.
    fn write_upgrade(&mut self, active: Option<&[u8]>) -> Result<Vec<PathBuf>> {
        let manifest = self.manifest()?;
        let bookkeeping = manifest.bookkeeping_patch.clone();
        let written = [
            self.manifest_path
                .strip_prefix(&self.repo)
                .unwrap_or(&self.manifest_path)
                .display()
                .to_string(),
            manifest.documents.ledger.clone(),
            manifest.documents.exports.clone(),
        ]
        .join(" ");
        let generated = self.write_generated().map_err(|error| {
            error.context(format!(
                "the upgrade stopped while writing {written}; fix the failure, then run git restore --source=HEAD --staged --worktree -- {written}, and run mise run fork upgrade again"
            ))
        })?;
        if let Some(bytes) = active {
            self.active_path()
                .and_then(|path| write_atomic(&path, bytes))
                .map_err(|error| {
                    error.context(format!(
                        "the upgrade stopped while writing the active patch draft; fix the failure, then run git restore --source=HEAD --staged --worktree -- {written}, and run mise run fork upgrade again"
                    ))
                })?;
        }
        self.refresh_bookkeeping(&generated).map_err(|error| {
            error.context(format!(
                "{} is written as schema {SCHEMA} but not committed; fix the failure, then run mise run fork patch select {bookkeeping}, git add the changed files, mise run fork patch refresh, and mise run fork patch finish",
                self.manifest_path.display()
            ))
        })?;
        Ok(generated)
    }

    /// The active-patch file rewritten for the current schema, when it holds a
    /// draft whose metadata names its layer `kind`.
    fn upgraded_active(&self) -> Result<Option<Vec<u8>>> {
        let path = self.active_path()?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(AppError::internal(
                    error,
                    format!("read {}", path.display()),
                ));
            }
        };
        let mut value: Value =
            serde_json::from_slice(&bytes).internal(format!("parse {}", path.display()))?;
        let metadata = value.get_mut("metadata").and_then(Value::as_object_mut);
        let Some(metadata) = metadata else {
            return Ok(None);
        };
        let Some(layer) = metadata.remove("kind") else {
            return Ok(None);
        };
        metadata.insert("layer".into(), layer);
        let state: ActivePatchState = serde_json::from_value(value)
            .internal(format!("parse the upgraded {}", path.display()))?;
        let mut upgraded =
            serde_json::to_vec_pretty(&state).internal(format!("serialize {}", path.display()))?;
        upgraded.push(b'\n');
        Ok(Some(upgraded))
    }
}
