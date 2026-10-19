use std::path::Path;

use crate::ai::{self, AiConfig, Kind};
use crate::git::{GitError, Result};
use crate::review::{self, Scope};

fn prompt(config: &AiConfig, scope: &Scope, diff: &str, truncated: bool) -> String {
    let access = match config.provider.kind() {
        Kind::Harness => "Read the code around the change in the repository when it helps, but do not modify any file.",
        Kind::Api => "You only have the diff below; say so when a conclusion depends on code you cannot see.",
    };
    let language = match config.review.language.trim() {
        "" => "English",
        language => language,
    };
    format!(
        r#"Explain a code change in this repository ({label}) in plain language, for a developer who knows the project but has not seen this change. {access}

Say what it does, why it was most likely made, and anything a reader should watch out for. Two or three short paragraphs of plain text: no headings, no lists, no code fences, no preamble. Write in {language}.

Commits:
{commits}

Diff{trunc}:
{diff}"#,
        label = scope.label,
        commits = scope.commits.join("\n"),
        trunc = if truncated {
            " (truncated: large change)"
        } else {
            ""
        },
    )
}

pub async fn run(config: &AiConfig, root: &Path, scope: &Scope) -> Result<String> {
    let (diff, truncated) = review::clip(&scope.diff_text);
    let answer = ai::ask(config, root, &prompt(config, scope, diff, truncated)).await?;
    let answer = ai::strip_ansi(&answer).trim().to_string();
    if answer.is_empty() {
        return Err(GitError::new(format!(
            "{} gave no answer.",
            config.provider.label()
        )));
    }
    Ok(answer)
}
