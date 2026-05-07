use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use wait_timeout::ChildExt;

#[derive(Debug, Clone)]
pub struct GitCommand {
    program: PathBuf,
    default_timeout: Duration,
}

impl GitCommand {
    pub fn new(program: impl Into<PathBuf>, default_timeout: Duration) -> Self {
        Self {
            program: program.into(),
            default_timeout,
        }
    }

    pub fn run(&self, request: GitCommandRequest) -> Result<GitCommandOutput, GitProcessError> {
        let timeout = request.timeout.unwrap_or(self.default_timeout);
        let mut command = Command::new(&self.program);
        command
            .args(&request.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if request.stdin.is_some() {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }

        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }

        for (key, value) in &request.env {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|source| GitProcessError::Spawn {
            program: self.program.clone(),
            args: request.args.clone(),
            source,
        })?;

        let stdin = match (request.stdin, child.stdin.take()) {
            (Some(input), Some(mut pipe)) => Some(thread::spawn(move || pipe.write_all(&input))),
            _ => None,
        };
        let stdout = child.stdout.take().map(read_pipe);
        let stderr = child.stderr.take().map(read_pipe);

        let status = match child
            .wait_timeout(timeout)
            .map_err(|source| GitProcessError::Wait {
                program: self.program.clone(),
                args: request.args.clone(),
                source,
            })? {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = join_writer(stdin);
                let stdout = join_reader(stdout);
                let stderr = join_reader(stderr);
                return Err(GitProcessError::TimedOut {
                    program: self.program.clone(),
                    args: request.args,
                    timeout,
                    stdout,
                    stderr,
                });
            }
        };

        let stdin_error = join_writer(stdin);
        let stdout = join_reader(stdout);
        let stderr = join_reader(stderr);
        let status = GitCommandStatus::from(status);

        if !status.success {
            return Err(GitProcessError::Failed {
                program: self.program.clone(),
                args: request.args,
                status,
                stdout,
                stderr,
            });
        }
        if let Some(error) = stdin_error {
            return Err(GitProcessError::PipeWrite(error));
        }

        Ok(GitCommandOutput {
            status,
            stdout,
            stderr,
        })
    }
}

impl Default for GitCommand {
    fn default() -> Self {
        Self::new("git", Duration::from_secs(15))
    }
}

#[derive(Debug, Clone)]
pub struct GitCommandRequest {
    args: Vec<String>,
    cwd: Option<PathBuf>,
    env: Vec<(String, String)>,
    timeout: Option<Duration>,
    stdin: Option<Vec<u8>>,
}

impl GitCommandRequest {
    pub fn new<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            args: args.into_iter().map(Into::into).collect(),
            cwd: None,
            env: Vec::new(),
            timeout: None,
            stdin: None,
        }
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn stdin(mut self, input: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(input.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandStatus {
    pub success: bool,
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

impl From<std::process::ExitStatus> for GitCommandStatus {
    fn from(status: std::process::ExitStatus) -> Self {
        #[cfg(unix)]
        let signal = {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        };

        #[cfg(not(unix))]
        let signal = None;

        Self {
            success: status.success(),
            code: status.code(),
            signal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitCommandOutput {
    pub status: GitCommandStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl GitCommandOutput {
    pub fn stdout_string(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.stdout.clone())
    }

    pub fn stderr_lossy(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GitProcessError {
    #[error("failed to spawn git command {program:?} {args:?}: {source}")]
    Spawn {
        program: PathBuf,
        args: Vec<String>,
        source: std::io::Error,
    },
    #[error("failed while waiting for git command {program:?} {args:?}: {source}")]
    Wait {
        program: PathBuf,
        args: Vec<String>,
        source: std::io::Error,
    },
    #[error("git command timed out after {timeout:?}: {program:?} {args:?}")]
    TimedOut {
        program: PathBuf,
        args: Vec<String>,
        timeout: Duration,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    #[error("git command failed with status {status:?}: {program:?} {args:?}")]
    Failed {
        program: PathBuf,
        args: Vec<String>,
        status: GitCommandStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    #[error("failed to read git command pipe: {0}")]
    PipeRead(String),
    #[error("failed to write git command stdin: {0}")]
    PipeWrite(String),
}

impl GitProcessError {
    pub fn stderr_lossy(&self) -> String {
        match self {
            Self::TimedOut { stderr, .. } | Self::Failed { stderr, .. } => {
                String::from_utf8_lossy(stderr).into_owned()
            }
            _ => String::new(),
        }
    }
}

fn read_pipe<R>(mut pipe: R) -> thread::JoinHandle<Result<Vec<u8>, std::io::Error>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = Vec::new();
        pipe.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn join_reader(reader: Option<thread::JoinHandle<Result<Vec<u8>, std::io::Error>>>) -> Vec<u8> {
    match reader {
        Some(handle) => match handle.join() {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => format!("pipe read failed: {error}").into_bytes(),
            Err(_) => b"pipe read thread panicked".to_vec(),
        },
        None => Vec::new(),
    }
}

fn join_writer(writer: Option<thread::JoinHandle<Result<(), std::io::Error>>>) -> Option<String> {
    writer.and_then(|handle| match handle.join() {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(format!("pipe write failed: {error}")),
        Err(_) => Some("pipe write thread panicked".to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_stdout_stderr_and_status() {
        let git = GitCommand::default();
        let output = git
            .run(GitCommandRequest::new(["--version"]).timeout(Duration::from_secs(5)))
            .unwrap();

        assert!(output.status.success);
        assert!(output.stdout_string().unwrap().starts_with("git version"));
    }

    #[test]
    fn captures_nonzero_exit() {
        let git = GitCommand::default();
        let error = git
            .run(GitCommandRequest::new(["not-a-real-command"]).timeout(Duration::from_secs(5)))
            .unwrap_err();

        match error {
            GitProcessError::Failed { status, stderr, .. } => {
                assert!(!status.success);
                assert!(!stderr.is_empty());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
