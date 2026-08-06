use crate::error::{AppResult, DomainError, InternalResultExt as _};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::OnceLock;

static GIT_LOCAL_ENV: OnceLock<Vec<OsString>> = OnceLock::new();

pub fn command(dir: &Path, program: &str) -> Command {
    let mut command = Command::new(program);
    command.current_dir(dir);
    for key in git_local_env_vars() {
        command.env_remove(key);
    }
    command
}

pub fn capture<I, S>(dir: &Path, program: &str, args: I) -> AppResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = output(dir, program, args)?;
    Ok(String::from_utf8(output.stdout)
        .internal(format!("decode {program} output as UTF-8"))?
        .trim()
        .to_string())
}

pub fn output<I, S>(dir: &Path, program: &str, args: I) -> AppResult<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = owned_args(args);
    let output = command(dir, program)
        .args(&args)
        .output()
        .internal(format!("run {program}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(DomainError::subprocess(program, &args, dir, &output).into())
    }
}

pub fn succeeds<I, S>(dir: &Path, program: &str, args: I) -> AppResult<bool>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(command(dir, program)
        .args(args)
        .output()
        .internal(format!("run {program}"))?
        .status
        .success())
}

pub fn run<I, S>(dir: &Path, program: &str, args: I) -> AppResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    output(dir, program, args).map(|_| ())
}

fn git_local_env_vars() -> &'static [OsString] {
    GIT_LOCAL_ENV.get_or_init(|| {
        Command::new("git")
            .args(["rev-parse", "--local-env-vars"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map_or_else(
                || {
                    [
                        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                        "GIT_CONFIG",
                        "GIT_CONFIG_PARAMETERS",
                        "GIT_CONFIG_COUNT",
                        "GIT_OBJECT_DIRECTORY",
                        "GIT_DIR",
                        "GIT_WORK_TREE",
                        "GIT_IMPLICIT_WORK_TREE",
                        "GIT_GRAFT_FILE",
                        "GIT_INDEX_FILE",
                        "GIT_NO_REPLACE_OBJECTS",
                        "GIT_REPLACE_REF_BASE",
                        "GIT_PREFIX",
                        "GIT_SHALLOW_FILE",
                        "GIT_COMMON_DIR",
                    ]
                    .into_iter()
                    .map(OsString::from)
                    .collect()
                },
                |output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter(|line| !line.is_empty())
                        .map(OsString::from)
                        .collect()
                },
            )
    })
}

fn owned_args<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .collect()
}
