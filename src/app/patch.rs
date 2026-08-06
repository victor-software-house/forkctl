use super::App;
use crate::error::DomainError;
use crate::manifest::{DisabledPatch, HistoryEvent, Patch};
use crate::process::{capture, run};
use crate::protocol::{
    CaptureSource, CheckEdit, CommandResult, ExecutionMode, MutationPlan, PatchCreateArgs,
    PatchCreateResult, PatchEditArgs, PatchEditResult, PatchFinishResult, PatchListResult,
    PatchRefreshArgs, PatchRefreshResult, PatchSelectResult, PatchShowResult, PatchSummary,
    PatchTarget, PatchTransitionArgs, PatchTransitionResult, ScopeEdit,
};

use crate::state::{ActivePatchState, OperationIntent, OperationKind, OperationState};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

impl App {
    pub fn patch_list(&self) -> Result<PatchListResult> {
        let active = self.read_active()?;
        let applied = series(self, "--applied");
        let unapplied = series(self, "--unapplied");
        let patches = self
            .manifest()?
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
            .chain(
                self.manifest()?
                    .disabled_patches
                    .iter()
                    .map(|record| PatchSummary {
                        name: record.patch.name.clone(),
                        kind: record.patch.kind,
                        state: "disabled".into(),
                        commit: Some(record.commit.clone()),
                        active: false,
                    }),
            )
            .collect();
        Ok(PatchListResult {
            patches,
            active_patch: active,
        })
    }

    pub fn patch_show(&self, target: &PatchTarget) -> Result<PatchShowResult> {
        if let Some(name) = target.patch.as_deref()
            && let Some(record) = self
                .manifest()?
                .disabled_patches
                .iter()
                .find(|record| record.patch.name == name)
        {
            return Ok(PatchShowResult {
                patch: record.patch.clone(),
                commit: Some(record.commit.clone()),
                changed_paths: self.patch_paths(&record.commit)?,
                export: None,
                active: false,
            });
        }
        let (name, patch) = self.resolve_patch(target.patch.as_deref())?;
        let active = self
            .read_active()?
            .is_some_and(|value| value.name() == name);
        let commit = self.patch_commit(&name).ok();
        let changed_paths = commit
            .as_deref()
            .map_or_else(|| Ok(Vec::new()), |commit| self.patch_paths(commit))?;
        let export = self
            .manifest()?
            .patches
            .iter()
            .position(|candidate| candidate.name == name)
            .and_then(|index| self.manifest().ok()?.export_path(index, &patch));
        Ok(PatchShowResult {
            patch,
            commit,
            changed_paths,
            export,
            active,
        })
    }

