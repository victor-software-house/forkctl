use super::App;
use crate::error::DomainError;
use crate::process::{capture, run};
use crate::protocol::{CommandResult, ExecutionMode, MutationPlan, PublishResult};
use anyhow::{Result, ensure};

impl App {
    pub fn publish(&self, mode: ExecutionMode) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        self.check_repository(false)?;
        let operation = self.read_operation()?.ok_or_else(|| {
            DomainError::operation_conflict("no forkctl operation is ready for publication", None)
        })?;
        if operation.phase != "ready_to_publish" {
            return Err(DomainError::operation_conflict(
                "operation is not ready to publish",
                Some(&operation),
            )
            .into());
        }
        let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        ensure!(
            operation.new_tip.as_deref() == Some(head.as_str()),
            "operation does not describe current HEAD"
        );
        self.check_operation(&operation)?;
        let manifest = self.manifest()?;
        let downstream_ref = self.downstream_ref()?;
        let tag_ref = format!("refs/tags/{}", operation.recovery.tag);
        let lease = format!(
            "--force-with-lease={downstream_ref}:{}",
            operation.expected_remote_sha
        );
        let tag_refspec = format!("{tag_ref}:{tag_ref}");
        let branch_refspec = format!("HEAD:{downstream_ref}");
        let plan = MutationPlan {
            command: "publish".into(),
            reads: vec![head.clone(), operation.expected_remote_sha.clone()],
            writes: Vec::new(),
            hooks: vec!["pre-push via git push".into()],
            ref_updates: vec![tag_refspec.clone(), branch_refspec.clone(), lease.clone()],
            paths: Vec::new(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        let actual_remote = self.downstream_sha()?;
        if actual_remote != operation.expected_remote_sha {
            return Err(DomainError::remote_advanced(
                manifest.downstream.remote.clone(),
                downstream_ref.clone(),
                operation.expected_remote_sha.clone(),
                actual_remote,
            )
            .into());
        }
        if let Ok(remote_tag) = self.remote_ref_sha(&manifest.downstream.remote, &tag_ref)
            && remote_tag != operation.recovery.tag_object
        {
            return Err(DomainError::publication_ref_mismatch(
                manifest.downstream.remote.clone(),
                tag_ref.clone(),
                operation.recovery.tag_object.clone(),
                remote_tag,
            )
            .into());
        }
        if let Err(error) = run(
            &self.repo,
            "git",
            [
                "push",
                "--atomic",
                lease.as_str(),
                manifest.downstream.remote.as_str(),
                tag_refspec.as_str(),
                branch_refspec.as_str(),
            ],
        ) {
            if let Some(domain) = error.downcast_ref::<DomainError>() {
                return Err(DomainError::publication_rejected(domain).into());
            }
            return Err(error);
        }
        ensure!(
            self.downstream_sha()? == head,
            "published branch differs from HEAD"
        );
        ensure!(
            self.remote_ref_sha(&manifest.downstream.remote, &tag_ref)?
                == operation.recovery.tag_object,
            "published recovery tag differs"
        );
        self.clear_operation()?;
        Ok(CommandResult::Publish(PublishResult {
            branch: manifest.downstream.branch.clone(),
            head,
            recovery_tag: operation.recovery.tag,
            recovery_tag_object: operation.recovery.tag_object,
            expected_lease: operation.expected_remote_sha,
        }))
    }
}
