use super::App;
use crate::process::{capture, run};
use crate::protocol::InitResult;
use anyhow::{Result, ensure};

impl App {
    pub fn init(&mut self) -> Result<InitResult> {
        self.require_clean()?;
        self.require_declared_branch()?;
        ensure!(
            self.read_pending()?.is_none(),
            "a forkctl operation is already pending"
        );

        let upstream = &self.manifest.upstream;
        match capture(&self.repo, "git", ["remote", "get-url", &upstream.remote]) {
            Ok(actual) => ensure!(
                actual == upstream.url,
                "remote {} is {actual}, expected {}",
                upstream.remote,
                upstream.url
            ),
            Err(_) => run(
                &self.repo,
                "git",
                ["remote", "add", &upstream.remote, &upstream.url],
            )?,
        }
        run(
            &self.repo,
            "git",
            ["remote", "set-url", "--push", &upstream.remote, "DISABLED"],
        )?;
        self.fetch_upstream(true)?;
        self.fetch_base_target(true)?;
        self.fetch_recovery_tags(true)?;

        let actual = self.stg_series()?;
        let expected = self.manifest.patch_names();
        if actual == expected {
            return Ok(InitResult {
                already_initialized: true,
                verification: self.verify()?,
            });
        }
        ensure!(
            actual.is_empty(),
            "existing StGit stack differs: got {}, expected {}",
            actual.join(", "),
            expected.join(", ")
        );

        run(&self.repo, "stg", ["init"])?;
        let args = std::iter::once("uncommit".to_string())
            .chain(expected.iter().rev().cloned())
            .collect::<Vec<_>>();
        run(&self.repo, "stg", args)?;
        Ok(InitResult {
            already_initialized: false,
            verification: self.verify()?,
        })
    }
}
