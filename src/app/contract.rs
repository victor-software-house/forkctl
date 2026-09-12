use super::App;
use crate::error::DomainError;
use crate::manifest::{CommitMessagePolicy, Contracts};
use crate::process::capture;
use crate::protocol::{
    CommandResult, CommitMessageMigrationArgs, CommitMessageMigrationResult, ContractEditArgs,
    ContractEditResult, ExecutionMode, MutationPlan,
};
use crate::state::{OperationIntent, OperationKind, OperationState};
use anyhow::{Context, Result};

impl App {
    pub fn contract_edit(
        &mut self,
        args: ContractEditArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        if !args.clear
            && args.allow_base.is_empty()
            && args.required_text.is_empty()
            && args.publish_mode.is_none()
        {
            return Err(DomainError::invalid_request(
                "contract edit requires --clear, --allow-base, --required-text, or --publish-mode",
            )
            .into());
        }

        let mut contracts = if args.clear {
            Contracts::default()
        } else {
            self.manifest()?.contracts.clone()
        };
        for pattern in args.allow_base {
            if !contracts.allow_base.contains(&pattern) {
                contracts.allow_base.push(pattern);
            }
        }
        for required in args.required_text {
            if !contracts.required_text.iter().any(|existing| {
                existing.path == required.path && existing.contains == required.contains
            }) {
                contracts.required_text.push(required);
            }
        }

        let mut proposed = self.manifest()?.clone();
        proposed.contracts = contracts.clone();
        if let Some(mode) = args.publish_mode {
            proposed.downstream.publish = mode;
        }
        proposed
            .validate(&self.repo, &self.manifest_path)
            .map_err(|error| DomainError::invalid_request(error.to_string()))?;
        self.validate_required_text(&contracts.required_text)
            .map_err(|error| DomainError::invalid_request(error.to_string()))?;

        let plan = MutationPlan {
            command: "contract.edit".into(),
            reads: vec![self.manifest_path.display().to_string()],
            writes: vec![
                self.manifest_path.display().to_string(),
                self.manifest()?.documents.ledger.clone(),
            ],
            hooks: vec!["pre-commit via bookkeeping refresh".into()],
            ref_updates: Vec::new(),
            paths: contracts
                .required_text
                .iter()
                .map(|required| required.path.clone())
                .collect(),
            requires_confirmation: args.clear,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }

        self.manifest = Some(proposed);
        self.write_manifest()?;
        let ledger = self.write_ledger()?;
        let generated = vec![self.manifest_path.clone(), ledger];
        self.refresh_bookkeeping(&generated)?;
        let check = self.check_repository(false)?;
        Ok(CommandResult::ContractEdit(ContractEditResult {
            contracts,
            generated_paths: generated
                .iter()
                .map(|path| {
                    path.strip_prefix(&self.repo)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            check,
        }))
    }

    pub fn migrate_commit_messages(
        &mut self,
        args: CommitMessageMigrationArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        if let Some(operation) = self.read_operation()? {
            return Err(DomainError::operation_in_progress(&operation).into());
        }
        let mut policy = self.manifest()?.commit_messages.clone();
        if let Some(source) = args.source {
            policy.source = source;
        }
        if let Some(tooling) = args.tooling {
            policy.tooling = tooling;
        }
        policy.mark_declared();
        policy
            .validate()
            .map_err(|error| DomainError::invalid_request(error.to_string()))?;
        self.check_repository_before_subject_migration()?;

        let plan = MutationPlan {
            command: "contract.migrate_commit_messages".into(),
            reads: vec![
                self.manifest_path.display().to_string(),
                "applied StGit patch messages".into(),
            ],
            writes: vec![
                "complete local StGit series".into(),
                self.manifest_path.display().to_string(),
                self.manifest()?.documents.ledger.clone(),
                self.manifest()?.documents.exports.clone(),
            ],
            hooks: vec![
                "commit-msg via stg edit".into(),
                "pre-commit via bookkeeping refresh".into(),
            ],
            ref_updates: vec!["annotated recovery tag".into()],
            paths: Vec::new(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }

        let mut operation = self.create_operation(OperationKind::CommitMessageMigration, None)?;
        operation.phase = "rewriting_commit_messages".into();
        operation.intent = Some(OperationIntent::CommitMessageMigration {
            policy: policy.clone(),
        });
        operation.next_actions = vec!["forkctl operation continue".into()];
        self.write_operation(&operation)?;
        self.continue_commit_message_migration(operation, policy)
            .map(CommandResult::CommitMessageMigration)
    }

    pub(super) fn continue_commit_message_migration(
        &mut self,
        mut operation: OperationState,
        policy: CommitMessagePolicy,
    ) -> Result<CommitMessageMigrationResult> {
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
            return Err(DomainError::operation_conflict(
                "commit-message migration requires every patch to remain applied",
                Some(&operation),
            )
            .into());
        }

        let patches = self.manifest()?.patches.clone();
        let mut rewritten_patches = Vec::new();
        for patch in &patches {
            let evidence = operation
                .old_patches
                .iter()
                .find(|evidence| evidence.name == patch.name)
                .with_context(|| format!("migration journal has no commit for {}", patch.name))?;
            let old_subject = capture(
                &self.repo,
                "git",
                ["log", "-1", "--format=%s", &evidence.commit],
            )?;
            if old_subject != patch.subject(&policy) {
                rewritten_patches.push(patch.name.clone());
            }
        }

        for patch in &patches {
            let commit = self.patch_commit(&patch.name)?;
            let actual = capture(&self.repo, "git", ["log", "-1", "--format=%s", &commit])?;
            let expected = patch.subject(&policy);
            if actual == expected {
                continue;
            }
            if let Err(error) = self.edit_patch_message(&patch.name, &patch.message(&policy)) {
                operation.phase = "conflict".into();
                operation.next_actions = vec![
                    "resolve the commit-msg hook failure".into(),
                    "forkctl operation continue".into(),
                ];
                self.write_operation(&operation)?;
                return Err(error).context(format!(
                    "commit-message migration stopped at {}; recovery tag {}",
                    patch.name, operation.recovery.tag
                ));
            }
        }

        self.manifest_mut()?.commit_messages = policy.clone();
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
        operation.next_actions = vec!["review rewritten subjects".into(), "forkctl publish".into()];
        self.write_operation(&operation)?;
        let check = self.check_repository(false)?;

        Ok(CommitMessageMigrationResult {
            policy,
            old_tip: operation.old_tip,
            new_tip,
            recovery_tag: operation.recovery.tag,
            recovery_tag_object: operation.recovery.tag_object,
            rewritten_patches,
            generated_paths: generated
                .iter()
                .map(|path| {
                    path.strip_prefix(&self.repo)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            check,
        })
    }
}
