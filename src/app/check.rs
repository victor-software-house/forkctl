use super::App;
use crate::error::{AppResult as Result, DomainError, InternalResultExt as _};
use crate::ledger;
use crate::manifest::{HistoryEvent, Patch};
use crate::process::{capture, run};
use crate::protocol::{CheckArgs, CheckResult, CheckScope};
use crate::state::ActivePatchState;
use std::fs;

macro_rules! ensure_check {
    ($condition:expr, $($message:tt)*) => {
        if !$condition {
            return Err(DomainError::check_failed(format!($($message)*)).into());
        }
    };
}

impl App {
    pub fn check(&self, args: &CheckArgs) -> Result<CheckResult> {
        match args.scope {
            CheckScope::Repository => {
                if args.patch.is_some() {
                    return Err(DomainError::invalid_request("--patch requires --staged").into());
                }
                self.check_repository(false)
            }
            CheckScope::Staged => self.check_staged(args.patch.as_deref()),
        }
    }

    pub(super) fn check_repository(&self, allow_active: bool) -> Result<CheckResult> {
        self.check_repository_state(allow_active, true)
    }

    pub(super) fn check_restored_repository(&self, allow_active: bool) -> Result<CheckResult> {
        self.check_repository_state(allow_active, false)
    }

    fn check_repository_state(
        &self,
        allow_active: bool,
        check_current_operation: bool,
    ) -> Result<CheckResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        let manifest = self.manifest()?;
        if !allow_active && let Some(active) = self.read_active()? {
            return Err(DomainError::active_patch_exists(active.name().to_string()).into());
        }
        self.check_remotes()?;
        for revision in [
            manifest.base.canonical.as_str(),
            manifest.base.stack.as_str(),
        ] {
            run(
                &self.repo,
                "git",
                ["cat-file", "-e", &format!("{revision}^{{commit}}")],
            )?;
        }
        self.verify_target_evidence(&manifest.base.target)?;
        self.check_history()?;
        let actual_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        ensure_check!(
            actual_base == manifest.base.stack,
            "StGit base is {actual_base}, expected {}",
            manifest.base.stack
        );
        let merge_base = capture(
            &self.repo,
            "git",
            [
                "merge-base",
                manifest.base.stack.as_str(),
                self.upstream_tracking_ref()?.as_str(),
            ],
        )?;
        ensure_check!(
            merge_base == manifest.base.canonical,
            "canonical merge base is {merge_base}, expected {}",
            manifest.base.canonical
        );
        let actual_stack = self.stg_series()?;
        let expected_stack = manifest.patch_names();
        ensure_check!(
            actual_stack == expected_stack,
            "StGit patch order differs: got {}, expected {}",
            actual_stack.join(", "),
            expected_stack.join(", ")
        );
        ensure_check!(
            capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? == "0",
            "all fork patches must be applied"
        );
        self.check_allowed_diff(
            &manifest.base.canonical,
            &manifest.base.stack,
            &manifest.contracts.allow_base,
            "pre-stack drift",
        )?;
        for patch in &manifest.patches {
            let commit = self.patch_commit(&patch.name)?;
            let paths = self.patch_paths(&commit)?;
            ensure_check!(!paths.is_empty(), "patch {} is empty", patch.name);
            Self::check_patch_paths(patch, &paths)?;
            self.check_patch_commit(patch, &commit)?;
        }
        self.check_required_text()?;
        self.check_ledger()?;
        self.check_exports()?;
        let findings = self.run_declared_checks()?;
        if !findings.is_empty() {
            return Err(DomainError::declared_checks_failed(findings).into());
        }
        let expected_tree = self.expected_reconstructed_tree()?;
        let reconstructed_tree = self.reconstruct_tree()?;
        ensure_check!(
            reconstructed_tree == expected_tree,
            "exported patches reconstruct {reconstructed_tree}, expected {expected_tree}"
        );
        if check_current_operation && let Some(operation) = self.read_operation()? {
            self.check_operation(&operation)?;
        }
        Ok(CheckResult {
            scope: CheckScope::Repository,
            ok: true,
            patch: None,
            checked_paths: Vec::new(),
            rejected_paths: Vec::new(),
            findings: Vec::new(),
            canonical_base: Some(manifest.base.canonical.clone()),
            stack_base: Some(manifest.base.stack.clone()),
            patch_count: Some(expected_stack.len()),
            declared_checks: Some(manifest.check_count()),
            source_tree: Some(expected_tree),
        })
    }

    fn check_staged(&self, requested: Option<&str>) -> Result<CheckResult> {
        let inventory = self.worktree_inventory()?;
        if inventory.staged.is_empty() {
            return Ok(CheckResult {
                scope: CheckScope::Staged,
                ok: true,
                patch: requested.map(str::to_string),
                checked_paths: Vec::new(),
                rejected_paths: Vec::new(),
                findings: Vec::new(),
                canonical_base: None,
                stack_base: None,
                patch_count: None,
                declared_checks: None,
                source_tree: None,
            });
        }
        let (name, patch) = self.resolve_patch(requested)?;
        let rejected = inventory
            .staged
            .iter()
            .filter(|path| !patch.owns(path))
            .cloned()
            .collect::<Vec<_>>();
        if !rejected.is_empty() {
            return Err(DomainError::staged_scope_violation(name, rejected).into());
        }
        Ok(CheckResult {
            scope: CheckScope::Staged,
            ok: true,
            patch: Some(name),
            checked_paths: inventory.staged,
            rejected_paths: Vec::new(),
            findings: Vec::new(),
            canonical_base: None,
            stack_base: None,
            patch_count: None,
            declared_checks: None,
            source_tree: None,
        })
    }

    pub(super) fn resolve_patch(&self, requested: Option<&str>) -> Result<(String, Patch)> {
        let active = self.read_active()?;
        let name = if let Some(name) = requested {
            name.to_string()
        } else {
            active
                .as_ref()
                .map(|active| active.name().to_string())
                .ok_or_else(DomainError::active_patch_required)?
        };
        let patch = match active {
            Some(ActivePatchState::Draft { metadata }) if metadata.name == name => metadata,
            _ => self.manifest()?.patch(&name).cloned().ok_or_else(|| {
                DomainError::patch_not_found(
                    &name,
                    self.manifest()
                        .map_or_else(|_| Vec::new(), crate::manifest::Manifest::patch_names),
                    self.read_active()
                        .ok()
                        .flatten()
                        .map(|active| active.name().to_string()),
                )
            })?,
        };
        Ok((name, patch))
    }

    fn check_remotes(&self) -> Result<()> {
        let manifest = self.manifest()?;
        capture(
            &self.repo,
            "git",
            ["remote", "get-url", &manifest.downstream.remote],
        )?;
        let actual_url = capture(
            &self.repo,
            "git",
            ["remote", "get-url", &manifest.upstream.remote],
        )?;
        ensure_check!(
            actual_url == manifest.upstream.url,
            "remote {} is {actual_url}, expected {}",
            manifest.upstream.remote,
            manifest.upstream.url
        );
        let push_url = capture(
            &self.repo,
            "git",
            ["remote", "get-url", "--push", &manifest.upstream.remote],
        )?;
        ensure_check!(
            push_url == "DISABLED",
            "remote {} push URL is {push_url}, expected DISABLED",
            manifest.upstream.remote
        );
        run(
            &self.repo,
            "git",
            [
                "cat-file",
                "-e",
                &format!("{}^{{commit}}", self.upstream_tracking_ref()?),
            ],
        )?;
        Ok(())
    }

    fn check_patch_paths(patch: &Patch, paths: &[String]) -> Result<()> {
        for path in paths {
            ensure_check!(
                patch.owns(path),
                "path in patch {} is outside scope: {path}",
                patch.name
            );
        }
        Ok(())
    }

    fn check_allowed_diff(
        &self,
        from: &str,
        to: &str,
        scope: &[String],
        label: &str,
    ) -> Result<()> {
        let paths = capture(&self.repo, "git", ["diff", "--name-only", from, to])?;
        for path in paths.lines().filter(|line| !line.is_empty()) {
            ensure_check!(
                scope
                    .iter()
                    .any(|pattern| crate::manifest::scope_matches(pattern, path)),
                "{label} is outside scope: {path}"
            );
        }
        Ok(())
    }

    fn check_patch_commit(&self, patch: &Patch, commit: &str) -> Result<()> {
        for (key, expected) in [
            ("Downstream-Reason", patch.purpose.as_str()),
            ("Upstream-Status", patch.upstream_status.as_str()),
            ("Drop-When", patch.drop_when.as_str()),
        ] {
            let format = format!("%(trailers:key={key},valueonly,unfold=true)");
            let actual = capture(
                &self.repo,
                "git",
                ["log", "-1", &format!("--format={format}"), commit],
            )?;
            ensure_check!(
                actual == expected,
                "patch {} trailer {key} is {actual:?}, expected {expected:?}",
                patch.name
            );
        }
        Ok(())
    }

    fn check_required_text(&self) -> Result<()> {
        self.validate_required_text(&self.manifest()?.contracts.required_text)
    }

    pub(super) fn validate_required_text(
        &self,
        required_text: &[crate::manifest::RequiredText],
    ) -> Result<()> {
        for required in required_text {
            let path = self.repo.join(&required.path);
            let contents =
                fs::read_to_string(&path).internal(format!("read {}", path.display()))?;
            ensure_check!(
                contents.contains(&required.contains),
                "required contract missing from {}: {}",
                required.path,
                required.contains
            );
        }
        Ok(())
    }

    fn check_ledger(&self) -> Result<()> {
        let manifest = self.manifest()?;
        let path = self.repo.join(&manifest.documents.ledger);
        let actual = fs::read(&path).internal(format!("read {}", path.display()))?;
        let expected = ledger::render(manifest).internal("render expected patch ledger")?;
        ensure_check!(
            actual == expected.as_bytes(),
            "generated ledger differs: {}",
            path.display()
        );
        Ok(())
    }

    fn check_exports(&self) -> Result<()> {
        let manifest = self.manifest()?;
        let expected = manifest
            .source_exports()
            .into_iter()
            .map(|export| export.path.clone())
            .collect::<HashSet<_>>();
        for export in manifest.source_exports() {
            let path = self.repo.join(&export.path);
            let actual = fs::read(&path).internal(format!("read {}", path.display()))?;
            let expected = self.export_patch(export.patch)?;
            ensure_check!(
                actual == expected,
                "export differs for patch {}: {}",
                export.patch.name,
                export.path
            );
        }
        let exports = self.repo.join(&manifest.documents.exports);
        let mut unexpected = if exports.exists() {
            fs::read_dir(&exports)
                .internal(format!("read {}", exports.display()))?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::io::Result<Vec<_>>>()
                .internal(format!("read entries in {}", exports.display()))?
                .into_iter()
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "patch")
                })
                .map(|path| {
                    path.strip_prefix(&self.repo)
                        .map(|path| path.to_string_lossy().into_owned())
                })
                .collect::<std::result::Result<Vec<_>, _>>()
                .internal(format!("make exports relative to {}", self.repo.display()))?
        } else {
            Vec::new()
        };
        unexpected.retain(|path| !expected.contains(path));
        unexpected.sort();
        ensure_check!(
            unexpected.is_empty(),
            "unexpected generated patch exports: {}",
            unexpected.join(", ")
        );
        Ok(())
    }

    fn check_history(&self) -> Result<()> {
        for recovery in self.manifest()?.recovery_evidence() {
            self.check_recovery(recovery)?;
        }
        for event in &self.manifest()?.history {
            match event {
                HistoryEvent::Rebase {
                    target,
                    recovery,
                    dropped,
                    path_changes,
                } => {
                    self.verify_target_evidence(target)?;
                    let commits = capture(
                        &self.repo,
                        "git",
                        [
                            "rev-list",
                            "--reverse",
                            &format!("{}..{}", recovery.old_base, recovery.old_tip),
                        ],
                    )?;
                    let commits = commits.lines().collect::<HashSet<_>>();
                    for item in dropped {
                        ensure_check!(
                            commits.contains(item.commit.as_str()),
                            "historical patch {} is outside recovery stack",
                            item.patch.name
                        );
                        self.check_historical_patch(&item.patch, &item.commit)?;
                    }
                    for item in path_changes {
                        ensure_check!(
                            commits.contains(item.commit.as_str()),
                            "replayed patch {} is outside recovery stack",
                            item.patch
                        );
                        let paths = self.patch_paths(&item.commit)?;
                        for path in &item.lost_paths {
                            ensure_check!(
                                paths.contains(path),
                                "replayed patch {} did not touch recorded path {path}",
                                item.patch
                            );
                        }
                    }
                }
                HistoryEvent::PatchRemoved { record }
                | HistoryEvent::PatchEnabled { record, .. } => {
                    self.check_historical_patch(&record.patch, &record.commit)?;
                }
            }
        }
        for record in &self.manifest()?.disabled_patches {
            self.check_historical_patch(&record.patch, &record.commit)?;
        }
        Ok(())
    }

    fn check_recovery(&self, recovery: &crate::manifest::RecoveryEvidence) -> Result<()> {
        run(
            &self.repo,
            "git",
            [
                "cat-file",
                "-e",
                &format!("{}^{{tag}}", recovery.tag_object),
            ],
        )
        .map_err(|error| {
            DomainError::check_failed(format!(
                "history recovery tag {} object {} is unavailable: {error}; restore the published recovery tag or run forkctl init to hydrate it",
                recovery.tag, recovery.tag_object
            ))
        })?;
        let local_object = capture(
            &self.repo,
            "git",
            ["rev-parse", &format!("refs/tags/{}", recovery.tag)],
        )
        .map_err(|error| {
            DomainError::check_failed(format!(
                "history recovery tag {} is missing locally: {error}; run forkctl init to hydrate published recovery tags",
                recovery.tag
            ))
        })?;
        ensure_check!(
            local_object == recovery.tag_object,
            "history recovery tag {} differs",
            recovery.tag
        );
        let old_tip = capture(
            &self.repo,
            "git",
            ["rev-parse", &format!("{}^{{commit}}", recovery.tag_object)],
        )?;
        ensure_check!(
            old_tip == recovery.old_tip,
            "history recovery tag peels to wrong tip"
        );
        Ok(())
    }

    fn check_historical_patch(&self, patch: &crate::manifest::Patch, commit: &str) -> Result<()> {
        let paths = self.patch_paths(commit)?;
        ensure_check!(
            !paths.is_empty(),
            "historical patch {} is empty",
            patch.name
        );
        Self::check_patch_paths(patch, &paths)?;
        self.check_patch_commit(patch, commit)
    }

    pub(super) fn check_operation(&self, operation: &crate::state::OperationState) -> Result<()> {
        let local_object = capture(
            &self.repo,
            "git",
            [
                "rev-parse",
                &format!("refs/tags/{}", operation.recovery.tag),
            ],
        )?;
        ensure_check!(
            local_object == operation.recovery.tag_object,
            "operation recovery tag object differs"
        );
        let peeled = capture(
            &self.repo,
            "git",
            [
                "rev-parse",
                &format!("{}^{{commit}}", operation.recovery.tag_object),
            ],
        )?;
        ensure_check!(
            peeled == operation.old_tip,
            "operation recovery tag peels to wrong tip"
        );
        if let Some(recovery) = &operation.publication_recovery {
            let local_object = capture(
                &self.repo,
                "git",
                ["rev-parse", &format!("refs/tags/{}", recovery.tag)],
            )?;
            ensure_check!(
                local_object == recovery.tag_object,
                "publication recovery tag object differs"
            );
            let peeled = capture(
                &self.repo,
                "git",
                ["rev-parse", &format!("{}^{{commit}}", recovery.tag_object)],
            )?;
            ensure_check!(
                peeled == recovery.old_tip,
                "publication recovery tag peels to wrong tip"
            );
        }
        if let Some(new_tip) = &operation.new_tip {
            ensure_check!(
                capture(&self.repo, "git", ["rev-parse", "HEAD"])? == *new_tip,
                "operation new tip differs from HEAD"
            );
        }
        if let Some(report) = &operation.report {
            ensure_check!(
                self.file_object_id(std::path::Path::new(&report.path))? == report.object_id,
                "operation report differs"
            );
        }
        Ok(())
    }
}

use std::collections::HashSet;
