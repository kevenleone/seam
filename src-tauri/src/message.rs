use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ai::{self, AiConfig, Kind};
use crate::git::{repo, GitError, Repo, Result};
use crate::{overview, review};

const STYLE_SAMPLE: &str = "20";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CommitMessage {
    pub summary: String,
    pub body: String,
}

/// What a commit would record: the index against HEAD, or against HEAD's parent when amending.
pub async fn staged_diff(repo: &Repo, amend: bool) -> Result<String> {
    let wanted = if amend { "HEAD^" } else { "HEAD" };
    let exists = repo
        .run(&[
            "rev-parse",
            "--verify",
            "-q",
            &format!("{wanted}^{{commit}}"),
        ])
        .await
        .is_ok();
    let base = if exists { wanted } else { repo::EMPTY_TREE };
    Ok(repo
        .run(&[
            "diff",
            "--cached",
            "--no-color",
            "--no-ext-diff",
            "-M",
            base,
        ])
        .await?
        .text())
}

fn prompt(
    config: &AiConfig,
    diff: &str,
    truncated: bool,
    recent: &str,
    project: &str,
    draft: &str,
) -> String {
    let access = match config.provider.kind() {
        Kind::Harness => "You may read files in the repository for context, but do not modify any file or run git commands that change anything.",
        Kind::Api => "You only have the diff below.",
    };
    let recent = if recent.trim().is_empty() {
        "(none yet)".to_string()
    } else {
        recent.trim().to_string()
    };
    let project = if project.trim().is_empty() {
        String::new()
    } else {
        format!("\nAbout the project:\n{}\n", project.trim())
    };
    let draft = if draft.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\nThe author's own draft, to keep the intent of:\n{}\n",
            draft.trim()
        )
    };
    format!(
        r#"Write the git commit message for the staged change below. {access}

Match the conventions of the repository's recent commit subjects (prefixes such as `feat:` or `fix(scope):`, ticket ids, tense, capitalisation). The summary is one line of at most 72 characters, says what the change does, and has no trailing period. The body is optional: leave it empty for a small, self-explanatory change; otherwise give a few short lines on why, wrapped at 72 characters, with no headings and no restating of the diff.

Answer with one JSON object and nothing else:
{{"summary": "...", "body": "..."}}

Recent commit subjects:
{recent}
{project}{draft}
Staged diff{trunc}:
{diff}"#,
        trunc = if truncated {
            " (truncated: large change)"
        } else {
            ""
        },
    )
}

pub async fn run(
    config: &AiConfig,
    root: &Path,
    amend: bool,
    draft: &str,
) -> Result<CommitMessage> {
    let repo = Repo::new(root);
    let diff_text = staged_diff(&repo, amend).await?;
    if diff_text.trim().is_empty() {
        return Err(GitError::new("Nothing staged to describe."));
    }
    let (diff, truncated) = review::clip(&diff_text);
    let recent = match repo.head().await {
        Some(_) => repo
            .run(&["log", "-n", STYLE_SAMPLE, "--no-merges", "--format=%s"])
            .await
            .map(|output| output.text())
            .unwrap_or_default(),
        None => String::new(),
    };
    let project = match repo.git_dir().await {
        Ok(dir) => overview::project_context(&dir),
        Err(_) => String::new(),
    };
    let answer = ai::ask(
        config,
        root,
        &prompt(config, diff, truncated, &recent, &project, draft),
    )
    .await?;
    parse(&answer)
}

pub fn parse(answer: &str) -> Result<CommitMessage> {
    let message: CommitMessage = ai::json_object(answer).ok_or_else(|| ai::not_json(answer))?;
    let summary = message
        .summary
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.')
        .to_string();
    if summary.is_empty() {
        return Err(ai::not_json(answer));
    }
    Ok(CommitMessage {
        summary,
        body: message.body.trim().to_string(),
    })
}
