use super::{App, recovery_id};
use crate::error::DomainError;
use crate::manifest::RecoveryEvidence;
use crate::process::{capture, run};
use crate::protocol::{CommandResult, ExecutionMode, MutationPlan, PublishResult};
use crate::state::OperationKind;
use anyhow::{Result, ensure};

const READY_TO_PUBLISH: &str = "ready_to_publish";

struct Publication {
    head: String,
    remote_sha: String,
    downstream_ref: String,
    lease: String,
    branch_refspec: String,
    fast_forward: bool,
    operation_recovery: Option<RecoveryEvidence>,
    needs_publication_recovery: bool,
    clears_operation: bool,
}

impl App {
    pub fn publish(&self, mode: ExecutionMode) -> Result<CommandResult> {
        let publication = self.preflight_publication()?;
        if publication.remote_sha == publication.head {
            return Ok(self.already_published(&publication, mode));
        }
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(publication_plan(&publication)));
        }
        self.execute_publication(&publication)
    }

    fn preflight_publication(&self) -> Result<Publication> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        self.check_repository(false)?;

        let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        let operation = self.read_operation()?;
        if let Some(operation) = &operation {
            if operation.phase != READY_TO_PUBLISH {
                return Err(DomainError::operation_conflict(
                    format!(
                        "operation {} is in phase {} and is not ready to publish",
                        operation.id, operation.phase
                    ),
                    Some(operation),
                )
                .into());
            }
            ensure!(
                operation.kind == OperationKind::Rebase,
                "only rebase operations require publication review"
            );
            ensure!(
                operation.new_tip.as_deref() == Some(head.as_str()),
                "operation does not describe current HEAD"
            );
            self.check_operation(operation)?;
        }

        let downstream_ref = self.downstream_ref()?;
        let remote_sha = self.downstream_sha()?;
        let fast_forward = remote_sha == head || self.is_ancestor(&remote_sha, &head)?;
        let operation_recovery = operation
            .as_ref()
            .map(|operation| operation.recovery.clone());
        if !fast_forward {
            let expected = match &operation {
                Some(operation) => operation.expected_remote_sha.clone(),
                None => self.downstream_tracking_sha()?,
            };
            if expected != remote_sha {
                return Err(DomainError::remote_advanced(
                    self.manifest()?.downstream.remote.clone(),
                    downstream_ref,
                    expected,
                    remote_sha,
                )
                .into());
            }
        }
        let needs_publication_recovery = !fast_forward
            && operation_recovery
                .as_ref()
                .is_none_or(|recovery| recovery.old_tip != remote_sha);
        Ok(Publication {
            head,
            lease: format!("--force-with-lease={downstream_ref}:{remote_sha}"),
            branch_refspec: format!("HEAD:{downstream_ref}"),
            remote_sha,
            downstream_ref,
            fast_forward,
            operation_recovery,
            needs_publication_recovery,
            clears_operation: operation.is_some(),
        })
    }

    fn already_published(&self, publication: &Publication, mode: ExecutionMode) -> CommandResult {
        if mode == ExecutionMode::Plan {
            return CommandResult::Plan(MutationPlan {
                command: "publish".into(),
                reads: vec![publication.head.clone(), publication.remote_sha.clone()],
                writes: Vec::new(),
                hooks: Vec::new(),
                ref_updates: Vec::new(),
                paths: Vec::new(),
                requires_confirmation: false,
            });
        }
        CommandResult::Publish(PublishResult {
            branch: self
                .manifest()
                .map(|manifest| manifest.downstream.branch.clone())
                .unwrap_or_default(),
            head: publication.head.clone(),
            already_published: true,
            fast_forward: true,
            recovery_tags: Vec::new(),
            pushed_refs: Vec::new(),
            expected_lease: publication.remote_sha.clone(),
        })
    }

    fn publication_recovery_tags(
        &self,
        publication: &Publication,
    ) -> Result<Vec<(String, String)>> {
        let mut tags = Vec::new();
        if let Some(recovery) = &publication.operation_recovery {
            tags.push((recovery.tag.clone(), recovery.tag_object.clone()));
        }
        if publication.needs_publication_recovery {
            let id = recovery_id(&publication.remote_sha)?;
            let (tag, object) =
                self.create_recovery_tag(&id, &publication.remote_sha, "publication")?;
            tags.push((tag, object));
        }
        let remote = self.manifest()?.downstream.remote.clone();
        for (tag, object) in &tags {
            let tag_ref = format!("refs/tags/{tag}");
            if let Ok(remote_object) = self.remote_ref_sha(&remote, &tag_ref)
                && remote_object != *object
            {
                return Err(DomainError::publication_ref_mismatch(
                    remote,
                    tag_ref,
                    object.clone(),
                    remote_object,
                )
                .into());
            }
        }
        Ok(tags)
    }

    fn execute_publication(&self, publication: &Publication) -> Result<CommandResult> {
        let manifest = self.manifest()?;
        let recovery_tags = self.publication_recovery_tags(publication)?;
        let actual_remote = self.downstream_sha()?;
        if actual_remote != publication.remote_sha {
            return Err(DomainError::remote_advanced(
                manifest.downstream.remote.clone(),
                publication.downstream_ref.clone(),
                publication.remote_sha.clone(),
                actual_remote,
            )
            .into());
        }

        let mut push = vec![
            "push".to_string(),
            "--atomic".to_string(),
            publication.lease.clone(),
            manifest.downstream.remote.clone(),
        ];
        let mut pushed_refs = Vec::new();
        for (tag, _) in &recovery_tags {
            let tag_ref = format!("refs/tags/{tag}");
            push.push(format!("{tag_ref}:{tag_ref}"));
            pushed_refs.push(tag_ref);
        }
        push.push(publication.branch_refspec.clone());
        pushed_refs.push(publication.branch_refspec.clone());

        if let Err(error) = run(&self.repo, "git", push) {
            if let Some(domain) = error.downcast_ref::<DomainError>() {
                return Err(DomainError::publication_rejected(domain).into());
            }
            return Err(error);
        }

        ensure!(
            self.downstream_sha()? == publication.head,
            "published branch differs from HEAD"
        );
        for (tag, object) in &recovery_tags {
            let tag_ref = format!("refs/tags/{tag}");
            ensure!(
                self.remote_ref_sha(&manifest.downstream.remote, &tag_ref)? == *object,
                "published recovery tag {tag} differs"
            );
        }
        if publication.clears_operation {
            self.clear_operation()?;
        }
        Ok(CommandResult::Publish(PublishResult {
            branch: manifest.downstream.branch.clone(),
            head: publication.head.clone(),
            already_published: false,
            fast_forward: publication.fast_forward,
            recovery_tags: recovery_tags
                .into_iter()
                .map(|(tag, object)| format!("{tag} -> {object}"))
                .collect(),
            pushed_refs,
            expected_lease: publication.remote_sha.clone(),
        }))
    }
}

fn publication_plan(publication: &Publication) -> MutationPlan {
    let mut ref_updates = vec![
        publication.branch_refspec.clone(),
        publication.lease.clone(),
    ];
    if let Some(recovery) = &publication.operation_recovery {
        ref_updates.push(format!("refs/tags/{}", recovery.tag));
    }
    if publication.needs_publication_recovery {
        ref_updates.push(format!(
            "new annotated recovery tag at overwritten {}",
            publication.remote_sha
        ));
    }
    MutationPlan {
        command: "publish".into(),
        reads: vec![publication.head.clone(), publication.remote_sha.clone()],
        writes: Vec::new(),
        hooks: vec!["pre-push via git push".into()],
        ref_updates,
        paths: Vec::new(),
        requires_confirmation: false,
    }
}
