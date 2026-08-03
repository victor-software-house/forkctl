use super::{App, relative_to, write_atomic};
use crate::process::{capture, output, run, succeeds};
use anyhow::{Context, Result, ensure};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::PathBuf;

const EXPORT_TEMPLATE: &str = include_str!("../patchexport.tmpl");

impl App {
    pub fn rebase(&mut self) -> Result<()> {
        self.verify()?;
        self.fetch_upstream(false)?;
        run(
            &self.repo,
            "stg",
            ["rebase", "--merged", &self.manifest.upstream.git_ref],
        )?;

        let new_base = capture(&self.repo, "stg", ["id", "{base}"])?;
        let expected_top = &self
            .manifest
            .patches
            .last()
            .expect("validated patches")
            .name;
        let actual_top = capture(&self.repo, "stg", ["top"])?;
        ensure!(
            actual_top == *expected_top,
            "top patch is {actual_top}, expected {expected_top}"
        );

        let export_dir = tempfile::tempdir().context("create patch export directory")?;
        let template_path = export_dir.path().join("patchexport.tmpl");
        fs::write(&template_path, EXPORT_TEMPLATE)
            .with_context(|| format!("write {}", template_path.display()))?;
        let mut exported = Vec::new();
        for patch in self.manifest.exported_patches() {
            let relative = patch.export.as_ref().expect("exported patch");
            let patch_output = output(
                &self.repo,
                "stg",
                [
                    OsStr::new("export"),
                    OsStr::new("--stdout"),
                    OsStr::new("--template"),
                    template_path.as_os_str(),
                    OsStr::new(&patch.name),
                ],
            )?;
            let target = self.repo.join(relative);
            write_atomic(&target, &patch_output.stdout)?;
            exported.push(target);
        }

        self.manifest.bases.canonical.clone_from(&new_base);
        self.manifest.bases.stack.clone_from(&new_base);
        self.write_manifest()?;

        let paths = std::iter::once(self.manifest_path.as_path())
            .chain(exported.iter().map(PathBuf::as_path))
            .map(|path| relative_to(&self.repo, path))
            .collect::<Result<Vec<_>>>()?;
        let mut add_args = vec![OsString::from("add"), OsString::from("--")];
        add_args.extend(paths.iter().cloned().map(OsString::from));
        run(&self.repo, "git", add_args)?;

        let diff_args = vec![
            OsString::from("diff"),
            OsString::from("--cached"),
            OsString::from("--quiet"),
            OsString::from("--"),
        ]
        .into_iter()
        .chain(paths.into_iter().map(OsString::from))
        .collect::<Vec<_>>();
        if !succeeds(&self.repo, "git", diff_args)? {
            run(&self.repo, "stg", ["refresh", "--index"])?;
        }
        self.verify()?;
        println!("forkctl: rebased and verified at {new_base}");
        Ok(())
    }
}
