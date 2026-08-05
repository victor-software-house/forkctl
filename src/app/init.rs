use super::{App, resolve_target};
use crate::manifest::{
    Base, Contracts, Documents, Downstream, Manifest, Patch, PatchKind, Upstream,
};
use crate::process::{capture, run};
use crate::protocol::{CommandResult, ExecutionMode, InitArgs, InitResult, MutationPlan};
use anyhow::{Context, Result, ensure};
use std::ffi::OsString;

impl App {
    pub fn init(&mut self, args: InitArgs, mode: ExecutionMode) -> Result<CommandResult> {
        if self.manifest_present() {
            ensure!(
                !args.is_bootstrap(),
                "bootstrap options require an absent manifest"
            );
            self.manifest()?;
            return self.hydrate(mode);
        }
        self.bootstrap(args, mode)
    }

    fn bootstrap(&mut self, args: InitArgs, mode: ExecutionMode) -> Result<CommandResult> {
        self.require_clean()?;
        let upstream_remote = args
            .upstream_remote
            .context("--upstream-remote is required")?;
        let upstream_url = args.upstream_url.context("--upstream-url is required")?;
        let upstream_ref = args.upstream_ref.context("--upstream-ref is required")?;
        let downstream_remote = args
            .downstream_remote
            .context("--downstream-remote is required")?;
        let downstream_branch = args
            .downstream_branch
            .context("--downstream-branch is required")?;
        let base = args.base.context("--base is required")?;
        let ledger = args.ledger.context("--ledger is required")?;
        let exports = args.exports.context("--exports is required")?;
        let bookkeeping_patch = args
            .bookkeeping_patch
            .context("--bookkeeping-patch is required")?;
        ensure!(
            self.current_branch()? == downstream_branch,
            "current branch must equal downstream branch {downstream_branch}"
        );
        ensure!(
            self.stg_series()?.is_empty(),
            "bootstrap requires no existing StGit patches"
        );

        ensure_remote(&self.repo, &upstream_remote, &upstream_url)?;
        run(
            &self.repo,
            "git",
            ["remote", "set-url", "--push", &upstream_remote, "DISABLED"],
        )?;
        let target = resolve_target(&self.repo, &upstream_remote, &base)?;
        let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        ensure!(
            head == target.commit,
            "bootstrap HEAD is {head}, expected base {}",
            target.commit
        );
        let manifest_relative = self
            .manifest_path
            .strip_prefix(&self.repo)?
            .to_string_lossy()
            .into_owned();
        let mut scope = vec![
            manifest_relative.clone(),
            ledger.clone(),
            format!("{}/**", exports.trim_end_matches('/')),
        ];
        for path in args.bookkeeping_scope {
            if !scope.contains(&path) {
                scope.push(path);
            }
        }
        let patch = Patch {
            name: bookkeeping_patch.clone(),
            kind: PatchKind::Tooling,
            purpose: "Own fork policy, generated evidence, and integration configuration.".into(),
            upstream_status: "inappropriate: downstream-only fork maintenance".into(),
            drop_when: "The downstream fork is retired.".into(),
            checks: Vec::new(),
            scope,
        };
        let manifest = Manifest {
            schema: 1,
            downstream: Downstream {
                remote: downstream_remote,
                branch: downstream_branch,
                recovery_tag_prefix: "forkctl/recovery".into(),
            },
            upstream: Upstream {
                remote: upstream_remote,
                url: upstream_url,
                fetch_ref: upstream_ref,
            },
            base: Base {
                target: target.clone(),
                canonical: target.commit.clone(),
                stack: target.commit.clone(),
            },
            documents: Documents { ledger, exports },
            bookkeeping_patch: bookkeeping_patch.clone(),
            patches: vec![patch.clone()],
            disabled_patches: Vec::new(),
            history: Vec::new(),
            contracts: Contracts {
                allow_base: args.allow_base,
                required_text: args.required_text,
            },
        };
        manifest.validate(&self.repo, &self.manifest_path)?;
        let plan = MutationPlan {
            command: "init".into(),
            reads: vec![head, target.selector.clone()],
            writes: vec![
                manifest_relative,
                manifest.documents.ledger.clone(),
                bookkeeping_patch.clone(),
            ],
            hooks: vec!["commit-msg and pre-commit via StGit".into()],
            ref_updates: Vec::new(),
            paths: patch.scope.clone(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        self.manifest = Some(manifest);
        self.write_manifest()?;
        let ledger_path = self.write_ledger()?;
        run(&self.repo, "stg", ["init"])?;
        run(
            &self.repo,
            "stg",
            ["new", "--message", &patch.message(), &bookkeeping_patch],
        )?;
        let mut add = vec![OsString::from("add"), OsString::from("--")];
        add.push(self.manifest_path.as_os_str().to_owned());
        add.push(ledger_path.as_os_str().to_owned());
        run(&self.repo, "git", add)?;
        run(&self.repo, "stg", ["refresh", "--index"])?;
        let check = self.check_repository(false)?;
        Ok(CommandResult::Init(InitResult {
            created: true,
            hydrated: false,
            manifest: self.manifest_path.display().to_string(),
            base_target: target,
            bookkeeping_commit: self.patch_commit(&bookkeeping_patch)?,
            check,
        }))
    }

    fn hydrate(&mut self, mode: ExecutionMode) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        ensure!(
            self.read_operation()?.is_none(),
            "an operation is in progress"
        );
        let manifest = self.manifest()?.clone();
        let plan = MutationPlan {
            command: "init".into(),
            reads: vec![self.manifest_path.display().to_string()],
            writes: vec!["StGit metadata when absent".into()],
            hooks: Vec::new(),
            ref_updates: manifest
                .recovery_evidence()
                .into_iter()
                .map(|recovery| format!("refs/tags/{}", recovery.tag))
                .collect(),
            paths: Vec::new(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        ensure_remote(
            &self.repo,
            &manifest.upstream.remote,
            &manifest.upstream.url,
        )?;
        run(
            &self.repo,
            "git",
            [
                "remote",
                "set-url",
                "--push",
                &manifest.upstream.remote,
                "DISABLED",
            ],
        )?;
        self.fetch_upstream(true)?;
        self.fetch_target(&manifest.base.target, true)?;
        for recovery in manifest.recovery_evidence() {
            if self
                .local_tag_object(&recovery.tag)
                .is_some_and(|object| object == recovery.tag_object)
            {
                continue;
            }
            let tag_ref = format!("refs/tags/{}", recovery.tag);
            let refspec = format!("+{tag_ref}:{tag_ref}");
            run(
                &self.repo,
                "git",
                [
                    "fetch",
                    "--quiet",
                    "--no-tags",
                    &manifest.downstream.remote,
                    &refspec,
                ],
            )
            .map_err(|error| {
                crate::error::DomainError::check_failed(format!(
                    "history recovery tag {} is unavailable from {}: {error}; restore the published recovery tag before hydrating this clone",
                    recovery.tag, manifest.downstream.remote
                ))
            })?;
        }
        let actual = self.stg_series()?;
        let expected = manifest.patch_names();
        let hydrated = if actual == expected {
            false
        } else {
            ensure!(
                actual.is_empty(),
                "existing StGit stack differs: {}",
                actual.join(", ")
            );
            run(&self.repo, "stg", ["init"])?;
            let command = std::iter::once("uncommit".to_string())
                .chain(expected.iter().rev().cloned())
                .collect::<Vec<_>>();
            run(&self.repo, "stg", command)?;
            true
        };
        let check = self.check_repository(false)?;
        Ok(CommandResult::Init(InitResult {
            created: false,
            hydrated,
            manifest: self.manifest_path.display().to_string(),
            base_target: manifest.base.target,
            bookkeeping_commit: self.patch_commit(&manifest.bookkeeping_patch)?,
            check,
        }))
    }
}

fn ensure_remote(repo: &std::path::Path, name: &str, url: &str) -> Result<()> {
    match capture(repo, "git", ["remote", "get-url", name]) {
        Ok(actual) => {
            ensure!(actual == url, "remote {name} is {actual}, expected {url}");
            Ok(())
        }
        Err(_) => run(repo, "git", ["remote", "add", name, url]),
    }
}
