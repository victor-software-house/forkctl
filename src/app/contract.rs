use super::App;
use crate::error::AppResult as Result;
use crate::error::DomainError;
use crate::manifest::Contracts;
use crate::protocol::{
    CommandResult, ContractEditArgs, ContractEditResult, ExecutionMode, MutationPlan,
};

impl App {
    pub fn contract_edit(
        &mut self,
        args: ContractEditArgs,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        self.require_no_active_patch()?;
        self.require_no_operation()?;
        if !args.clear && args.allow_base.is_empty() && args.required_text.is_empty() {
            return Err(DomainError::invalid_request(
                "contract edit requires --clear, --allow-base, or --required-text",
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
}
