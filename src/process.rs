use anyhow::{Context, Result, bail, ensure};
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

pub fn capture<I, S>(dir: &Path, program: &str, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = output(dir, program, args)?;
    Ok(String::from_utf8(output.stdout)
        .with_context(|| format!("{program} output is not UTF-8"))?
        .trim()
        .to_string())
}

pub fn output<I, S>(dir: &Path, program: &str, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = owned_args(args);
    let output = Command::new(program)
        .args(&args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("run {program}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "{} {}{}",
            program,
            display_args(&args),
            if stderr.is_empty() {
                format!(" failed with {}", output.status)
            } else {
                format!(": {stderr}")
            }
        )
    }
}

pub fn run<I, S>(dir: &Path, program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = owned_args(args);
    let status = Command::new(program)
        .args(&args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("run {program}"))?;
    ensure!(
        status.success(),
        "{} {} failed with {status}",
        program,
        display_args(&args)
    );
    Ok(())
}

fn owned_args<I, S>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .collect()
}

fn display_args(args: &[std::ffi::OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}
