use super::App;
use crate::process::{capture, run};
use anyhow::{Context, Result, ensure};
use std::fs;

impl App {
    pub fn verify(&self) -> Result<()> {
        self.require_clean()?;
        let upstream = &self.manifest.upstream;
        let actual_url = capture(&self.repo, "git", ["remote", "get-url", &upstream.remote])?;
        ensure!(
            actual_url == upstream.url,
            "remote {} is {}, expected {}",
            upstream.remote,
            actual_url,
            upstream.url
        );
        let push_url = capture(
            &self.repo,
            "git",
            ["remote", "get-url", "--push", &upstream.remote],
        )?;
        ensure!(
            push_url == "DISABLED",
            "remote {} push URL is {}, expected DISABLED",
            upstream.remote,
            push_url
        );

        for (label, revision) in [
            ("canonical base", self.manifest.bases.canonical.as_str()),
            ("stack base", self.manifest.bases.stack.as_str()),
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
            actual_base == self.manifest.bases.stack,
            "StGit base is {}, expected {}",
            actual_base,
            self.manifest.bases.stack
        );
        let merge_base = capture(
            &self.repo,
            "git",
            ["merge-base", &self.manifest.bases.stack, &upstream.git_ref],
        )?;
        ensure!(
            merge_base == self.manifest.bases.canonical,
            "canonical merge base is {}, expected {}",
            merge_base,
            self.manifest.bases.canonical
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
            &self.manifest.bases.canonical,
            &self.manifest.bases.stack,
            &self.manifest.allow.base,
            "pre-stack drift",
        )?;
        let source_top_name = self.manifest.source_top()?.name.as_str();
        let source_top = capture(&self.repo, "stg", ["id", source_top_name])?;
        self.verify_allowed_diff(
            &source_top,
            "HEAD",
            &self.manifest.allow.tooling,
            "tooling patch path",
        )?;

        self.verify_required_text()?;

        let expected_tree = capture(
            &self.repo,
            "git",
            ["rev-parse", &format!("{source_top}^{{tree}}")],
        )?;
        let reconstructed_tree = self.reconstruct_tree()?;
        ensure!(
            reconstructed_tree == expected_tree,
            "exported patches reconstruct {reconstructed_tree}, expected {expected_tree}"
        );

        println!(
            "forkctl: canonical={} stack-base={} patches={} source-tree={}",
            self.manifest.bases.canonical,
            self.manifest.bases.stack,
            expected_stack.len(),
            expected_tree
        );
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
}
