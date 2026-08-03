use super::App;
use crate::process::{capture, run};
use crate::state::PendingOperation;
use anyhow::{Context, Result, ensure};

impl App {
    pub fn publish(&self) -> Result<()> {
        self.verify()?;
        let pending = self
            .read_pending()?
            .context("no forkctl operation is pending publication")?;
        let head = capture(&self.repo, "git", ["rev-parse", "HEAD"])?;
        if pending.operation == PendingOperation::Rebase {
            ensure!(
                pending.new_tip.as_deref() == Some(head.as_str()),
                "pending rebase does not describe current HEAD"
            );
            let report = pending
                .report
                .as_deref()
                .context("pending rebase has no review report")?;
            ensure!(
                std::path::Path::new(report).is_file(),
                "rebase report is unavailable: {report}"
            );
        }

        let downstream_ref = self.downstream_ref();
        let actual_remote = self.downstream_sha()?;
        ensure!(
            actual_remote == pending.expected_remote_sha,
            "downstream advanced to {actual_remote}, expected {}",
            pending.expected_remote_sha
        );
        let tag_ref = format!("refs/tags/{}", pending.backup_tag);
        if let Ok(remote_tag) = self.remote_ref_sha(&self.manifest.downstream.remote, &tag_ref) {
            let local_tag = capture(&self.repo, "git", ["rev-parse", &tag_ref])?;
            ensure!(
                remote_tag == local_tag,
                "remote backup tag {} points to another object",
                pending.backup_tag
            );
        }

        let lease = format!(
            "--force-with-lease={downstream_ref}:{}",
            pending.expected_remote_sha
        );
        let tag_refspec = format!("{tag_ref}:{tag_ref}");
        let branch_refspec = format!("HEAD:{downstream_ref}");
        run(
            &self.repo,
            "git",
            [
                "push",
                "--atomic",
                lease.as_str(),
                self.manifest.downstream.remote.as_str(),
                tag_refspec.as_str(),
                branch_refspec.as_str(),
            ],
        )?;

        let remote_branch = self.downstream_sha()?;
        ensure!(
            remote_branch == head,
            "published branch is {remote_branch}, expected {head}"
        );
        let remote_tag = self.remote_ref_sha(&self.manifest.downstream.remote, &tag_ref)?;
        let local_tag = capture(&self.repo, "git", ["rev-parse", &tag_ref])?;
        ensure!(remote_tag == local_tag, "published backup tag differs");
        self.clear_pending()?;
        println!(
            "forkctl: published {} and {} at {head}",
            pending.backup_tag, self.manifest.downstream.branch
        );
        Ok(())
    }
}
