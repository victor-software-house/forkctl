use super::App;
use crate::process::{capture, run};
use anyhow::{Result, ensure};

impl App {
    pub fn init(&mut self) -> Result<()> {
        self.require_clean()?;
        let upstream = &self.manifest.upstream;
        match capture(&self.repo, "git", ["remote", "get-url", &upstream.remote]) {
            Ok(actual) => ensure!(
                actual == upstream.url,
                "remote {} is {}, expected {}",
                upstream.remote,
                actual,
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

        let actual = self.stg_series()?;
        let expected = self.manifest.patch_names();
        if actual == expected {
            println!("forkctl: stack already initialized");
            return self.verify();
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
        self.verify()
    }
}
