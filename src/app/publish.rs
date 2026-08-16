use super::{App, recovery_id};
use crate::error::DomainError;
use crate::manifest::RecoveryEvidence;
use crate::process::{capture, run_operator};
use crate::protocol::{CommandResult, ExecutionMode, MutationPlan, PublishResult};
use anyhow::{Context, Result, ensure};

const READY_TO_PUBLISH: &str = "ready_to_publish";

struct Publication {
    head: String,
    remote_sha: String,
    downstream_ref: String,
    lease: String,
    branch_refspec: String,
    fast_forward: bool,
    operation_recovery: Option<RecoveryEvidence>,
    prepared_recovery: Option<RecoveryEvidence>,
    overwritten_tip: Option<String>,
    clears_operation: bool,
}

impl App {
    pub fn publish(&self, mode: ExecutionMode) -> Result<CommandResult> {
        let publication = self.preflight_publication()?;
        if publication.remote_sha == publication.head
            && (!publication.clears_operation
                || self.operation_recovery_is_published(&publication)?)
        {
            return self.already_published(&publication, mode);
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
                operation.new_tip.as_deref() == Some(head.as_str()),
                "operation does not describe current HEAD"
            );
            self.check_operation(operation)?;
        }

        let downstream_ref = self.downstream_ref()?;
        let remote_sha = self.downstream_sha()?;
        let fast_forward = remote_sha == head || self.is_ancestor(&remote_sha, &head)?;
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
        let (operation_recovery, prepared_recovery, overwritten_tip) = match &operation {
            Some(operation) => (
                Some(operation.recovery.clone()),
                operation.publication_recovery.clone(),
                (operation.recovery.old_tip != operation.expected_remote_sha)
                    .then(|| operation.expected_remote_sha.clone()),
            ),
            None => (None, None, (!fast_forward).then(|| remote_sha.clone())),
        };
        if let Some(recovery) = &prepared_recovery {
            ensure!(
                overwritten_tip.as_deref() == Some(recovery.old_tip.as_str()),
                "publication recovery does not preserve the expected remote tip"
            );
        }
        Ok(Publication {
            head,
            lease: format!("--force-with-lease={downstream_ref}:{remote_sha}"),
            branch_refspec: format!("HEAD:{downstream_ref}"),
            remote_sha,
            downstream_ref,
            fast_forward,
            operation_recovery,
            prepared_recovery,
            overwritten_tip,
            clears_operation: operation.is_some(),
        })
    }

    fn already_published(
        &self,
        publication: &Publication,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(MutationPlan {
                command: "publish".into(),
                reads: vec![publication.head.clone(), publication.remote_sha.clone()],
                writes: Vec::new(),
                hooks: Vec::new(),
                ref_updates: Vec::new(),
                paths: Vec::new(),
                requires_confirmation: false,
            }));
        }
        if publication.clears_operation {
            self.complete_published_operation()?;
        }
        Ok(CommandResult::Publish(PublishResult {
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
        }))
    }

    fn operation_recovery_is_published(&self, publication: &Publication) -> Result<bool> {
        if let Some(recovery) = &publication.operation_recovery
            && !self.recovery_ref_is_published(&recovery.tag, &recovery.tag_object)?
        {
            return Ok(false);
        }
        match &publication.overwritten_tip {
            Some(_) => match &publication.prepared_recovery {
                Some(recovery) => {
                    self.recovery_ref_is_published(&recovery.tag, &recovery.tag_object)
                }
                None => Ok(false),
            },
            None => Ok(true),
        }
    }

    fn recovery_ref_is_published(&self, tag: &str, tag_object: &str) -> Result<bool> {
        let manifest = self.manifest()?;
        let git_ref = format!("refs/tags/{tag}");
        let line = capture(
            &self.repo,
            "git",
            ["ls-remote", &manifest.downstream.remote, &git_ref],
        )?;
        if line.is_empty() {
            return Ok(false);
        }
        let mut fields = line.split_whitespace();
        let actual = fields.next().context("remote ref output has no SHA")?;
        ensure!(
            fields.next() == Some(git_ref.as_str()),
            "unexpected remote ref output: {line}"
        );
        if actual != tag_object {
            return Err(DomainError::publication_ref_mismatch(
                manifest.downstream.remote.clone(),
                git_ref,
                tag_object.to_string(),
                actual.to_string(),
            )
            .into());
        }
        Ok(true)
    }

    fn publication_recovery_tags(
        &self,
        publication: &Publication,
    ) -> Result<Vec<(String, String)>> {
        let mut tags = Vec::new();
        if let Some(recovery) = &publication.operation_recovery {
            tags.push((recovery.tag.clone(), recovery.tag_object.clone()));
        }
        if let Some(tip) = &publication.overwritten_tip {
            let recovery = if let Some(recovery) = &publication.prepared_recovery {
                recovery.clone()
            } else {
                let id = recovery_id(tip)?;
                let (tag, tag_object) = self.create_recovery_tag(&id, tip, "publication")?;
                let recovery = RecoveryEvidence {
                    tag,
                    tag_object,
                    old_base: tip.clone(),
                    old_tip: tip.clone(),
                };
                if publication.clears_operation {
                    let mut operation = self.read_operation()?.ok_or_else(|| {
                        DomainError::operation_conflict(
                            "publication operation disappeared before recovery evidence was recorded",
                            None,
                        )
                    })?;
                    operation.publication_recovery = Some(recovery.clone());
                    self.write_operation(&operation)?;
                }
                recovery
            };
            tags.push((recovery.tag, recovery.tag_object));
        }
        for (tag, object) in &tags {
            let _ = self.recovery_ref_is_published(tag, object)?;
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
            "--progress".to_string(),
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

        if let Err(error) = run_operator(&self.repo, "git", push) {
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
            self.complete_published_operation()?;
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
    if let Some(recovery) = &publication.prepared_recovery {
        ref_updates.push(format!("refs/tags/{}", recovery.tag));
    } else if let Some(tip) = &publication.overwritten_tip {
        ref_updates.push(format!("new annotated recovery tag at overwritten {tip}"));
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
