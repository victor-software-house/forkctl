use super::{App, write_atomic};
use crate::manifest::{BaseTarget, DroppedPatch, HistoryEvent};
use crate::process::{capture, run};
use crate::protocol::{CommandResult, ExecutionMode, MutationPlan, RebaseResult};
use crate::report::{self, ExportEvidence, RebaseReport};
use crate::state::{OperationKind, OperationState, ReportEvidence};
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::PathBuf;

impl App {
    pub fn rebase(&mut self, selector: &str, mode: ExecutionMode) -> Result<CommandResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        ensure!(
            self.read_active()?.is_none(),
            "active patch must be finished before rebase"
        );
        ensure!(
            self.read_operation()?.is_none(),
            "another forkctl operation is in progress"
        );
        self.check_repository(false)?;
        self.fetch_upstream(false)?;
        let target = self.resolve_target(selector)?;
        let plan = MutationPlan {
            command: "rebase".into(),
            reads: vec![
                self.manifest_path.display().to_string(),
                target.selector.clone(),
            ],
            writes: vec![
                "StGit series".into(),
                "manifest base/history".into(),
                "generated evidence".into(),
            ],
            hooks: Vec::new(),
            ref_updates: vec!["annotated recovery tag".into()],
            paths: Vec::new(),
            requires_confirmation: false,
        };
        if mode == ExecutionMode::Plan {
            return Ok(CommandResult::Plan(plan));
        }
        let mut operation = self.create_operation(OperationKind::Rebase, Some(target.clone()))?;
        operation.phase = "replaying".into();
        self.write_operation(&operation)?;
        if let Err(error) = run(
            &self.repo,
            "stg",
            ["rebase", "--merged", target.commit.as_str()],
        ) {
            operation.phase = "conflict".into();
            operation.next_actions = vec![
                "resolve conflicts".into(),
                "stg add --update".into(),
                "stg refresh".into(),
                "stg goto <bookkeeping-patch>".into(),
                "forkctl operation continue".into(),
            ];
            self.write_operation(&operation)?;
            return Err(error).context(format!(
                "rebase stopped; recovery tag {}; resolve conflicts and run forkctl operation continue",
                operation.recovery.tag
            ));
        }
        Ok(CommandResult::Rebase(Box::new(
            self.continue_rebase(operation)?,
        )))
    }

    pub(super) fn continue_rebase(
        &mut self,
        mut operation: OperationState,
    ) -> Result<RebaseResult> {
        ensure!(
            operation.kind == OperationKind::Rebase,
            "current operation is not rebase"
        );
        let target = operation
            .target
            .clone()
            .context("rebase operation has no target")?;
        ensure!(
            capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? == "0",
            "rebase is incomplete; apply all patches before continuing"
        );
        let new_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        ensure!(
            new_base == target.commit,
            "StGit base is {new_base}, expected {}",
            target.commit
        );
        let bookkeeping = self.manifest()?.bookkeeping_patch.clone();
        ensure!(
            capture(&self.repo, "stg", ["top"])? == bookkeeping,
            "bookkeeping patch must be top"
        );
        let dropped = self.drop_upstream_merged(&target, &operation)?;
        let upstream_tracking = self.upstream_tracking_ref()?;
        let canonical = capture(
            &self.repo,
            "git",
            ["merge-base", &new_base, &upstream_tracking],
        )?;
        let manifest = self.manifest_mut()?;
        manifest.base.target = target.clone();
        manifest.base.stack.clone_from(&new_base);
        manifest.base.canonical = canonical;
        if !dropped.items.is_empty() {
            manifest.history.push(HistoryEvent::Rebase {
                target: target.clone(),
                recovery: operation.recovery.clone(),
                dropped: dropped.items.clone(),
            });
        }
        self.write_manifest()?;
        let exports = self.write_exports()?;
        let ledger = self.write_ledger()?;
        let generated = std::iter::once(self.manifest_path.clone())
            .chain(std::iter::once(ledger))
            .chain(exports)
            .chain(dropped.removed_exports)
            .collect::<Vec<PathBuf>>();
        self.refresh_bookkeeping(&generated)?;
        let new_tip = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        operation.phase = "replayed".into();
        operation.new_base = Some(new_base.clone());
        operation.new_tip = Some(new_tip.clone());
        operation.next_actions = vec!["review range-diff report".into(), "forkctl publish".into()];
        self.write_operation(&operation)?;
        let check = self.check_repository(false)?;
        let range = capture(
            &self.repo,
            "git",
            [
                "range-diff",
                "--no-color",
                &format!("{}..{}", operation.old_base, operation.old_tip),
                &format!("{new_base}..{new_tip}"),
            ],
        )?;
        let report_path = self.report_path(&new_tip)?;
        let report = self.render_report(&operation, &new_base, &new_tip, &range)?;
        write_atomic(&report_path, report.as_bytes())?;
        let report_object_id = self.file_object_id(&report_path)?;
        operation.report = Some(ReportEvidence {
            path: report_path.display().to_string(),
            object_id: report_object_id.clone(),
        });
        operation.phase = "ready_to_publish".into();
        self.write_operation(&operation)?;
        self.check_operation(&operation)?;
        Ok(RebaseResult {
            selected_target: target.selector,
            old_base: operation.old_base,
            old_tip: operation.old_tip,
            new_base,
            new_tip,
            recovery_tag: operation.recovery.tag,
            recovery_tag_object: operation.recovery.tag_object,
            report_path: report_path.display().to_string(),
            report_object_id,
            dropped_patches: dropped.names,
            check,
        })
    }

    fn drop_upstream_merged(
        &mut self,
        _target: &BaseTarget,
        operation: &OperationState,
    ) -> Result<DroppedPatches> {
        let bookkeeping = self.manifest()?.bookkeeping_patch.clone();
        let patches = self.manifest()?.patches.clone();
        let mut dropped = Vec::new();
        for patch in patches {
            if patch.name == bookkeeping {
                continue;
            }
            let commit = self.patch_commit(&patch.name)?;
            if self.patch_paths(&commit)?.is_empty() {
                let old_commit = operation
                    .old_patches
                    .iter()
                    .find(|evidence| evidence.name == patch.name)
                    .with_context(|| format!("operation has no old commit for {}", patch.name))?
                    .commit
                    .clone();
                dropped.push(DroppedPatch {
                    patch,
                    commit: old_commit,
                });
            }
        }
        if dropped.is_empty() {
            return Ok(DroppedPatches::default());
        }
        let command = std::iter::once("delete".to_string())
            .chain(dropped.iter().map(|item| item.patch.name.clone()))
            .collect::<Vec<_>>();
        run(&self.repo, "stg", command)?;
        let names = dropped
            .iter()
            .map(|item| item.patch.name.clone())
            .collect::<Vec<_>>();
        let set = names
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        self.manifest_mut()?
            .patches
            .retain(|patch| !set.contains(patch.name.as_str()));
        let mut removed_exports = Vec::new();
        for item in &dropped {
            if item.patch.kind == crate::manifest::PatchKind::Source {
                for entry in fs::read_dir(self.repo.join(&self.manifest()?.documents.exports))? {
                    let path = entry?.path();
                    if path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(&format!("-{}.patch", item.patch.name)))
                    {
                        fs::remove_file(&path)?;
                        removed_exports.push(path);
                    }
                }
            }
        }
        Ok(DroppedPatches {
            names,
            items: dropped,
            removed_exports,
        })
    }

    fn report_path(&self, new_tip: &str) -> Result<PathBuf> {
        let short = new_tip.get(..12).context("new tip is not a full SHA")?;
        self.git_private_path(&format!("forkctl/rebases/{short}.md"))
    }

    fn render_report(
        &self,
        operation: &OperationState,
        new_base: &str,
        new_tip: &str,
        range: &str,
    ) -> Result<String> {
        let exports = self
            .manifest()?
            .source_exports()
            .into_iter()
            .map(|export| {
                Ok(ExportEvidence {
                    path: export.path.clone(),
                    hash: capture(&self.repo, "git", ["hash-object", &export.path])?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        report::render(RebaseReport {
            target: operation
                .target
                .as_ref()
                .map_or_else(|| "unknown".to_string(), |target| target.selector.clone()),
            old_base: operation.old_base.clone(),
            old_tip: operation.old_tip.clone(),
            new_base: new_base.to_string(),
            new_tip: new_tip.to_string(),
            recovery_tag: operation.recovery.tag.clone(),
            exports,
            range_diff: range.to_string(),
        })
    }
}

#[derive(Default)]
struct DroppedPatches {
    names: Vec<String>,
    items: Vec<DroppedPatch>,
    removed_exports: Vec<PathBuf>,
}
