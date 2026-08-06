use super::{App, resolve_target};
use crate::error::{AppResult as Result, DomainError, InternalResultExt as _};
use crate::manifest::{
    Base, Contracts, Documents, Downstream, Manifest, Patch, PatchKind, Upstream,
};
use crate::process::{capture, run};
use crate::protocol::{CommandResult, ExecutionMode, InitArgs, InitResult, MutationPlan};
use std::ffi::OsString;

impl App {
    pub fn init(&mut self, args: InitArgs, mode: ExecutionMode) -> Result<CommandResult> {
        if self.manifest_present() {
            if args.is_bootstrap() {
                return Err(DomainError::invalid_request(
                    "bootstrap options require an absent manifest",
                )
                .into());
            }
            self.manifest()?;
            return self.hydrate(mode);
        }
        self.bootstrap(args, mode)
    }

    fn bootstrap(&mut self, args: InitArgs, mode: ExecutionMode) -> Result<CommandResult> {
        self.require_clean()?;
        let upstream_remote = required(args.upstream_remote, "--upstream-remote")?;
        let upstream_url = required(args.upstream_url, "--upstream-url")?;
        let upstream_ref = required(args.upstream_ref, "--upstream-ref")?;
        let downstream_remote = required(args.downstream_remote, "--downstream-remote")?;
        let downstream_branch = required(args.downstream_branch, "--downstream-branch")?;
        let base = required(args.base, "--base")?;
        let ledger = required(args.ledger, "--ledger")?;
        let exports = required(args.exports, "--exports")?;
        let bookkeeping_patch = required(args.bookkeeping_patch, "--bookkeeping-patch")?;
        if self.current_branch()? != downstream_branch {
            return Err(DomainError::invalid_request(format!(
                "current branch must equal downstream branch {downstream_branch}"
            ))
            .into());
        }
        if !self.stg_series()?.is_empty() {
            return Err(DomainError::invalid_request(
                "bootstrap requires no existing StGit patches",
            )
            .into());
        }

        ensure_remote(&self.repo, &upstream_remote, &upstream_url)?;
        run(
            &self.repo,
            "git",
            ["remote", "set-url", "--push", &upstream_remote, "DISABLED"],
        )?;
        let target = resolve_target(&self.repo, &upstream_remote, &base)?;
        let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        if head != target.commit {
            return Err(DomainError::invalid_request(format!(
                "bootstrap HEAD is {head}, expected base {}",
                target.commit
            ))
            .into());
        }
        let manifest_relative = self
            .manifest_path
            .strip_prefix(&self.repo)
            .internal(format!(
                "make {} relative to {}",
                self.manifest_path.display(),
                self.repo.display()
            ))?
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
        manifest
            .validate(&self.repo, &self.manifest_path)
            .internal("validate bootstrapped manifest")?;
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
        self.require_no_operation()?;
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
            if !actual.is_empty() {
                return Err(DomainError::invalid_request(format!(
                    "existing StGit stack differs: {}",
                    actual.join(", ")
                ))
                .into());
            }
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
            if actual != url {
                return Err(DomainError::invalid_request(format!(
                    "remote {name} is {actual}, expected {url}"
                ))
                .into());
            }
            Ok(())
        }
        Err(_) => run(repo, "git", ["remote", "add", name, url]),
    }
}

fn required<T>(value: Option<T>, option: &str) -> Result<T> {
    value.ok_or_else(|| DomainError::invalid_request(format!("{option} is required")).into())
}
