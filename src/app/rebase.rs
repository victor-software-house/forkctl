use super::{App, write_atomic};
use crate::process::{capture, run};
use crate::report::{self, ExportEvidence, RebaseReport};
use crate::state::{PendingOperation, PendingState};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;

impl App {
    pub fn rebase(&mut self, target: &str) -> Result<()> {
        self.require_clean()?;
        self.require_declared_branch()?;
        if let Some(pending) = self.read_pending()? {
            ensure!(
                pending.operation == PendingOperation::Rebase,
                "a {:?} operation is already pending",
                pending.operation
            );
            ensure!(
                pending.target_label.as_deref() == Some(target),
                "pending rebase targets {}, not {target}",
                pending.target_label.as_deref().unwrap_or("unknown")
            );
            return self.finish_rebase(pending);
        }

        self.verify()?;
        self.fetch_upstream(false)?;
        let target_sha = self.resolve_target(target)?;
        let mut pending = self.create_recovery(PendingOperation::Rebase)?;
        pending.target_label = Some(target.to_string());
        pending.target_sha = Some(target_sha.clone());
        self.write_pending(&pending)?;

        if let Err(error) = run(&self.repo, "stg", ["rebase", "--merged", &target_sha]) {
            eprintln!(
                "forkctl: rebase stopped; recovery tag: {}",
                pending.backup_tag
            );
            eprintln!(
                "forkctl: resolve with stg add --update, stg refresh, and stg goto {}; then rerun forkctl rebase --onto {target}",
                self.manifest.bookkeeping_patch
            );
            return Err(error).context("StGit rebase stopped");
        }
        self.finish_rebase(pending)
    }

    fn finish_rebase(&mut self, mut pending: PendingState) -> Result<()> {
        let target_sha = pending
            .target_sha
            .as_deref()
            .context("pending rebase has no target SHA")?;
        ensure!(
            capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? == "0",
            "rebase is incomplete; apply all patches before resuming"
        );
        let new_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        ensure!(
            new_base == target_sha,
            "StGit base is {new_base}, expected {target_sha}"
        );
        let actual_top = capture(&self.repo, "stg", ["top"])?;
        ensure!(
            actual_top == self.manifest.bookkeeping_patch,
            "top patch is {actual_top}, expected {}",
            self.manifest.bookkeeping_patch
        );

        self.manifest.base.label = pending
            .target_label
            .clone()
            .context("pending rebase has no target label")?;
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
        pending.report = Some(report_path.display().to_string());
        self.write_pending(&pending)?;
        println!("forkctl: rebased and verified at {new_base}");
        println!("forkctl: recovery tag: {}", pending.backup_tag);
        println!("forkctl: review report: {}", report_path.display());
        println!("forkctl: run consumer semantic checks before forkctl publish");
        Ok(())
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
                .target_label
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
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
