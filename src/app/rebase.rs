use super::{App, write_atomic};
use crate::manifest::{BaseTarget, PatchEvent, PatchEventKind};
use crate::process::{capture, run};
use crate::protocol::RebaseResult;
use crate::report::{self, ExportEvidence, RebaseReport};
use crate::state::{PendingOperation, PendingState, ReportEvidence};
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::PathBuf;

impl App {
    pub fn rebase(&mut self, selector: &str) -> Result<RebaseResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(pending) = self.read_pending()? {
            ensure!(
                pending.operation == PendingOperation::Rebase,
                "a {:?} operation is already pending",
                pending.operation
            );
            ensure!(
                pending
                    .target
                    .as_ref()
                    .map(|target| target.selector.as_str())
                    == Some(selector),
                "pending rebase targets {}, not {selector}",
                pending
                    .target
                    .as_ref()
                    .map_or("unknown", |target| target.selector.as_str())
            );
            return self.finish_rebase(pending);
        }

        self.verify()?;
        self.fetch_upstream(false)?;
        let target = self.resolve_target(selector)?;
        let mut pending = self.create_recovery(PendingOperation::Rebase)?;
        pending.target = Some(target.clone());
        self.write_pending(&pending)?;

        if let Err(error) = run(
            &self.repo,
            "stg",
            ["rebase", "--merged", target.commit.as_str()],
        ) {
            return Err(error).context(format!(
                "StGit rebase stopped; recovery tag {}; resolve with stg add --update, stg refresh, and stg goto {}, then rerun forkctl rebase --onto {selector}",
                pending.backup_tag, self.manifest.bookkeeping_patch
            ));
        }
        self.finish_rebase(pending)
    }

    fn finish_rebase(&mut self, mut pending: PendingState) -> Result<RebaseResult> {
        let target = pending
            .target
            .clone()
            .context("pending rebase has no target")?;
        ensure!(
            capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? == "0",
            "rebase is incomplete; apply all patches before resuming"
        );
        let new_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        ensure!(
            new_base == target.commit,
            "StGit base is {new_base}, expected {}",
            target.commit
        );
        let actual_top = capture(&self.repo, "stg", ["top"])?;
        ensure!(
            actual_top == self.manifest.bookkeeping_patch,
            "top patch is {actual_top}, expected {}",
            self.manifest.bookkeeping_patch
        );

        let dropped = self.drop_upstream_merged(&target, &pending)?;
        self.manifest.base.target = target;
        self.manifest.base.stack.clone_from(&new_base);
        self.manifest.base.canonical = capture(
            &self.repo,
            "git",
            [
                "merge-base",
                &new_base,
                self.upstream_tracking_ref().as_str(),
            ],
        )?;
        let exports = self.write_exports()?;
        self.write_manifest()?;
        let ledger = self.write_ledger()?;
        let paths = std::iter::once(self.manifest_path.clone())
            .chain(std::iter::once(ledger))
            .chain(exports)
            .chain(dropped.removed_exports)
            .collect::<Vec<PathBuf>>();
        self.stage_and_refresh_bookkeeping(&paths)?;

        let new_tip = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        pending.new_base = Some(new_base.clone());
        pending.new_tip = Some(new_tip.clone());
        self.write_pending(&pending)?;
        self.verify()?;

        let range = capture(
            &self.repo,
            "git",
            [
                "range-diff",
                "--no-color",
                &format!("{}..{}", pending.old_base, pending.old_tip),
                &format!("{new_base}..{new_tip}"),
            ],
        )?;
        let report_path = self.report_path(&new_tip)?;
        let report = self.render_report(&pending, &new_base, &new_tip, &range)?;
        write_atomic(&report_path, report.as_bytes())?;
        pending.report = Some(ReportEvidence {
            path: report_path.display().to_string(),
            object_id: self.file_object_id(&report_path)?,
        });
        self.write_pending(&pending)?;
        self.verify_pending_state(&pending)?;
        let report = pending.report.as_ref().expect("report was just recorded");
        Ok(RebaseResult {
            selected_target: self.manifest.base.target.selector.clone(),
            new_base,
            new_tip,
            recovery_tag: pending.backup_tag,
            report_path: report.path.clone(),
            report_object_id: report.object_id.clone(),
            dropped_patches: dropped.names,
        })
    }

    fn drop_upstream_merged(
        &mut self,
        target: &BaseTarget,
        pending: &PendingState,
    ) -> Result<DroppedPatches> {
        let mut dropped = Vec::new();
        for patch in &self.manifest.patches {
            if patch.name == self.manifest.bookkeeping_patch {
                continue;
            }
            let commit = self.patch_commit(&patch.name)?;
            if self.patch_paths(&commit)?.is_empty() {
                let old_commit = pending
                    .old_patches
                    .iter()
                    .find(|evidence| evidence.name == patch.name)
                    .with_context(|| {
                        format!(
                            "pending recovery has no old commit for patch {}",
                            patch.name
                        )
                    })?
                    .commit
                    .clone();
                dropped.push((patch.clone(), old_commit));
            }
        }
        if dropped.is_empty() {
            return Ok(DroppedPatches::default());
        }

        let arguments = std::iter::once("delete".to_string())
            .chain(dropped.iter().map(|(patch, _)| patch.name.clone()))
            .collect::<Vec<_>>();
        run(&self.repo, "stg", arguments)?;
        let dropped_names = dropped
            .iter()
            .map(|(patch, _)| patch.name.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.manifest
            .patches
            .retain(|patch| !dropped_names.contains(patch.name.as_str()));

        let mut removed_exports = Vec::new();
        let mut names = Vec::new();
        for (patch, commit) in dropped {
            if let Some(export) = &patch.export {
                let path = self.repo.join(export);
                match fs::remove_file(&path) {
                    Ok(()) => removed_exports.push(path),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error).with_context(|| format!("remove {export}")),
                }
            }
            names.push(patch.name.clone());
            self.manifest.history.push(PatchEvent {
                kind: PatchEventKind::UpstreamMerged,
                patch,
                commit,
                target: target.clone(),
            });
        }
        Ok(DroppedPatches {
            names,
            removed_exports,
        })
    }

    fn report_path(&self, new_tip: &str) -> Result<PathBuf> {
        let short = new_tip.get(..12).context("new tip is not a full SHA")?;
        self.git_private_path(&format!("forkctl/rebases/{short}.md"))
    }

    fn render_report(
        &self,
        pending: &PendingState,
        new_base: &str,
        new_tip: &str,
        range: &str,
    ) -> Result<String> {
        let exports = self
            .manifest
            .exported_patches()
            .map(|patch| {
                let path = patch.export.as_ref().expect("exported patch");
                Ok(ExportEvidence {
                    path: path.clone(),
                    hash: capture(&self.repo, "git", ["hash-object", path])?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        report::render(RebaseReport {
            target: pending
                .target
                .as_ref()
                .map_or_else(|| "unknown".to_string(), |target| target.selector.clone()),
            old_base: pending.old_base.clone(),
            old_tip: pending.old_tip.clone(),
            new_base: new_base.to_string(),
            new_tip: new_tip.to_string(),
            recovery_tag: pending.backup_tag.clone(),
            exports,
            range_diff: range.to_string(),
        })
    }
}

#[derive(Default)]
struct DroppedPatches {
    names: Vec<String>,
    removed_exports: Vec<PathBuf>,
}
