//! Every model Seam can talk to, natively: a coding agent's own CLI (signed in as the user)
//! or a provider's HTTP API with a key kept in the system keychain.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::git::process::base_env;
use crate::git::{GitError, Result};

const AGENT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const API_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const KEYCHAIN_SERVICE: &str = "app.seam.desktop.ai";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    #[default]
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "opencode", alias = "openCode")]
    OpenCode,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "google")]
    Google,
    /// Ollama, LM Studio, OpenRouter, a gateway: anything speaking OpenAI's chat API.
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Harness,
    Api,
}

impl Provider {
    pub const ALL: [Provider; 7] = [
        Provider::Claude,
        Provider::Codex,
        Provider::OpenCode,
        Provider::Anthropic,
        Provider::OpenAi,
        Provider::Google,
        Provider::OpenAiCompatible,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Provider::Claude => "Claude Code",
            Provider::Codex => "Codex",
            Provider::OpenCode => "OpenCode",
            Provider::Anthropic => "Anthropic",
            Provider::OpenAi => "OpenAI",
            Provider::Google => "Google Gemini",
            Provider::OpenAiCompatible => "OpenAI-compatible endpoint",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
            Provider::OpenCode => "opencode",
            Provider::Anthropic => "anthropic",
            Provider::OpenAi => "openai",
            Provider::Google => "google",
            Provider::OpenAiCompatible => "openai-compatible",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Provider::Claude | Provider::Codex | Provider::OpenCode => Kind::Harness,
            _ => Kind::Api,
        }
    }

    fn program(self) -> Option<&'static str> {
        match self {
            Provider::Claude => Some("claude"),
            Provider::Codex => Some("codex"),
            Provider::OpenCode => Some("opencode"),
            _ => None,
        }
    }

    /// Only where a model id is safe to assume; otherwise the user names one.
    pub fn default_model(self) -> Option<&'static str> {
        match self {
            Provider::Anthropic => Some("claude-sonnet-5-5"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// Only what should stop a merge.
    Blockers,
    #[default]
    Balanced,
    /// Everything, including readability and suggestions.
    Thorough,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReviewStyle {
    /// Any of: bugs, security, tests, breaking, performance, style.
    pub focus: Vec<String>,
    pub level: Level,
    /// Language the comments are written in, e.g. "English".
    pub language: String,
}

impl Default for ReviewStyle {
    fn default() -> Self {
        ReviewStyle {
            focus: ["bugs", "security", "tests", "breaking"]
                .map(String::from)
                .to_vec(),
            level: Level::Balanced,
            language: "English".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AiConfig {
    pub provider: Provider,
    /// Empty: the provider's own default.
    pub model: String,
    /// Only for `openai-compatible`, up to and including `/v1`.
    pub base_url: String,
    pub review: ReviewStyle,
}

impl AiConfig {
    fn model(&self) -> Option<String> {
        let model = self.model.trim();
        if model.is_empty() {
            self.provider.default_model().map(String::from)
        } else {
            Some(model.to_string())
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: Provider,
    pub label: &'static str,
    pub kind: Kind,
    /// A harness is installed / an API is usable (has a key, or needs none).
    pub available: bool,
    pub path: Option<String>,
    pub has_key: bool,
    pub default_model: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub reply: String,
    pub duration_ms: u64,
}

// MARK: Keychain

fn keychain_entry(provider: Provider) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, provider.id())
        .map_err(|e| GitError::new(format!("Keychain: {e}")))
}

pub fn key(provider: Provider) -> Option<String> {
    keychain_entry(provider)
        .ok()?
        .get_password()
        .ok()
        .filter(|key| !key.is_empty())
}

pub fn set_key(provider: Provider, key: &str) -> Result<()> {
    keychain_entry(provider)?
        .set_password(key.trim())
        .map_err(|e| GitError::new(format!("Couldn't save the key in the keychain: {e}")))
}

pub fn clear_key(provider: Provider) -> Result<()> {
    match keychain_entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(GitError::new(format!("Couldn't remove the key: {e}"))),
    }
}

// MARK: Discovery

/// Finds `name` on the login shell's PATH, then where these tools install themselves.
pub async fn find_program(name: &str) -> Option<PathBuf> {
    let path = base_env().await.get("PATH").cloned().unwrap_or_default();
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
    let extra = [
        home.join(".local/bin"),
        home.join(".claude/local"),
        home.join(".opencode/bin"),
        home.join(".bun/bin"),
        home.join(".npm-global/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    std::env::split_paths(&path)
        .chain(extra)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub async fn statuses() -> Vec<ProviderStatus> {
    let mut list = Vec::new();
    for provider in Provider::ALL {
        let path = match provider.program() {
            Some(program) => find_program(program)
                .await
                .map(|p| p.to_string_lossy().into_owned()),
            None => None,
        };
        let has_key = provider.kind() == Kind::Api && key(provider).is_some();
        let available = match provider.kind() {
            Kind::Harness => path.is_some(),
            Kind::Api => has_key || provider == Provider::OpenAiCompatible,
        };
        list.push(ProviderStatus {
            id: provider,
            label: provider.label(),
            kind: provider.kind(),
            available,
            path,
            has_key,
            default_model: provider.default_model(),
        });
    }
    list
}

// MARK: Asking

/// Sends `prompt` and returns the model's final answer. A harness runs in `cwd`,
/// read-only, so it can open the files around a change.
pub async fn ask(config: &AiConfig, cwd: &Path, prompt: &str) -> Result<String> {
    match config.provider.kind() {
        Kind::Harness => ask_harness(config, cwd, prompt).await,
        Kind::Api => tokio::time::timeout(API_TIMEOUT, ask_api(config, prompt))
            .await
            .map_err(|_| {
                GitError::new(format!(
                    "{} didn't answer within 5 minutes.",
                    config.provider.label()
                ))
            })?,
    }
}

pub async fn test(config: &AiConfig) -> Result<TestResult> {
    let started = Instant::now();
    let cwd = std::env::temp_dir();
    let reply = ask(config, &cwd, "Reply with exactly one word: ready").await?;
    Ok(TestResult {
        reply: strip_ansi(&reply).trim().chars().take(200).collect(),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

async fn ask_harness(config: &AiConfig, cwd: &Path, prompt: &str) -> Result<String> {
    let provider = config.provider;
    let program_name = provider.program().unwrap_or_default();
    let program = find_program(program_name).await.ok_or_else(|| {
        GitError::new(format!(
            "{} isn't installed (no `{program_name}` on your PATH).",
            provider.label()
        ))
    })?;
    let work = std::env::temp_dir().join(format!("seam-ai-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work)?;
    let answer_file = work.join("answer.txt");
    let prompt_file = work.join("request.md");
    let model = config.model();

    let mut command = Command::new(&program);
    command
        .current_dir(cwd)
        .envs(base_env().await)
        .kill_on_drop(true);
    let mut stdin_prompt = true;
    match provider {
        Provider::Claude => {
            command.args([
                "-p",
                "--output-format",
                "text",
                "--allowedTools",
                "Read,Grep,Glob",
            ]);
            if let Some(model) = &model {
                command.args(["--model", model]);
            }
        }
        Provider::Codex => {
            command.args([
                "exec",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--color",
                "never",
            ]);
            if let Some(model) = &model {
                command.args(["-m", model]);
            }
            command.arg("-o").arg(&answer_file).arg("-");
        }
        Provider::OpenCode => {
            // Long prompts don't fit in argv; the request goes in an attached file.
            std::fs::write(&prompt_file, prompt)?;
            command.arg("run").arg("--dir").arg(cwd);
            if let Some(model) = &model {
                command.args(["-m", model]);
            }
            // The message goes before `-f`, which takes a list and would swallow it as a file.
            command
                .arg("Follow the instructions in the attached request exactly.")
                .arg("-f")
                .arg(&prompt_file);
            stdin_prompt = false;
        }
        _ => unreachable!("not a harness"),
    }
    command
        .stdin(if stdin_prompt {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| GitError::new(format!("Couldn't start {}: {e}", provider.label())))?;
    if stdin_prompt {
        if let Some(mut stdin) = child.stdin.take() {
            let input = prompt.to_string();
            tokio::spawn(async move {
                let _ = stdin.write_all(input.as_bytes()).await;
            });
        }
    }
    let output = tokio::time::timeout(AGENT_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| {
            GitError::new(format!(
                "{} didn't finish within 15 minutes.",
                provider.label()
            ))
        })??;
    let answer = std::fs::read_to_string(&answer_file)
        .ok()
        .filter(|a| !a.trim().is_empty());
    let _ = std::fs::remove_dir_all(&work);
    if !output.status.success() {
        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
        let tail: Vec<&str> = stderr.trim().lines().rev().take(8).collect();
        let message = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        return Err(GitError::new(format!(
            "{} failed: {}",
            provider.label(),
            if message.is_empty() {
                "no output"
            } else {
                &message
            }
        )));
    }
    Ok(answer.unwrap_or_else(|| String::from_utf8_lossy(&output.stdout).into_owned()))
}

async fn ask_api(config: &AiConfig, prompt: &str) -> Result<String> {
    let provider = config.provider;
    let model = config.model().ok_or_else(|| {
        GitError::new(format!(
            "Name a {} model in Settings › AI.",
            provider.label()
        ))
    })?;
    let key = key(provider);
    if key.is_none() && provider != Provider::OpenAiCompatible {
        return Err(GitError::new(format!(
            "Add a {} API key in Settings › AI.",
            provider.label()
        )));
    }
    let client = reqwest::Client::new();

    let request = match provider {
        Provider::Anthropic => client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key.unwrap_or_default())
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": model,
                "max_tokens": 8192,
                "messages": [{ "role": "user", "content": prompt }],
            })),
        Provider::Google => client
            .post(format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            ))
            .header("x-goog-api-key", key.unwrap_or_default())
            .json(&json!({ "contents": [{ "role": "user", "parts": [{ "text": prompt }] }] })),
        Provider::OpenAi | Provider::OpenAiCompatible => {
            let base = if provider == Provider::OpenAi {
                "https://api.openai.com/v1".to_string()
            } else {
                let base = config.base_url.trim().trim_end_matches('/').to_string();
                if base.is_empty() {
                    return Err(GitError::new("Set the endpoint URL in Settings › AI (Ollama's is http://localhost:11434/v1)."));
                }
                base
            };
            let mut request = client
                .post(format!("{base}/chat/completions"))
                .json(&json!({
                    "model": model,
                    "messages": [{ "role": "user", "content": prompt }],
                }));
            if let Some(key) = key {
                request = request.bearer_auth(key);
            }
            request
        }
        _ => unreachable!("not an API"),
    };

    let response = request
        .send()
        .await
        .map_err(|e| GitError::new(format!("Couldn't reach {}: {e}", provider.label())))?;
    let status = response.status();
    let body: Value = response.json().await.map_err(|e| {
        GitError::new(format!(
            "{} sent an unreadable answer: {e}",
            provider.label()
        ))
    })?;
    if !status.is_success() {
        let message = body
            .pointer("/error/message")
            .and_then(Value::as_str)
            .or_else(|| body.pointer("/error").and_then(Value::as_str))
            .unwrap_or("no details");
        return Err(GitError::new(format!(
            "{} answered {status}: {message}",
            provider.label()
        )));
    }

    let text = match provider {
        Provider::Anthropic => body["content"].as_array().map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        }),
        Provider::Google => body
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            }),
        _ => body
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(String::from),
    };
    text.filter(|t| !t.trim().is_empty())
        .ok_or_else(|| GitError::new(format!("{} returned an empty answer.", provider.label())))
}

pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_round_trip_and_accept_the_old_agent_names() {
        for provider in Provider::ALL {
            let json = serde_json::to_string(&provider).unwrap();
            assert_eq!(json, format!("\"{}\"", provider.id()));
            assert_eq!(serde_json::from_str::<Provider>(&json).unwrap(), provider);
        }
        assert_eq!(
            serde_json::from_str::<Provider>("\"openCode\"").unwrap(),
            Provider::OpenCode
        );
    }

    #[test]
    fn strips_terminal_colours() {
        assert_eq!(strip_ansi("\u{1b}[32m{\"a\":1}\u{1b}[0m"), "{\"a\":1}");
    }
}
