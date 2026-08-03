use super::App;
use crate::manifest::Patch;
use crate::process::{capture, run};
use crate::protocol::{NewPatchRequest, NewPhase, NewResult};
use crate::state::PendingOperation;
use anyhow::{Context, Result, ensure};
use std::ffi::OsString;

impl App {
    pub fn new_patch(&mut self, args: NewPatchRequest) -> Result<NewResult> {
        self.verify()?;
        let patch = Patch {
            name: args.name,
            kind: args.kind,
            purpose: args.purpose,
            upstream_status: args.upstream_status,
            drop_when: args.drop_when,
            paths: args.paths,
            export: args.export,
        };
        ensure!(
            !self
                .manifest
                .patches
                .iter()
                .any(|candidate| candidate.name == patch.name),
            "patch already exists: {}",
            patch.name
        );
        let insertion = self.manifest.insertion_index(patch.kind);
        let mut next_manifest = self.manifest.clone();
        next_manifest.patches.insert(insertion, patch.clone());
        next_manifest.validate(&self.repo, &self.manifest_path)?;

        let mut pending = self.create_recovery(PendingOperation::New)?;
        self.write_pending(&pending)?;
        if insertion == 0 {
            run(&self.repo, "stg", ["pop", "--all"])?;
        } else {
            run(
                &self.repo,
                "stg",
                ["goto", self.manifest.patches[insertion - 1].name.as_str()],
            )?;
        }
        run(
            &self.repo,
            "stg",
            [
                OsString::from("new"),
                OsString::from("--message"),
                OsString::from(patch.message()),
                OsString::from(&patch.name),
            ],
        )?;
        run(&self.repo, "stg", ["push", "--all"])?;

        self.manifest = next_manifest;
        self.write_manifest()?;
        let ledger = self.write_ledger()?;
        self.stage_and_refresh_bookkeeping(&[self.manifest_path.clone(), ledger])?;

        pending.new_base = Some(capture(&self.repo, "stg", ["id", "{base}"])?);
        self.write_pending(&pending)?;
        Ok(NewResult {
            phase: NewPhase::Created,
            patch: Some(patch.name),
            kind: Some(patch.kind),
            allowed_paths: patch.paths,
            verification: None,
        })
    }

    pub fn finish_new(&mut self) -> Result<NewResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        let mut pending = self
            .read_pending()?
            .context("no forkctl new operation is pending")?;
        ensure!(
            pending.operation == PendingOperation::New,
            "pending operation is not new"
        );
        ensure!(
            capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? == "0",
            "apply all patches before finishing new"
        );
        let top = capture(&self.repo, "stg", ["top"])?;
        ensure!(
            top == self.manifest.bookkeeping_patch,
            "top patch is {top}, expected {}",
            self.manifest.bookkeeping_patch
        );
        let exports = self.write_exports()?;
        let ledger = self.write_ledger()?;
        let paths = std::iter::once(self.manifest_path.clone())
            .chain(std::iter::once(ledger))
            .chain(exports)
            .collect::<Vec<_>>();
        self.stage_and_refresh_bookkeeping(&paths)?;
        pending.new_base = Some(capture(&self.repo, "stg", ["id", "{base}"])?);
        pending.new_tip = Some(capture(&self.repo, "git", ["rev-parse", "HEAD"])?);
        self.write_pending(&pending)?;
        let verification = self.verify()?;
        Ok(NewResult {
            phase: NewPhase::Finished,
            patch: None,
            kind: None,
            allowed_paths: Vec::new(),
            verification: Some(verification),
        })
    }
}