    pub fn patch_create(
        &self,
        args: PatchCreateArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        let patch: Patch = args.into();
        patch
            .validate()
            .map_err(|error| DomainError::invalid_request(error.to_string()))?;
        if self.manifest()?.patch(&patch.name).is_some() {
            return Err(DomainError::invalid_request(format!(
                "patch already exists: {}",
                patch.name
            ))
            .into());
        }
        let state = ActivePatchState::Draft { metadata: patch };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(MutationPlan {
                command: "patch.create".into(),
                reads: vec![self.manifest_path.display().to_string()],
                writes: vec![self.active_path()?.display().to_string()],
                hooks: Vec::new(),
                ref_updates: Vec::new(),
                paths: Vec::new(),
                requires_confirmation: false,
            }));
        }
        self.write_active(&state)?;
        Ok(CommandResult::PatchCreate(PatchCreateResult {
            active_patch: state,
        }))
    }

    pub fn patch_select(&self, name: &str, mode: ExecutionMode) -> Result<CommandResult> {
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        if self.manifest()?.patch(name).is_none() {
            return Err(DomainError::patch_not_found(
                name,
                self.manifest()?.patch_names(),
                self.read_active()?.map(|active| active.name().to_string()),
            )
            .into());
        }
        let previous = self.read_active()?;
        let active = ActivePatchState::Existing {
            patch: name.to_string(),
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(MutationPlan {
                command: "patch.select".into(),
                reads: vec![self.manifest_path.display().to_string()],
                writes: vec![self.active_path()?.display().to_string()],
                hooks: Vec::new(),
                ref_updates: Vec::new(),
                paths: Vec::new(),
                requires_confirmation: false,
            }));
        }
        self.write_active(&active)?;
        Ok(CommandResult::PatchSelect(PatchSelectResult {
            previous,
            active_patch: active,
        }))
    }

    pub fn patch_edit(
        &mut self,
        args: PatchEditArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        let (name, mut patch) = self.resolve_patch(args.patch.as_deref())?;
        if self.manifest()?.patch(&name).is_none() {
            return Err(DomainError::invalid_request(
                "draft metadata is edited by recreating the draft",
            )
            .into());
        }
        let old_commit = self.patch_commit(&name)?;
        if let Some(kind) = args.kind {
            patch.kind = kind;
        }
        if let Some(value) = args.purpose {
            patch.purpose = value;
        }
        if let Some(value) = args.upstream_status {
            patch.upstream_status = value;
        }
        if let Some(value) = args.drop_when {
            patch.drop_when = value;
        }
        if let Some(scope) = args.scope {
            match scope {
                ScopeEdit::Set { patterns } => patch.scope = patterns,
                ScopeEdit::AddRemove { add, remove } => {
                    let remove = remove.into_iter().collect::<HashSet<_>>();
                    patch.scope.retain(|pattern| !remove.contains(pattern));
                    for pattern in add {
                        if !patch.scope.contains(&pattern) {
                            patch.scope.push(pattern);
                        }
                    }
                }
            }
        }
        if let Some(edit) = args.checks {
            apply_check_edit(&mut patch, edit);
        }
        patch
            .validate()
            .map_err(|error| DomainError::invalid_request(error.to_string()))?;
        let old_kind = self.manifest()?.patch(&name).expect("patch exists").kind;
        let proposed = self.proposed_manifest_with_patch(patch.clone(), old_kind)?;
        let plan = MutationPlan {
            command: "patch.edit".into(),
            reads: vec![self.manifest_path.display().to_string(), old_commit.clone()],
            writes: vec![name.clone(), self.manifest_path.display().to_string()],
            hooks: vec!["commit-msg via stg edit".into()],
            ref_updates: Vec::new(),
            paths: patch.scope.clone(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        let mut operation = self.create_operation(OperationKind::PatchEdit, None)?;
        operation.phase = "editing".into();
        operation.intent = Some(OperationIntent::Edit {
            patch: patch.clone(),
        });
        self.write_operation(&operation)?;
        let mutation = if old_kind == patch.kind {
            run(
                &self.repo,
                "stg",
                ["edit", &name, "--message", &patch.message()],
            )
        } else {
            self.reorder_series(&proposed).and_then(|()| {
                run(
                    &self.repo,
                    "stg",
                    ["edit", &name, "--message", &patch.message()],
                )
            })
        };
        if let Err(error) = mutation {
            operation.phase = "conflict".into();
            operation.next_actions = vec![
                "resolve conflict content".into(),
                "git add --update".into(),
                "forkctl operation continue".into(),
            ];
            self.write_operation(&operation)?;
            return Err(error).context(format!(
                "patch edit stopped; recovery tag {}; resolve the StGit operation and continue",
                operation.recovery.tag
            ));
        }
        self.finish_patch_edit(&operation, proposed, patch, old_commit)
    }

    pub(super) fn continue_patch_edit(
        &mut self,
        operation: &OperationState,
        patch: Patch,
    ) -> Result<CommandResult> {
        // The manifest still holds the pre-edit entry, so its kind is the old kind.
        let old_kind = self
            .manifest()?
            .patch(&patch.name)
            .map_or(patch.kind, |existing| existing.kind);
        let proposed = self.proposed_manifest_with_patch(patch.clone(), old_kind)?;
        let conflicts = capture(
            &self.repo,
            "git",
            ["diff", "--name-only", "--diff-filter=U"],
        )?;
        if !conflicts.is_empty() {
            return Err(DomainError::operation_conflict(
                format!(
                    "unresolved paths remain: {}",
                    conflicts.lines().collect::<Vec<_>>().join(", ")
                ),
                Some(operation),
            )
            .into());
        }
        if !crate::process::succeeds(&self.repo, "git", ["diff", "--cached", "--quiet"])? {
            run(&self.repo, "stg", ["refresh", "--index"])?;
        }
        if capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? != "0"
            && let Err(error) = run(&self.repo, "stg", ["push", "--all"])
        {
            return Err(error).context(
                "patch edit continuation stopped while restoring applied patches; resolve and continue again",
            );
        }
        let expected = proposed.patch_names();
        if self.stg_series()? != expected {
            return Err(DomainError::operation_conflict(
                "patch edit is incomplete; finish the StGit operation before continuing",
                Some(operation),
            )
            .into());
        }
        let old_commit = operation
            .old_patches
            .iter()
            .find(|evidence| evidence.name == patch.name)
            .map(|evidence| evidence.commit.clone())
            .ok_or_else(|| {
                DomainError::operation_conflict(
                    "patch edit journal has no old commit evidence",
                    Some(operation),
                )
            })?;
        self.finish_patch_edit(operation, proposed, patch, old_commit)
    }

    fn finish_patch_edit(
        &mut self,
        operation: &OperationState,
        proposed: crate::manifest::Manifest,
        patch: Patch,
        old_commit: String,
    ) -> Result<CommandResult> {
        self.manifest = Some(proposed);
        self.write_manifest()?;
        let exports = self.write_exports()?;
        let ledger = self.write_ledger()?;
        let generated = std::iter::once(self.manifest_path.clone())
            .chain(std::iter::once(ledger))
            .chain(exports)
            .collect::<Vec<_>>();
        self.refresh_bookkeeping(&generated)?;
        let new_commit = self.patch_commit(&patch.name)?;
        let check = self.check_repository(true)?;
        self.complete_local_operation(operation)?;
        Ok(CommandResult::PatchEdit(PatchEditResult {
            patch,
            old_commit,
            new_commit,
            generated_paths: display_paths(&self.repo, &generated),
            check,
        }))
    }

    fn proposed_manifest_with_patch(
        &self,
        patch: Patch,
        old_kind: crate::manifest::PatchKind,
    ) -> Result<crate::manifest::Manifest> {
        let mut proposed = self.manifest()?.clone();
        let old_index = proposed
            .patches
            .iter()
            .position(|candidate| candidate.name == patch.name)
            .ok_or_else(|| {
                DomainError::patch_not_found(
                    &patch.name,
                    proposed.patch_names(),
                    self.read_active()
                        .ok()
                        .flatten()
                        .map(|active| active.name().to_string()),
                )
            })?;
        proposed.patches.remove(old_index);
        // Only a kind change repositions a patch; an in-place edit must not move it, or the
        // declared order would diverge from the series StGit still holds.
        let index = if old_kind == patch.kind {
            old_index
        } else {
            proposed.insertion_index(&patch)
        };
        proposed.patches.insert(index, patch);
        proposed
            .validate(&self.repo, &self.manifest_path)
            .map_err(|error| DomainError::invalid_request(error.to_string()))?;
        Ok(proposed)
    }

    fn reorder_series(&self, manifest: &crate::manifest::Manifest) -> Result<()> {
        let mut series = tempfile::NamedTempFile::new_in(&self.repo)?;
        for patch in &manifest.patches {
            writeln!(series, "{}", patch.name)?;
        }
        series.flush()?;
        run(
            &self.repo,
            "stg",
            [
                std::ffi::OsStr::new("float"),
                std::ffi::OsStr::new("--series"),
                series.path().as_os_str(),
            ],
        )
    }

    pub fn patch_refresh(
        &mut self,
        args: PatchRefreshArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        self.require_declared_branch()?;
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        let active = self
            .read_active()?
            .ok_or_else(DomainError::active_patch_required)?;
        let (name, patch) = self.resolve_patch(args.patch.as_deref())?;
        if active.name() != name {
            return Err(DomainError::invalid_request(format!(
                "requested patch {name} is not active"
            ))
            .into());
        }
        let capture_paths = self.capture_paths(&patch, &args.capture)?;
        if capture_paths.is_empty() {
            return Err(DomainError::capture_conflict("no changes selected for capture").into());
        }
        let plan = MutationPlan {
            command: "patch.refresh".into(),
            reads: vec![
                "Git index/worktree".into(),
                self.manifest_path.display().to_string(),
            ],
            writes: vec![
                name.clone(),
                self.manifest_path.display().to_string(),
                self.manifest()?.documents.ledger.clone(),
            ],
            hooks: vec!["pre-commit via stg refresh".into()],
            ref_updates: Vec::new(),
            paths: capture_paths.clone(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        let old_commit = self.patch_commit(&name).ok();
        let mut operation = self.create_operation(OperationKind::PatchRefresh, None)?;
        operation.phase = "refreshing".into();
        operation.intent = Some(OperationIntent::Refresh {
            patch: patch.clone(),
            capture: args.capture.clone(),
            captured_paths: capture_paths.clone(),
        });
        self.write_operation(&operation)?;
        let mutation = self
            .stage_capture(&capture_paths, &args.capture)
            .and_then(|()| {
                if matches!(active, ActivePatchState::Draft { .. }) {
                    let insertion = self.manifest()?.insertion_index(&patch);
                    run(
                        &self.repo,
                        "stg",
                        [
                            "new",
                            "--message",
                            &patch.message(),
                            "--refresh",
                            "--index",
                            &name,
                        ],
                    )?;
                    let target = self.manifest()?.patches[insertion].name.clone();
                    run(&self.repo, "stg", ["sink", "--below", &target, &name])
                } else {
                    run(&self.repo, "stg", ["refresh", "--patch", &name, "--index"])
                }
            });
        if let Err(error) = mutation {
            operation.phase = "conflict".into();
            operation.next_actions = vec![
                "resolve conflict content".into(),
                "git add --update".into(),
                "forkctl operation continue".into(),
            ];
            self.write_operation(&operation)?;
            return Err(error).context(format!(
                "patch refresh stopped; recovery tag {}; resolve the StGit operation and continue",
                operation.recovery.tag
            ));
        }
        self.finish_patch_refresh(&operation, patch, args.capture, capture_paths, old_commit)
    }

    pub(super) fn continue_patch_refresh(
        &mut self,
        operation: &OperationState,
        patch: Patch,
        capture_source: CaptureSource,
        captured_paths: Vec<String>,
    ) -> Result<CommandResult> {
        let conflicts = capture(
            &self.repo,
            "git",
            ["diff", "--name-only", "--diff-filter=U"],
        )?;
        if !conflicts.is_empty() {
            return Err(DomainError::operation_conflict(
                format!(
                    "unresolved paths remain: {}",
                    conflicts.lines().collect::<Vec<_>>().join(", ")
                ),
                Some(operation),
            )
            .into());
        }
        if !crate::process::succeeds(&self.repo, "git", ["diff", "--cached", "--quiet"])? {
            run(&self.repo, "stg", ["refresh", "--index"])?;
        }
        let series = self.stg_series()?;
        let temporary = series
            .iter()
            .find(|name| name.starts_with("refresh-temp"))
            .cloned();
        if let Some(temporary) = temporary
            && let Err(error) = run(
                &self.repo,
                "stg",
                [
                    "squash",
                    "--name",
                    &patch.name,
                    "--message",
                    &patch.message(),
                    &patch.name,
                    &temporary,
                ],
            )
        {
            return Err(error).context(
                "refresh continuation stopped while squashing StGit's temporary patch; resolve and continue again",
            );
        }
        if capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? != "0"
            && let Err(error) = run(&self.repo, "stg", ["push", "--all"])
        {
            return Err(error).context(
                "refresh continuation stopped while restoring applied patches; resolve and continue again",
            );
        }
        if capture(&self.repo, "stg", ["top"])? != self.manifest()?.bookkeeping_patch {
            return Err(DomainError::operation_conflict(
                "patch refresh is incomplete; restore the bookkeeping patch before continuing",
                Some(operation),
            )
            .into());
        }
        let old_commit = operation
            .old_patches
            .iter()
            .find(|evidence| evidence.name == patch.name)
            .map(|evidence| evidence.commit.clone());
        self.finish_patch_refresh(operation, patch, capture_source, captured_paths, old_commit)
    }

    fn finish_patch_refresh(
        &mut self,
        operation: &OperationState,
        patch: Patch,
        capture: CaptureSource,
        captured_paths: Vec<String>,
        old_commit: Option<String>,
    ) -> Result<CommandResult> {
        if self.manifest()?.patch(&patch.name).is_none() {
            let insertion = self.manifest()?.insertion_index(&patch);
            self.manifest_mut()?
                .patches
                .insert(insertion, patch.clone());
        }
        self.write_manifest()?;
        let exports = self.write_exports()?;
        let ledger = self.write_ledger()?;
        let generated = std::iter::once(self.manifest_path.clone())
            .chain(std::iter::once(ledger))
            .chain(exports)
            .collect::<Vec<_>>();
        self.refresh_bookkeeping(&generated)?;
        let new_commit = self.patch_commit(&patch.name)?;
        self.write_active(&ActivePatchState::Existing {
            patch: patch.name.clone(),
        })?;
        let check = self.check_repository(true)?;
        self.complete_local_operation(operation)?;
        Ok(CommandResult::PatchRefresh(PatchRefreshResult {
            patch: patch.name,
            capture,
            captured_paths,
            old_commit,
            new_commit,
            generated_paths: display_paths(&self.repo, &generated),
            check,
        }))
    }

    pub fn patch_finish(&self, target: &PatchTarget, mode: ExecutionMode) -> Result<CommandResult> {
        let active = self
            .read_active()?
            .ok_or_else(DomainError::active_patch_required)?;
        let name = target
            .patch
            .clone()
            .unwrap_or_else(|| active.name().to_string());
        if active.name() != name {
            return Err(DomainError::invalid_request(format!(
                "requested patch {name} is not active"
            ))
            .into());
        }
        if !matches!(active, ActivePatchState::Existing { .. }) {
            return Err(DomainError::invalid_request("draft patch has not been refreshed").into());
        }
        self.require_clean()?;
        let check = self.check_repository(true)?;
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(MutationPlan {
                command: "patch.finish".into(),
                reads: vec![self.active_path()?.display().to_string()],
                writes: vec![self.active_path()?.display().to_string()],
                hooks: Vec::new(),
                ref_updates: Vec::new(),
                paths: Vec::new(),
                requires_confirmation: false,
            }));
        }
        self.clear_active()?;
        Ok(CommandResult::PatchFinish(PatchFinishResult {
            patch: name,
            check,
        }))
    }

    pub fn patch_remove(
        &mut self,
        args: PatchTransitionArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        self.patch_deactivate(args, mode, false)
    }

    pub fn patch_disable(
        &mut self,
        args: PatchTransitionArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        self.patch_deactivate(args, mode, true)
    }

    fn patch_deactivate(
        &mut self,
        args: PatchTransitionArgs,
        mode: ExecutionMode,
        disable: bool,
    ) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        self.check_repository(false)?;
        if args.reason.trim().is_empty() {
            return Err(DomainError::invalid_request("transition reason is required").into());
        }
        let available = self.manifest()?.patch_names();
        let position = self
            .manifest()?
            .patches
            .iter()
            .position(|patch| patch.name == args.patch)
            .ok_or_else(|| DomainError::patch_not_found(&args.patch, available, None))?;
        let patch = self.manifest()?.patches[position].clone();
        if patch.name == self.manifest()?.bookkeeping_patch {
            return Err(DomainError::invalid_request(
                "bookkeeping patch cannot be removed or disabled",
            )
            .into());
        }
        let commit = self.patch_commit(&patch.name)?;
        let command = if disable {
            "patch.disable"
        } else {
            "patch.remove"
        };
        let plan = MutationPlan {
            command: command.into(),
            reads: vec![self.manifest_path.display().to_string(), commit.clone()],
            writes: vec![
                "StGit series".into(),
                "manifest/history".into(),
                "generated evidence".into(),
            ],
            hooks: Vec::new(),
            ref_updates: vec!["annotated recovery tag".into()],
            paths: patch.scope.clone(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        let kind = if disable {
            OperationKind::PatchDisable
        } else {
            OperationKind::PatchRemove
        };
        let mut operation = self.create_operation(kind, None)?;
        operation.phase = "removing".into();
        operation.intent = Some(OperationIntent::Transition {
            patch: patch.clone(),
            commit: commit.clone(),
            position,
            reason: args.reason.clone(),
        });
        self.write_operation(&operation)?;
        if let Err(error) = run(&self.repo, "stg", ["delete", &patch.name]) {
            operation.phase = "conflict".into();
            operation.next_actions = vec![
                "resolve conflicts".into(),
                "stg refresh".into(),
                "stg push --all".into(),
                "forkctl operation continue".into(),
            ];
            self.write_operation(&operation)?;
            return Err(error).context(format!(
                "{command} stopped; recovery tag {}",
                operation.recovery.tag
            ));
        }
        self.finish_patch_deactivate(
            &mut operation,
            patch,
            commit,
            position,
            args.reason,
            disable,
        )
    }

    fn finish_patch_deactivate(
        &mut self,
        operation: &mut OperationState,
        patch: Patch,
        commit: String,
        position: usize,
        reason: String,
        disable: bool,
    ) -> Result<CommandResult> {
        let record = DisabledPatch {
            patch: patch.clone(),
            commit: commit.clone(),
            position,
            reason,
            recovery: operation.recovery.clone(),
        };
        let manifest = self.manifest_mut()?;
        manifest
            .patches
            .retain(|candidate| candidate.name != patch.name);
        if disable {
            manifest.disabled_patches.push(record);
        } else {
            manifest.history.push(HistoryEvent::PatchRemoved { record });
        }
        self.finish_patch_transition(operation, patch.name, commit, disable)
    }

    pub fn patch_enable(&mut self, name: &str, mode: ExecutionMode) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        self.check_repository(false)?;
        let record = self
            .manifest()?
            .disabled_patches
            .iter()
            .find(|record| record.patch.name == name)
            .cloned()
            .ok_or_else(|| {
                DomainError::invalid_request(format!("disabled patch not found: {name}"))
            })?;
        let plan = MutationPlan {
            command: "patch.enable".into(),
            reads: vec![record.commit.clone(), record.recovery.tag.clone()],
            writes: vec![
                "StGit series".into(),
                "manifest/history".into(),
                "generated evidence".into(),
            ],
            hooks: vec!["commit-msg via stg import".into()],
            ref_updates: vec!["annotated recovery tag".into()],
            paths: record.patch.scope.clone(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        let mut operation = self.create_operation(OperationKind::PatchEnable, None)?;
        operation.phase = "enabling".into();
        operation.intent = Some(OperationIntent::Transition {
            patch: record.patch.clone(),
            commit: record.commit.clone(),
            position: record.position,
            reason: record.reason.clone(),
        });
        self.write_operation(&operation)?;
        if let Err(error) = self.apply_disabled_patch(&record) {
            operation.phase = "conflict".into();
            operation.next_actions = vec![
                "resolve conflicts".into(),
                "stg add --update".into(),
                "stg refresh".into(),
                "stg push --all".into(),
                "forkctl operation continue".into(),
            ];
            self.write_operation(&operation)?;
            return Err(error).context(format!(
                "patch enable stopped; recovery tag {}",
                operation.recovery.tag
            ));
        }
        let manifest = self.manifest_mut()?;
        manifest
            .disabled_patches
            .retain(|candidate| candidate.patch.name != name);
        let insertion = record
            .position
            .min(manifest.patches.len().saturating_sub(1));
        manifest.patches.insert(insertion, record.patch.clone());
        manifest.history.push(HistoryEvent::PatchEnabled {
            record: record.clone(),
            recovery: operation.recovery.clone(),
        });
        self.finish_patch_transition(&mut operation, name.to_string(), record.commit, false)
    }

    fn apply_disabled_patch(&self, record: &DisabledPatch) -> Result<()> {
        let active = self.manifest()?.patch_names();
        let insertion = record.position.min(active.len().saturating_sub(1));
        if insertion == 0 {
            run(&self.repo, "stg", ["pop", "--all"])?;
        } else {
            run(&self.repo, "stg", ["goto", &active[insertion - 1]])?;
        }
        let patch = capture(
            &self.repo,
            "git",
            ["format-patch", "--stdout", "-1", &record.commit],
        )?;
        let mut file = tempfile::NamedTempFile::new_in(&self.repo)?;
        file.write_all(patch.as_bytes())?;
        file.flush()?;
        run(
            &self.repo,
            "stg",
            [
                OsString::from("import"),
                OsString::from("--3way"),
                OsString::from("--name"),
                OsString::from(&record.patch.name),
                file.path().as_os_str().to_owned(),
            ],
        )?;
        run(&self.repo, "stg", ["push", "--all"])
    }

    pub(super) fn continue_patch_transition(
        &mut self,
        mut operation: OperationState,
        kind: OperationKind,
        patch: Patch,
        commit: String,
        position: usize,
        reason: String,
    ) -> Result<CommandResult> {
        let conflicts = capture(
            &self.repo,
            "git",
            ["diff", "--name-only", "--diff-filter=U"],
        )?;
        if !conflicts.is_empty() {
            return Err(DomainError::operation_conflict(
                format!(
                    "unresolved paths remain: {}",
                    conflicts.lines().collect::<Vec<_>>().join(", ")
                ),
                Some(&operation),
            )
            .into());
        }
        if capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? != "0" {
            run(&self.repo, "stg", ["push", "--all"])?;
        }
        if kind == OperationKind::PatchEnable {
            let record = self
                .manifest()?
                .disabled_patches
                .iter()
                .find(|record| record.patch.name == patch.name)
                .cloned()
                .ok_or_else(|| {
                    DomainError::invalid_request(format!(
                        "disabled patch not found: {}",
                        patch.name
                    ))
                })?;
            let manifest = self.manifest_mut()?;
            manifest
                .disabled_patches
                .retain(|candidate| candidate.patch.name != patch.name);
            let insertion = position.min(manifest.patches.len().saturating_sub(1));
            manifest.patches.insert(insertion, patch.clone());
            manifest.history.push(HistoryEvent::PatchEnabled {
                record,
                recovery: operation.recovery.clone(),
            });
            return self.finish_patch_transition(&mut operation, patch.name, commit, false);
        }
        self.finish_patch_deactivate(
            &mut operation,
            patch,
            commit,
            position,
            reason,
            kind == OperationKind::PatchDisable,
        )
    }

    fn finish_patch_transition(
        &mut self,
        operation: &mut OperationState,
        patch: String,
        commit: String,
        disabled: bool,
    ) -> Result<CommandResult> {
        self.write_manifest()?;
        let exports = self.write_exports()?;
        let ledger = self.write_ledger()?;
        let generated = std::iter::once(self.manifest_path.clone())
            .chain(std::iter::once(ledger))
            .chain(exports)
            .collect::<Vec<_>>();
        self.refresh_bookkeeping(&generated)?;
        let new_tip = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        operation.phase = "ready_to_publish".into();
        operation.new_tip = Some(new_tip.clone());
        operation.next_actions = vec!["forkctl publish".into()];
        self.write_operation(operation)?;
        let check = self.check_repository(false)?;
        let result = PatchTransitionResult {
            patch,
            commit,
            recovery_tag: operation.recovery.tag.clone(),
            new_tip,
            check,
        };
        Ok(if disabled {
            CommandResult::PatchDisable(result)
        } else if operation.kind == OperationKind::PatchRemove {
            CommandResult::PatchRemove(result)
        } else {
            CommandResult::PatchEnable(result)
        })
    }

    fn capture_paths(&self, patch: &Patch, source: &CaptureSource) -> Result<Vec<String>> {
        let inventory = self.worktree_inventory()?;
        let candidates = match source {
            CaptureSource::Staged => inventory.staged,
            CaptureSource::All => {
                let mut paths = inventory.staged;
                paths.extend(inventory.unstaged);
                paths.extend(inventory.untracked);
                paths
            }
            CaptureSource::Paths { pathspecs } => {
                if pathspecs.is_empty() {
                    return Err(
                        DomainError::invalid_request("at least one pathspec is required").into(),
                    );
                }
                let mut command = vec![
                    "ls-files",
                    "--modified",
                    "--others",
                    "--exclude-standard",
                    "--",
                ];
                command.extend(pathspecs.iter().map(String::as_str));
                capture(&self.repo, "git", command)?
                    .lines()
                    .map(str::to_string)
                    .collect()
            }
        };
        let mut paths = candidates;
        paths.sort();
        paths.dedup();
        let rejected = paths
            .iter()
            .filter(|path| !patch.owns(path))
            .cloned()
            .collect::<Vec<_>>();
        if !rejected.is_empty() {
            return Err(DomainError::staged_scope_violation(patch.name.clone(), rejected).into());
        }
        Ok(paths)
    }

    fn stage_capture(&self, paths: &[String], source: &CaptureSource) -> Result<()> {
        if matches!(source, CaptureSource::Staged) {
            return Ok(());
        }
        let mut command = vec![OsString::from("add"), OsString::from("--")];
        command.extend(paths.iter().map(OsString::from));
        run(&self.repo, "git", command)
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

fn display_paths(repo: &std::path::Path, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| {
            path.strip_prefix(repo)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn apply_check_edit(patch: &mut Patch, edit: CheckEdit) {
    match edit {
        CheckEdit::Set { checks } => patch.checks = checks,
        CheckEdit::Add { checks } => {
            for check in checks {
                if let Some(existing) = patch
                    .checks
                    .iter_mut()
                    .find(|existing| existing.name == check.name)
                {
                    *existing = check;
                } else {
                    patch.checks.push(check);
                }
            }
        }
    }
}
