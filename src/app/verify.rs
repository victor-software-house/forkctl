use super::App;
use crate::ledger;
use crate::process::{capture, run};
use anyhow::{Context, Result, ensure};
use std::fs;

pub struct Verification {
    pub patches: usize,
    pub source_tree: String,
}

impl App {
    pub fn verify(&self) -> Result<()> {
        let result = self.verify_quiet()?;
        println!(
            "forkctl: canonical={} stack-base={} patches={} source-tree={}",
            self.manifest.base.canonical,
            self.manifest.base.stack,
            result.patches,
            result.source_tree
        );
        Ok(())
    }

    pub(super) fn verify_quiet(&self) -> Result<Verification> {
        self.require_clean()?;
        self.require_declared_branch()?;
        self.verify_remotes()?;

        for (label, revision) in [
            ("canonical base", self.manifest.base.canonical.as_str()),
            ("stack base", self.manifest.base.stack.as_str()),
        ] {
            run(
                &self.repo,
                "git",
                ["cat-file", "-e", &format!("{revision}^{{commit}}")],
            )
            .with_context(|| format!("{label} commit is unavailable: {revision}"))?;
        }
        let actual_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        ensure!(
            actual_base == self.manifest.base.stack,
            "StGit base is {actual_base}, expected {}",
            self.manifest.base.stack
        );
        let merge_base = capture(
            &self.repo,
            "git",
            [
                "merge-base",
                self.manifest.base.stack.as_str(),
                self.upstream_tracking_ref().as_str(),
            ],
        )?;
        ensure!(
            merge_base == self.manifest.base.canonical,
            "canonical merge base is {merge_base}, expected {}",
            self.manifest.base.canonical
        );

        let actual_stack = self.stg_series()?;
        let expected_stack = self.manifest.patch_names();
        ensure!(
            actual_stack == expected_stack,
            "StGit patch order differs: got {}, expected {}",
            actual_stack.join(", "),
            expected_stack.join(", ")
        );
        ensure!(
            capture(&self.repo, "stg", ["series", "--unapplied", "--count"])? == "0",
            "all fork patches must be applied"
        );

        self.verify_allowed_diff(
            &self.manifest.base.canonical,
            &self.manifest.base.stack,
            &self.manifest.allow.base,
            "pre-stack drift",
        )?;
        for patch in &self.manifest.patches {
            let commit = self.patch_commit(&patch.name)?;
            let paths = self.patch_paths(&commit)?;
            ensure!(!paths.is_empty(), "patch {} is empty", patch.name);
            Self::verify_allowed_paths(
                &paths,
                &patch.paths,
                &format!("path in patch {}", patch.name),
            )?;
            self.verify_patch_trailers(&patch.name, &commit)?;
        }

        self.verify_required_text()?;
        self.verify_ledger()?;
        self.verify_exports()?;

        let expected_tree = self.expected_reconstructed_tree()?;
        let reconstructed_tree = self.reconstruct_tree()?;
        ensure!(
            reconstructed_tree == expected_tree,
            "exported patches reconstruct {reconstructed_tree}, expected {expected_tree}"
        );
        self.verify_pending()?;

        Ok(Verification {
            patches: expected_stack.len(),
            source_tree: expected_tree,
        })
    }

    fn verify_remotes(&self) -> Result<()> {
        let downstream = &self.manifest.downstream;
        capture(&self.repo, "git", ["remote", "get-url", &downstream.remote])
            .with_context(|| format!("downstream remote {} is unavailable", downstream.remote))?;

        let upstream = &self.manifest.upstream;
        let actual_url = capture(&self.repo, "git", ["remote", "get-url", &upstream.remote])?;
        ensure!(
            actual_url == upstream.url,
            "remote {} is {actual_url}, expected {}",
            upstream.remote,
            upstream.url
        );
        let push_url = capture(
            &self.repo,
            "git",
            ["remote", "get-url", "--push", &upstream.remote],
        )?;
        ensure!(
            push_url == "DISABLED",
            "remote {} push URL is {push_url}, expected DISABLED",
            upstream.remote
        );
        run(
            &self.repo,
            "git",
            [
                "cat-file",
                "-e",
                &format!("{}^{{commit}}", self.upstream_tracking_ref()),
            ],
        )
        .context("upstream tracking commit is unavailable")
    }

    fn verify_patch_trailers(&self, patch_name: &str, commit: &str) -> Result<()> {
        let patch = self
            .manifest
            .patches
            .iter()
            .find(|candidate| candidate.name == patch_name)
            .expect("patch name came from manifest");
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
            ensure!(
                actual == expected,
                "patch {patch_name} trailer {key} is {actual:?}, expected {expected:?}"
            );
        }
        Ok(())
    }

    fn verify_required_text(&self) -> Result<()> {
        for required in &self.manifest.required {
            let path = self.repo.join(&required.path);
            let contents =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            ensure!(
                contents.contains(&required.contains),
                "required contract missing from {}: {}",
                required.path,
                required.contains
            );
        }
        Ok(())
    }

    fn verify_ledger(&self) -> Result<()> {
        let path = self.repo.join(&self.manifest.ledger);
        let actual = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let expected = ledger::render(&self.manifest);
        ensure!(
            actual == expected.as_bytes(),
            "generated ledger differs: {}",
            self.manifest.ledger
        );
        Ok(())
    }

    fn verify_exports(&self) -> Result<()> {
        for patch in self.manifest.exported_patches() {
            let relative = patch.export.as_ref().expect("exported patch");
            let path = self.repo.join(relative);
            let actual = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            let expected = self.export_patch(patch)?;
            ensure!(
                actual == expected,
                "export differs for patch {}: {relative}",
                patch.name
            );
        }
        Ok(())
    }

    fn verify_pending(&self) -> Result<()> {
        let Some(state) = self.read_pending()? else {
            return Ok(());
        };
        run(
            &self.repo,
            "git",
            ["cat-file", "-e", &format!("{}^{{tag}}", state.backup_tag)],
        )
        .with_context(|| format!("backup tag is unavailable: {}", state.backup_tag))?;
        if let Some(new_tip) = state.new_tip {
            let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
            ensure!(
                head == new_tip,
                "pending operation records tip {new_tip}, current tip is {head}"
            );
        }
        Ok(())
    }
}
