use super::{App, recovery_id};
use crate::error::DomainError;
use crate::manifest::{PublishMode, RecoveryEvidence};
use crate::process::{capture, run, run_operator};
use crate::protocol::{CommandResult, ExecutionMode, MutationPlan, PublishArgs, PublishResult};
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
    pub fn publish(&mut self, args: &PublishArgs, mode: ExecutionMode) -> Result<CommandResult> {
        if args.promote && args.mode.is_some() {
            return Err(DomainError::invalid_request(
                "publish --promote cannot be combined with --rewrite, --append, or --propose",
            )
            .into());
        }
        if let Some(default) = args.set_default {
            self.persist_publish_mode(default, mode)?;
            if args.mode.is_none() && !args.promote {
                return self.publish_default_updated(default, mode);
            }
        }
        if args.promote {
            return self.publish_promote(args.proposal.as_deref(), mode);
        }
        match args.mode.unwrap_or(self.manifest()?.downstream.publish) {
            PublishMode::Rewrite => self.publish_rewrite(mode),
            PublishMode::Append => self.publish_append(mode),
            PublishMode::Propose => self.publish_propose(mode),
        }
    }

    fn persist_publish_mode(&mut self, publish: PublishMode, mode: ExecutionMode) -> Result<()> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        if mode == ExecutionMode::Plan {
            return Ok(());
        }
        let mut proposed = self.manifest()?.clone();
        proposed.downstream.publish = publish;
        proposed
            .validate(&self.repo, &self.manifest_path)
            .map_err(|error| DomainError::invalid_request(error.to_string()))?;
        self.manifest = Some(proposed);
        self.write_manifest()?;
        let ledger = self.write_ledger()?;
        self.refresh_bookkeeping(&[self.manifest_path.clone(), ledger])?;
        Ok(())
    }

    fn publish_default_updated(
        &self,
        publish: PublishMode,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(MutationPlan {
                command: "publish".into(),
                reads: vec![self.manifest_path.display().to_string()],
                writes: vec![self.manifest_path.display().to_string()],
                hooks: vec!["bookkeeping refresh".into()],
                ref_updates: Vec::new(),
                paths: Vec::new(),
                requires_confirmation: false,
            }));
        }
        let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        Ok(CommandResult::Publish(PublishResult {
            branch: self.manifest()?.downstream.branch.clone(),
            head,
            already_published: true,
            fast_forward: true,
            mode: publish,
            recovery_tags: Vec::new(),
            pushed_refs: Vec::new(),
            expected_lease: String::new(),
            proposal_branch: None,
            proposal_url: None,
        }))
    }

    fn publish_rewrite(&self, mode: ExecutionMode) -> Result<CommandResult> {
        let publication = self.preflight_publication()?;
        if publication.remote_sha == publication.head
            && (!publication.clears_operation
                || self.operation_recovery_is_published(&publication)?)
        {
            return self.already_published(&publication, PublishMode::Rewrite, mode);
        }
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(publication_plan(&publication)));
        }
        self.execute_publication(&publication, PublishMode::Rewrite)
    }

    fn publish_append(&self, mode: ExecutionMode) -> Result<CommandResult> {
        let publication = self.preflight_publication()?;
        if publication.remote_sha == publication.head
            && (!publication.clears_operation
                || self.operation_recovery_is_published(&publication)?)
        {
            return self.already_published(&publication, PublishMode::Append, mode);
        }
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(publication_plan(&publication)));
        }
        if !publication.fast_forward {
            let short = &publication.remote_sha[..publication.remote_sha.len().min(12)];
            let message = format!("forkctl: keep published history {short}");
            run(
                &self.repo,
                "git",
                [
                    "merge",
                    "-s",
                    "ours",
                    "--no-ff",
                    "-m",
                    &message,
                    &publication.remote_sha,
                ],
            )?;
        }
        let publication = self.preflight_publication()?;
        ensure!(
            publication.fast_forward,
            "append epoch did not produce a fast-forward"
        );
        self.execute_publication(&publication, PublishMode::Append)
    }

    fn proposal_branch(&self) -> Result<String> {
        Ok(format!(
            "forkctl/proposal/{}",
            self.manifest()?.downstream.branch
        ))
    }

    fn publish_propose(&self, mode: ExecutionMode) -> Result<CommandResult> {
        let publication = self.preflight_publication()?;
        if publication.remote_sha == publication.head {
            return self.already_published(&publication, PublishMode::Propose, mode);
        }
        let proposal_branch = self.proposal_branch()?;
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(MutationPlan {
                command: "publish".into(),
                reads: vec![publication.head.clone(), publication.remote_sha.clone()],
                writes: Vec::new(),
                hooks: vec!["git push of proposal branch".into()],
                ref_updates: vec![format!("HEAD^{{tree}} -> refs/heads/{proposal_branch}")],
                paths: Vec::new(),
                requires_confirmation: false,
            }));
        }
        let tree = capture(&self.repo, "git", ["rev-parse", "HEAD^{tree}"])?;
        let review = capture(
            &self.repo,
            "git",
            [
                "commit-tree",
                &tree,
                "-p",
                &publication.remote_sha,
                "-m",
                "forkctl: proposal of net tree",
            ],
        )?;
        let local_ref = format!("refs/heads/{proposal_branch}");
        run(&self.repo, "git", ["update-ref", &local_ref, &review])?;
        let remote = self.manifest()?.downstream.remote.clone();
        run_operator(
            &self.repo,
            "git",
            [
                "push",
                "--progress",
                &remote,
                &format!("{local_ref}:{local_ref}"),
            ],
        )?;
        let proposal_url = self.open_proposal_pr(&proposal_branch).ok().flatten();
        Ok(CommandResult::Publish(PublishResult {
            branch: self.manifest()?.downstream.branch.clone(),
            head: review,
            already_published: false,
            fast_forward: false,
            mode: PublishMode::Propose,
            recovery_tags: Vec::new(),
            pushed_refs: vec![format!("{local_ref}:{local_ref}")],
            expected_lease: publication.remote_sha,
            proposal_branch: Some(proposal_branch),
            proposal_url,
        }))
    }

    fn open_proposal_pr(&self, proposal_branch: &str) -> Result<Option<String>> {
        let base = self.manifest()?.downstream.branch.clone();
        let output = std::process::Command::new("gh")
            .args([
                "pr",
                "create",
                "--draft",
                "--base",
                &base,
                "--head",
                proposal_branch,
                "--title",
                "forkctl: proposal of net tree",
                "--body",
                "Exact candidate is the current stack tip. Promote with `mise run fork -- publish --promote`.",
            ])
            .current_dir(&self.repo)
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        let url = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.starts_with("http"))
            .unwrap_or("")
            .trim()
            .to_string();
        Ok((!url.is_empty()).then_some(url))
    }

    fn publish_promote(
        &self,
        proposal: Option<&str>,
        mode: ExecutionMode,
    ) -> Result<CommandResult> {
        let publication = self.preflight_publication()?;
        let proposal_branch = proposal
            .map(ToOwned::to_owned)
            .unwrap_or(self.proposal_branch()?);
        let review = capture(
            &self.repo,
            "git",
            ["rev-parse", &format!("{proposal_branch}^{{commit}}")],
        )
        .or_else(|_| {
            capture(
                &self.repo,
                "git",
                [
                    "rev-parse",
                    &format!(
                        "refs/remotes/{}/{proposal_branch}",
                        self.manifest()?.downstream.remote
                    ),
                ],
            )
        })?;
        let review_tree = capture(
            &self.repo,
            "git",
            ["rev-parse", &format!("{review}^{{tree}}")],
        )?;
        let head_tree = capture(&self.repo, "git", ["rev-parse", "HEAD^{tree}"])?;
        ensure!(
            review_tree == head_tree,
            "proposal {proposal_branch} tree {review_tree} does not match HEAD {head_tree}; check out the candidate stack first"
        );
        let parent = capture(&self.repo, "git", ["rev-parse", &format!("{review}^")])?;
        if parent != publication.remote_sha {
            return Err(DomainError::remote_advanced(
                self.manifest()?.downstream.remote.clone(),
                self.downstream_ref()?,
                parent,
                publication.remote_sha,
            )
            .into());
        }
        self.publish_rewrite(mode)
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
        publish_mode: PublishMode,
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
            mode: publish_mode,
            recovery_tags: Vec::new(),
            pushed_refs: Vec::new(),
            expected_lease: publication.remote_sha.clone(),
            proposal_branch: None,
            proposal_url: None,
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

    fn execute_publication(
        &self,
        publication: &Publication,
        publish_mode: PublishMode,
    ) -> Result<CommandResult> {
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
            mode: publish_mode,
            recovery_tags: recovery_tags
                .into_iter()
                .map(|(tag, object)| format!("{tag} -> {object}"))
                .collect(),
            pushed_refs,
            expected_lease: publication.remote_sha.clone(),
            proposal_branch: None,
            proposal_url: None,
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
