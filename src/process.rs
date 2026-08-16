use crate::error::DomainError;
use anyhow::{Context, Result};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

static GIT_LOCAL_ENV: OnceLock<Vec<OsString>> = OnceLock::new();
static STREAM_OPERATOR: AtomicBool = AtomicBool::new(false);

pub fn set_stream_operator_output(enabled: bool) {
    STREAM_OPERATOR.store(enabled, Ordering::Relaxed);
}

pub fn command(dir: &Path, program: &str) -> Command {
    let mut command = Command::new(program);
    command.current_dir(dir);
    for key in git_local_env_vars() {
        command.env_remove(key);
    }
    command
}

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
    let output = command(dir, program)
        .args(&args)
        .output()
        .with_context(|| format!("run {program}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(DomainError::subprocess(program, &args, dir, &output).into())
    }
}

pub fn succeeds<I, S>(dir: &Path, program: &str, args: I) -> Result<bool>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(command(dir, program)
        .args(args)
        .output()
        .with_context(|| format!("run {program}"))?
        .status
        .success())
}

pub fn run<I, S>(dir: &Path, program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    output(dir, program, args).map(|_| ())
}

pub fn run_operator<I, S>(dir: &Path, program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    if STREAM_OPERATOR.load(Ordering::Relaxed) {
        streamed_output(dir, program, args).map(|_| ())
    } else {
        run(dir, program, args)
    }
}

fn streamed_output<I, S>(dir: &Path, program: &str, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = owned_args(args);
    let mut child = command(dir, program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("run {program}"))?;
    let stdout_pipe = child
        .stdout
        .take()
        .with_context(|| format!("{program} stdout"))?;
    let stderr_pipe = child
        .stderr
        .take()
        .with_context(|| format!("{program} stderr"))?;
    let stdout_handle = thread::spawn(move || tee_stderr(stdout_pipe));
    let stderr_handle = thread::spawn(move || tee_stderr(stderr_pipe));
    let status = child
        .wait()
        .with_context(|| format!("wait for {program}"))?;
    let stdout = join_collected(stdout_handle);
    let stderr = join_collected(stderr_handle);
    let output = Output {
        status,
        stdout,
        stderr,
    };
    if output.status.success() {
        Ok(output)
    } else {
        Err(DomainError::subprocess(program, &args, dir, &output).into())
    }
}

fn tee_stderr(mut reader: impl Read) -> Vec<u8> {
    let mut collected = Vec::new();
    let mut buf = [0_u8; 8192];
    let mut sink = io::stderr();
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                collected.extend_from_slice(&buf[..n]);
                let _ = sink.write_all(&buf[..n]);
                let _ = sink.flush();
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    collected
}

fn join_collected(handle: thread::JoinHandle<Vec<u8>>) -> Vec<u8> {
    handle.join().unwrap_or_else(|_| Vec::new())
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

#[cfg(test)]
mod tests {
    use super::{run, run_operator, set_stream_operator_output};
    use std::env;
    use std::sync::Mutex;

    static STREAM_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn run_operator_buffers_when_streaming_is_off() {
        let _guard = STREAM_LOCK.lock().unwrap();
        set_stream_operator_output(false);
        let err = run_operator(
            &env::temp_dir(),
            "sh",
            ["-c", "echo hidden-stdout; echo hidden-stderr >&2; exit 1"],
        )
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("hidden-stderr"), "{message}");
    }

    #[test]
    fn run_still_captures_without_the_operator_path() {
        let err = run(
            &env::temp_dir(),
            "sh",
            ["-c", "echo captured-stderr >&2; exit 1"],
        )
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("captured-stderr"), "{message}");
    }
}
