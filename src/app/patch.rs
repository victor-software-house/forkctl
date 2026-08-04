use super::App;
use crate::error::DomainError;
use crate::manifest::Patch;
use crate::process::{capture, run};
use crate::protocol::{
    CaptureSource, CommandResult, ExecutionMode, MutationPlan, PatchCreateArgs, PatchCreateResult,
    PatchEditArgs, PatchEditResult, PatchFinishResult, PatchListResult, PatchRefreshArgs,
    PatchRefreshResult, PatchSelectResult, PatchShowResult, PatchSummary, PatchTarget, ScopeEdit,
};
use crate::state::{ActivePatchState, OperationIntent, OperationKind, OperationState};
use anyhow::{Context, Result, ensure};
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
            .collect();
        Ok(PatchListResult {
            patches,
            active_patch: active,
        })
    }

    pub fn patch_show(&self, target: &PatchTarget) -> Result<PatchShowResult> {
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
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        let patch: Patch = args.into();
        patch.validate()?;
        ensure!(
            self.manifest()?.patch(&patch.name).is_none(),
            "patch already exists: {}",
            patch.name
        );
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
        patch.validate()?;
        let proposed = self.proposed_manifest_with_patch(patch.clone())?;
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
        let old_kind = self.manifest()?.patch(&name).expect("patch exists").kind;
        let mut operation = self.create_operation(OperationKind::PatchEdit, None)?;
        operation.phase = "editing".into();
        operation.intent = Some(OperationIntent::PatchEdit {
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
        let proposed = self.proposed_manifest_with_patch(patch.clone())?;
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

    fn proposed_manifest_with_patch(&self, patch: Patch) -> Result<crate::manifest::Manifest> {
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
        let index = proposed.insertion_index(patch.kind);
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
        ensure!(
            active.name() == name,
            "requested patch {name} is not active"
        );
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
        operation.intent = Some(OperationIntent::PatchRefresh {
            patch: patch.clone(),
            capture: args.capture.clone(),
            captured_paths: capture_paths.clone(),
        });
        self.write_operation(&operation)?;
        let mutation = self
            .stage_capture(&capture_paths, &args.capture)
            .and_then(|()| {
                if matches!(active, ActivePatchState::Draft { .. }) {
                    let insertion = self.manifest()?.insertion_index(patch.kind);
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
            let insertion = self.manifest()?.insertion_index(patch.kind);
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
        ensure!(
            active.name() == name,
            "requested patch {name} is not active"
        );
        ensure!(
            matches!(active, ActivePatchState::Existing { .. }),
            "draft patch has not been refreshed"
        );
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
                ensure!(!pathspecs.is_empty(), "at least one pathspec is required");
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
