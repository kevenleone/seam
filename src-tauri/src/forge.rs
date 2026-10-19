use std::path::Path;
use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command;

use crate::ai::find_program;
use crate::git::process::base_env;
use crate::git::{GitError, Repo, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    GitLab,
}

impl Forge {
    pub const ALL: [Forge; 2] = [Forge::GitHub, Forge::GitLab];

    pub fn program(self) -> &'static str {
        match self {
            Forge::GitHub => "gh",
            Forge::GitLab => "glab",
        }
    }
}

pub struct PullRequest {
    pub label: String,
    pub commits: Vec<String>,
    pub diff_text: String,
}

/// `git@host:owner/repo.git`, `ssh://git@host:22/owner/repo`, `https://host/owner/repo`.
pub fn host_of(url: &str) -> Option<String> {
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest.split('/').next()?,
        None => url.split(':').next()?,
    };
    let host = rest.rsplit('@').next()?;
    let host = host.split(':').next()?;
    (!host.is_empty()).then(|| host.to_lowercase())
}

pub fn forge_by_name(host: &str) -> Option<Forge> {
    if host == "github.com" || host.ends_with(".github.com") {
        Some(Forge::GitHub)
    } else if host.contains("gitlab") {
        Some(Forge::GitLab)
    } else {
        None
    }
}

pub async fn remote_host(repo: &Repo) -> Result<String> {
    let remotes = repo.remotes().await?;
    let remote = remotes
        .iter()
        .find(|remote| remote.name == "origin")
        .or(remotes.first())
        .ok_or_else(|| GitError::new("This repository has no remote to find the request on."))?;
    host_of(&remote.url)
        .ok_or_else(|| GitError::new(format!("Couldn't read a host from {}.", remote.url)))
}

/// By the host's name, else by whichever CLI is signed in to it (GitHub Enterprise, self-hosted GitLab).
pub async fn detect(repo: &Repo) -> Result<(Forge, String)> {
    let host = remote_host(repo).await?;
    if let Some(forge) = forge_by_name(&host) {
        return Ok((forge, host));
    }
    for forge in Forge::ALL {
        if signed_in(forge, &host, &repo.root).await {
            return Ok((forge, host));
        }
    }
    Err(GitError::new(format!(
        "Couldn't tell whether {host} is GitHub or GitLab: sign in with `gh auth login --hostname {host}` or `glab auth login --hostname {host}`."
    )))
}

pub async fn signed_in(forge: Forge, host: &str, cwd: &Path) -> bool {
    run(forge, cwd, &["auth", "status", "--hostname", host])
        .await
        .is_ok()
}

async fn run(forge: Forge, cwd: &Path, args: &[&str]) -> Result<String> {
    let name = forge.program();
    let program = find_program(name).await.ok_or_else(|| {
        GitError::new(format!(
            "This needs `{name}`: install it and sign in with `{name} auth login`."
        ))
    })?;
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .envs(base_env().await)
        .env("NO_COLOR", "1")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GLAB_NO_PROMPT", "1")
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitError::new(if message.is_empty() {
            format!("`{name} {}` failed.", args.join(" "))
        } else {
            message
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[derive(Deserialize)]
struct GitHubCommit {
    oid: String,
    #[serde(rename = "messageHeadline")]
    headline: String,
}

#[derive(Deserialize)]
struct GitHubRequest {
    title: String,
    #[serde(default)]
    commits: Vec<GitHubCommit>,
}

#[derive(Deserialize)]
struct GitLabRequest {
    title: String,
}

pub async fn pull_request(repo: &Repo, number: u32) -> Result<PullRequest> {
    let (forge, _) = detect(repo).await?;
    let id = number.to_string();
    let cwd = repo.root.as_path();
    match forge {
        Forge::GitHub => {
            let view = run(forge, cwd, &["pr", "view", &id, "--json", "title,commits"]).await?;
            let view: GitHubRequest = serde_json::from_str(&view)
                .map_err(|error| GitError::new(format!("Unexpected answer from gh: {error}")))?;
            let diff_text = run(forge, cwd, &["pr", "diff", &id, "--color", "never"]).await?;
            Ok(PullRequest {
                label: format!("PR #{number} · {}", view.title),
                commits: view
                    .commits
                    .iter()
                    .rev()
                    .map(|commit| {
                        let short = commit.oid.get(..7).unwrap_or(&commit.oid);
                        format!("{short} {}", commit.headline)
                    })
                    .collect(),
                diff_text,
            })
        }
        Forge::GitLab => {
            let view = run(forge, cwd, &["mr", "view", &id, "--output", "json"]).await?;
            let view: GitLabRequest = serde_json::from_str(&view)
                .map_err(|error| GitError::new(format!("Unexpected answer from glab: {error}")))?;
            let diff_text = run(forge, cwd, &["mr", "diff", &id, "--raw"]).await?;
            Ok(PullRequest {
                label: format!("MR !{number} · {}", view.title),
                commits: vec![],
                diff_text,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_host_of_every_remote_form() {
        let cases = [
            ("git@github.com:kevenleone/seam.git", "github.com"),
            (
                "ssh://git@gitlab.example.com:2222/team/app.git",
                "gitlab.example.com",
            ),
            ("https://github.com/kevenleone/seam", "github.com"),
            ("https://user:token@GitLab.com/team/app.git", "gitlab.com"),
            ("git.corp.local:team/app.git", "git.corp.local"),
        ];
        for (url, host) in cases {
            assert_eq!(host_of(url).as_deref(), Some(host), "{url}");
        }
    }

    #[test]
    fn names_the_forge_by_its_host_when_it_can() {
        assert_eq!(forge_by_name("github.com"), Some(Forge::GitHub));
        assert_eq!(forge_by_name("gitlab.com"), Some(Forge::GitLab));
        assert_eq!(forge_by_name("gitlab.corp.local"), Some(Forge::GitLab));
        assert_eq!(forge_by_name("git.corp.local"), None);
    }
}
