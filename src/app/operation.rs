use super::App;
use crate::error::DomainError;
use crate::process::{capture, run};
use crate::protocol::{
    CommandResult, ExecutionMode, MutationPlan, OperationAbortResult, OperationContinueResult,
    OperationStatusResult,
};
use crate::state::{OperationIntent, OperationKind};
use anyhow::Result;

impl App {
    pub fn operation_status(&self) -> Result<OperationStatusResult> {
        Ok(OperationStatusResult {
            operation: self.read_operation()?,
        })
    }

    pub fn operation_continue(&mut self, mode: ExecutionMode) -> Result<CommandResult> {
        let operation = self
            .read_operation()?
            .ok_or_else(|| DomainError::invalid_request("no forkctl operation is in progress"))?;
        self.load_operation_manifest()?;
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(MutationPlan {
                command: "operation.continue".into(),
                reads: vec![self.operation_path()?.display().to_string()],
                writes: vec![format!(
                    "continue {:?} from {}",
                    operation.kind, operation.phase
                )],
                hooks: Vec::new(),
                ref_updates: Vec::new(),
                paths: Vec::new(),
                requires_confirmation: false,
            }));
        }
        let result = match operation.kind {
            OperationKind::Rebase => Some(Box::new(CommandResult::Rebase(Box::new(
                self.continue_rebase(operation)?,
            )))),
            OperationKind::PatchEdit => {
                let Some(OperationIntent::Edit { patch }) = operation.intent.clone() else {
                    return Err(DomainError::operation_conflict(
                        "patch edit operation has no typed intent",
                        Some(&operation),
                    )
                    .into());
                };
                Some(Box::new(self.continue_patch_edit(&operation, patch)?))
            }
            OperationKind::PatchRefresh => {
                let Some(OperationIntent::Refresh {
                    patch,
                    capture,
                    captured_paths,
                }) = operation.intent.clone()
                else {
                    return Err(DomainError::operation_conflict(
                        "patch refresh operation has no typed intent",
                        Some(&operation),
                    )
                    .into());
                };
                Some(Box::new(self.continue_patch_refresh(
                    &operation,
                    patch,
                    capture,
                    captured_paths,
                )?))
            }
            kind @ (OperationKind::PatchRemove
            | OperationKind::PatchDisable
            | OperationKind::PatchEnable) => {
                let Some(OperationIntent::Transition {
                    patch,
                    commit,
                    position,
                    reason,
                }) = operation.intent.clone()
                else {
                    return Err(DomainError::operation_conflict(
                        "patch transition operation has no typed intent",
                        Some(&operation),
                    )
                    .into());
                };
                Some(Box::new(self.continue_patch_transition(
                    operation, kind, patch, commit, position, reason,
                )?))
            }
        };
        Ok(CommandResult::OperationContinue(Box::new(
            OperationContinueResult {
                operation: self.read_operation()?,
                result,
            },
        )))
    }

    pub fn operation_abort(
        &mut self,
        confirmed: bool,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        let operation = self
            .read_operation()?
            .ok_or_else(|| DomainError::invalid_request("no forkctl operation is in progress"))?;
        self.load_operation_manifest()?;
        let plan = MutationPlan {
            command: "operation.abort".into(),
            reads: vec![self.operation_path()?.display().to_string()],
            writes: vec![
                format!("restore HEAD to {}", operation.old_tip),
                "restore StGit operation state".into(),
                "restore tracked manifest from recovered stack".into(),
            ],
            hooks: Vec::new(),
            ref_updates: vec![format!("HEAD -> {}", operation.old_tip)],
            paths: self.dirty_paths()?,
            requires_confirmation: true,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        if !confirmed {
            return Err(DomainError::invalid_request("operation abort requires --yes").into());
        }
        if matches!(operation.kind, OperationKind::Rebase)
            && let Some(active) = self.read_active()?
        {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        self.restore_operation_stack(&operation)?;
        self.verify_restored_operation_stack(&operation)?;
        self.manifest = None;
        self.manifest_error = None;
        let rediscovered = App::discover(
            self.manifest_path
                .strip_prefix(&self.repo)
                .unwrap_or(&self.manifest_path),
        )?;
        self.manifest = rediscovered.manifest;
        self.manifest_error = rediscovered.manifest_error;
        let check = self.check_restored_repository(matches!(
            operation.kind,
            OperationKind::PatchRefresh
                | OperationKind::PatchEdit
                | OperationKind::PatchRemove
                | OperationKind::PatchDisable
                | OperationKind::PatchEnable
        ))?;
        self.complete_local_operation(&operation)?;
        Ok(CommandResult::OperationAbort(OperationAbortResult {
            operation_id: operation.id,
            restored_tip: operation.old_tip,
            check,
        }))
    }

    fn restore_operation_stack(&self, operation: &crate::state::OperationState) -> Result<()> {
        if !capture(
            &self.repo,
            "git",
            ["diff", "--name-only", "--diff-filter=U"],
        )?
        .is_empty()
        {
            run(&self.repo, "stg", ["undo", "--hard"])?;
        }
        run(&self.repo, "stg", ["delete", "--all", "--conflicts=allow"])?;
        run(&self.repo, "git", ["reset", "--hard", &operation.old_tip])?;
        run(
            &self.repo,
            "stg",
            ["uncommit", "--to", &operation.old_base, "--exclusive"],
        )
    }

    fn verify_restored_operation_stack(
        &self,
        operation: &crate::state::OperationState,
    ) -> Result<()> {
        let actual_tip = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        if actual_tip != operation.old_tip {
            return Err(DomainError::operation_conflict(
                format!(
                    "abort restored HEAD to {actual_tip}, expected {}",
                    operation.old_tip
                ),
                Some(operation),
            )
            .into());
        }
        let actual_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        if actual_base != operation.old_base {
            return Err(DomainError::operation_conflict(
                format!(
                    "abort restored StGit base to {actual_base}, expected {}",
                    operation.old_base
                ),
                Some(operation),
            )
            .into());
        }
        let expected = operation
            .old_patches
            .iter()
            .map(|patch| patch.name.clone())
            .collect::<Vec<_>>();
        let actual = self.stg_series()?;
        if actual != expected {
            return Err(DomainError::operation_conflict(
                format!(
                    "abort restored patch order {}, expected {}",
                    actual.join(", "),
                    expected.join(", ")
                ),
                Some(operation),
            )
            .into());
        }
        for evidence in &operation.old_patches {
            let actual = self.patch_commit(&evidence.name)?;
            if actual != evidence.commit {
                return Err(DomainError::operation_conflict(
                    format!(
                        "abort restored patch {} to {actual}, expected {}",
                        evidence.name, evidence.commit
                    ),
                    Some(operation),
                )
                .into());
            }
        }
        Ok(())
    }
}
