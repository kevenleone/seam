//! Rewrites commits on the current branch by running `git rebase -i` with a todo list we
//! write ourselves, so no editor ever opens.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::process::{GitError, Result, RunOptions};
use super::repo::Repo;

pub enum Step {
    Pick,
    Drop,
    Reword(String),
}

/// Replays every commit from `oldest` to HEAD, choosing a step per commit.
pub async fn rewrite(repo: &Repo, oldest: &str, step: impl Fn(&str) -> Step) -> Result<()> {
    let commits = commits_to_replay(repo, oldest).await?;
    let dir = work_dir()?;
    let mut todo = Vec::new();
    for sha in &commits {
        match step(sha) {
            Step::Pick => todo.push(format!("pick {sha}")),
            Step::Drop => todo.push(format!("drop {sha}")),
            Step::Reword(message) => {
                todo.push(format!("pick {sha}"));
                todo.push(amend_exec(&dir, sha, &message)?);
            }
        }
    }
    run_rebase(repo, oldest, &todo, &dir).await
}

/// Gathers `shas` right after the oldest of them, in their original order, as one commit.
pub async fn squash(
    repo: &Repo,
    oldest: &str,
    shas: &HashSet<String>,
    message: &str,
) -> Result<()> {
    let commits = commits_to_replay(repo, oldest).await?;
    let dir = work_dir()?;
    let selected: Vec<&String> = commits.iter().filter(|c| shas.contains(*c)).collect();
    let mut todo = Vec::new();
    for sha in &commits {
        if sha == oldest {
            todo.push(format!("pick {sha}"));
            todo.extend(selected.iter().skip(1).map(|s| format!("fixup {s}")));
            todo.push(amend_exec(&dir, "squash", message)?);
        } else if !shas.contains(sha) {
            todo.push(format!("pick {sha}"));
        }
    }
    run_rebase(repo, oldest, &todo, &dir).await
}

async fn commits_to_replay(repo: &Repo, oldest: &str) -> Result<Vec<String>> {
    let parents = repo.parents(oldest).await?;
    let range = parents
        .first()
        .map(|p| format!("{p}..HEAD"))
        .unwrap_or_else(|| "HEAD".into());
    let list = repo
        .run_with(
            &["rev-list", "--reverse", "--topo-order", "--parents", &range],
            RunOptions {
                read: true,
                ..Default::default()
            },
        )
        .await?
        .text();
    let mut commits = Vec::new();
    for line in list.lines() {
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() > 2 {
            return Err(GitError::new("There is a merge commit between this commit and HEAD. Seam can't rewrite past merges yet."));
        }
        commits.push(parts[0].to_string());
    }
    if commits.first().map(String::as_str) != Some(oldest) {
        return Err(GitError::new("That commit is not on the current branch."));
    }
    Ok(commits)
}

async fn run_rebase(repo: &Repo, oldest: &str, todo: &[String], dir: &Path) -> Result<()> {
    let todo_file = dir.join("todo");
    std::fs::write(&todo_file, todo.join("\n") + "\n")?;
    let parents = repo.parents(oldest).await?;
    let mut args = vec![
        "-c",
        "rebase.autoSquash=false",
        "-c",
        "rebase.updateRefs=false",
        "rebase",
        "-i",
        "--autostash",
        "--keep-empty",
    ];
    match parents.first() {
        Some(p) => args.push(p),
        None => args.push("--root"),
    }
    repo.run_with(
        &args,
        RunOptions {
            env: vec![
                (
                    "GIT_SEQUENCE_EDITOR",
                    format!("cp {}", shell_quote(&todo_file.to_string_lossy())),
                ),
                ("GIT_EDITOR", "true".into()),
            ],
            ..Default::default()
        },
    )
    .await?;
    // On a conflict the rebase pauses and its exec lines still need the message files,
    // so the folder is only removed once the rebase has finished.
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

fn amend_exec(dir: &Path, name: &str, message: &str) -> Result<String> {
    let file = dir.join(format!("msg-{name}"));
    std::fs::write(&file, message)?;
    Ok(format!(
        "exec git commit --amend --allow-empty --no-verify --cleanup=strip -F {}",
        shell_quote(&file.to_string_lossy())
    ))
}

fn work_dir() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("seam-rebase-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
