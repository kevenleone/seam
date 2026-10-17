use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::RwLock;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::OnceCell;

/// A failed git command, with git's own message.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct GitError {
    pub message: String,
}

impl GitError {
    pub fn new(message: impl Into<String>) -> Self {
        GitError {
            message: message.into(),
        }
    }

    fn from_stderr(args: &[String], code: i32, stderr: &str) -> Self {
        let message = stderr
            .lines()
            .filter(|l| !l.starts_with("hint:"))
            .map(|l| {
                l.trim_start_matches("error: ")
                    .trim_start_matches("fatal: ")
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        if message.is_empty() {
            GitError::new(format!(
                "git {} failed (exit {code})",
                args.first().map(String::as_str).unwrap_or("")
            ))
        } else {
            GitError::new(message)
        }
    }
}

impl serde::Serialize for GitError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.message)
    }
}

impl From<std::io::Error> for GitError {
    fn from(e: std::io::Error) -> Self {
        GitError::new(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GitError>;

pub struct Output {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl Output {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

#[derive(Default)]
pub struct RunOptions<'a> {
    pub stdin: Option<&'a str>,
    /// Background reads skip optional locks so they never block a git command in Terminal.
    pub read: bool,
    pub env: Vec<(&'a str, String)>,
    pub allow_failure: bool,
    /// Called with each progress line git writes to stderr (split on \r and \n).
    pub progress: Option<&'a (dyn Fn(String) + Send + Sync)>,
}

static GIT_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Override the git executable; None goes back to the default.
pub fn set_git_executable(path: Option<PathBuf>) {
    *GIT_PATH.write().unwrap() = path;
}

pub fn git_executable() -> PathBuf {
    GIT_PATH
        .read()
        .unwrap()
        .clone()
        .unwrap_or_else(|| PathBuf::from("/usr/bin/git"))
}

static SHELL_ENV: OnceCell<HashMap<String, String>> = OnceCell::const_new();

/// Apps launched from Finder get launchd's minimal environment, so hooks that need node,
/// python or a version manager fail. Ask the login shell for its environment once.
async fn base_env() -> &'static HashMap<String, String> {
    SHELL_ENV
        .get_or_init(|| async {
            let mut env: HashMap<String, String> = std::env::vars().collect();
            if let Some(shell_env) = login_shell_env().await {
                env.extend(shell_env);
            }
            env
        })
        .await
}

async fn login_shell_env() -> Option<HashMap<String, String>> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let marker = "__SEAM_ENV__";
    let child = Command::new(&shell)
        .args([
            "-l",
            "-i",
            "-c",
            &format!("printf '{marker}'; /usr/bin/env -0"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    // A broken shell profile must not hang the app.
    let out = tokio::time::timeout(Duration::from_secs(5), child)
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let start = text.find(marker)? + marker.len();
    let env: HashMap<String, String> = text[start..]
        .split('\0')
        .filter_map(|entry| {
            entry
                .split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
        })
        .collect();
    (!env.is_empty()).then_some(env)
}

/// Runs git in `dir`. Fails with git's message unless `allow_failure` is set.
pub async fn git(dir: &Path, args: &[&str], opts: RunOptions<'_>) -> Result<Output> {
    let mut cmd = Command::new(git_executable());
    cmd.args(["-c", "color.ui=false", "-c", "core.quotepath=false"])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(base_env().await)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        // Never let git open an interactive editor; commands that need one pass their own.
        .env("GIT_EDITOR", "true")
        .stdin(if opts.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if opts.read {
        cmd.env("GIT_OPTIONAL_LOCKS", "0");
    }
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().map_err(|e| {
        GitError::new(format!(
            "Couldn't run git at {}: {e}",
            git_executable().display()
        ))
    })?;

    if let (Some(input), Some(mut stdin)) = (opts.stdin, child.stdin.take()) {
        let input = input.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(input.as_bytes()).await;
        });
    }

    let mut stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let out_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf).await;
        buf
    });

    // Read stderr here so progress lines reach the caller as they arrive.
    let mut err_bytes = Vec::new();
    let mut reader = BufReader::new(stderr);
    let mut chunk = Vec::new();
    loop {
        chunk.clear();
        // git rewrites progress lines with \r; treat it as a line break.
        let n = read_until_any(&mut reader, &mut chunk).await?;
        if n == 0 {
            break;
        }
        err_bytes.extend_from_slice(&chunk);
        if let Some(progress) = opts.progress {
            let line = String::from_utf8_lossy(&chunk).trim().to_string();
            if !line.is_empty() {
                progress(line);
            }
        }
    }

    let status = child.wait().await?;
    let stdout = out_task.await.unwrap_or_default();
    let output = Output {
        code: status.code().unwrap_or(-1),
        stdout,
        stderr: String::from_utf8_lossy(&err_bytes).into_owned(),
    };
    if output.code != 0 && !opts.allow_failure {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        return Err(GitError::from_stderr(&owned, output.code, &output.stderr));
    }
    Ok(output)
}

async fn read_until_any<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
) -> std::io::Result<usize> {
    let mut total = 0;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(total);
        }
        if let Some(i) = available.iter().position(|&b| b == b'\n' || b == b'\r') {
            buf.extend_from_slice(&available[..=i]);
            reader.consume(i + 1);
            return Ok(total + i + 1);
        }
        let n = available.len();
        buf.extend_from_slice(available);
        reader.consume(n);
        total += n;
    }
}
