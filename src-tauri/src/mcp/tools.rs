use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::Provider;
use crate::git::Repo;
use crate::handoff::{self, Action, Request};
use crate::review::{self, Finding, Review, Scope, Severity, Target};

/// Claude Code reads up to about 25,000 tokens of a tool's answer; the rest of a diff is named.
const DIFF_BUDGET: usize = 60_000;

pub type Opener = Box<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;

pub struct Context {
    pub cwd: PathBuf,
    /// Shows a repository in the app, which then takes the request left for it.
    pub open: Opener,
    pub fallback_provider: Provider,
}

#[derive(Debug)]
pub struct ToolError(pub String);

impl<T: Into<String>> From<T> for ToolError {
    fn from(message: T) -> Self {
        ToolError(message.into())
    }
}

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub read_only: bool,
}

pub const TOOLS: [Tool; 4] = [
    Tool {
        name: "get_diff",
        description: "Read a change in the repository as a unified diff, with its commits and files. Read it before leave_comments so each comment names a file in the diff and a line in its new version.",
        read_only: true,
    },
    Tool {
        name: "open_diff",
        description: "Show a change to the user in Seam, the desktop git client, without commenting on it.",
        read_only: false,
    },
    Tool {
        name: "leave_comments",
        description: "Leave review comments on a change and open them in Seam's Review tab, beside the diff. Put every comment in one call: this replaces the last review Seam kept for the repository.",
        read_only: false,
    },
    Tool {
        name: "get_review",
        description: "Read the review Seam last kept for the repository (from its own AI review, or from leave_comments), to act on its findings.",
        read_only: true,
    },
];

fn subject_schema() -> Value {
    json!({
        "repository": {
            "type": "string",
            "description": "A folder inside the repository. Defaults to the one you are working in."
        },
        "range": {
            "type": "string",
            "description": "A range such as main..HEAD, or one revision such as HEAD~3 meaning everything after it."
        },
        "commit": { "type": "string", "description": "One commit, on its own." },
        "staged": { "type": "boolean", "description": "What is staged for the next commit." },
        "uncommitted": {
            "type": "boolean",
            "description": "Every change not yet committed, staged or not."
        },
        "pull_request": {
            "type": "integer",
            "description": "A pull or merge request by number, read through gh or glab."
        },
        "paths": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Narrow a range, the staged or the uncommitted changes to these paths."
        }
    })
}

const SUBJECT_NOTE: &str = "Name at most one of range, commit, staged, uncommitted or pull_request; with none, it is the current branch against main, or the uncommitted changes when on main.";

pub fn input_schema(name: &str) -> Value {
    let mut properties = subject_schema();
    let mut required: Vec<&str> = vec![];
    match name {
        "leave_comments" => {
            properties["summary"] = json!({
                "type": "string",
                "description": "One or two sentences on the change as a whole."
            });
            properties["comments"] = json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "The file's path as the diff shows it." },
                        "line": {
                            "type": "integer",
                            "description": "A line in the file's new version; leave it out to comment on the whole file."
                        },
                        "severity": {
                            "type": "string",
                            "enum": ["risk", "check", "note"],
                            "description": "risk: likely a bug or a regression. check: worth verifying. note: minor. Defaults to note."
                        },
                        "title": { "type": "string", "description": "The point, in one line." },
                        "body": { "type": "string", "description": "Why it matters, in a few sentences of Markdown." },
                        "suggestion": {
                            "type": "string",
                            "description": "Replacement code for that line, when there is an obvious fix."
                        }
                    },
                    "required": ["file", "title", "body"]
                }
            });
            required = vec!["summary", "comments"];
        }
        "get_review" => {
            properties = json!({ "repository": properties["repository"].clone() });
        }
        _ => {}
    }
    let mut schema = json!({ "type": "object", "properties": properties, "required": required });
    if name != "get_review" {
        schema["description"] = json!(SUBJECT_NOTE);
    }
    schema
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Subject {
    repository: Option<String>,
    range: Option<String>,
    commit: Option<String>,
    staged: bool,
    uncommitted: bool,
    pull_request: Option<u32>,
    paths: Vec<String>,
}

impl Subject {
    fn target(&self) -> Result<Target, ToolError> {
        let named = [
            self.range.is_some(),
            self.commit.is_some(),
            self.staged,
            self.uncommitted,
            self.pull_request.is_some(),
        ];
        if named.iter().filter(|&&given| given).count() > 1 {
            return Err(
                "Name one of range, commit, staged, uncommitted or pull_request, not several."
                    .into(),
            );
        }
        let paths = self.paths.clone();
        let narrowable = self.range.is_some() || self.staged || self.uncommitted;
        if !paths.is_empty() && !narrowable {
            return Err("paths narrow a range, staged or uncommitted changes; name one.".into());
        }
        Ok(if let Some(range) = &self.range {
            Target::Range {
                range: range.clone(),
                paths,
            }
        } else if let Some(rev) = &self.commit {
            Target::Commit { rev: rev.clone() }
        } else if self.staged {
            Target::Staged { paths }
        } else if self.uncommitted {
            Target::WorkingTree { paths }
        } else if let Some(number) = self.pull_request {
            Target::PullRequest { number }
        } else {
            Target::Auto
        })
    }
}

#[derive(Debug, Deserialize)]
struct Comment {
    file: String,
    line: Option<u32>,
    #[serde(default)]
    severity: Option<Severity>,
    title: String,
    body: String,
    suggestion: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Comments {
    #[serde(flatten)]
    subject: Subject,
    summary: String,
    comments: Vec<Comment>,
}

fn arguments<T: for<'de> Deserialize<'de>>(arguments: Value) -> Result<T, ToolError> {
    let arguments = if arguments.is_null() {
        json!({})
    } else {
        arguments
    };
    serde_json::from_value(arguments).map_err(|error| ToolError(format!("Bad arguments: {error}")))
}

async fn repository(context: &Context, folder: Option<&str>) -> Result<(Repo, PathBuf), ToolError> {
    let folder = match folder {
        Some(folder) => context.cwd.join(folder),
        None => context.cwd.clone(),
    };
    let root = Repo::top_level(&folder).await.ok_or_else(|| {
        ToolError(format!(
            "{} is not inside a git repository.",
            folder.display()
        ))
    })?;
    let repo = Repo::new(root);
    let git_dir = repo.git_dir().await.map_err(|error| error.message)?;
    Ok((repo, git_dir))
}

async fn scope(repo: &Repo, target: &Target) -> Result<Scope, ToolError> {
    review::scope_of(repo, target)
        .await
        .map_err(|error| ToolError(error.message))
}

fn stat(scope: &Scope) -> String {
    let files = scope.diffs.len();
    let additions: u32 = scope.diffs.iter().map(|diff| diff.additions).sum();
    let deletions: u32 = scope.diffs.iter().map(|diff| diff.deletions).sum();
    format!(
        "{files} file{} +{additions} -{deletions}",
        if files == 1 { "" } else { "s" }
    )
}

/// The diff a file at a time, until the budget runs out; what is left is named, not cut.
fn bounded(diff_text: &str) -> (String, Vec<String>) {
    let mut shown = String::new();
    let mut left = Vec::new();
    for file in split_files(diff_text) {
        if left.is_empty() && shown.len() + file.len() <= DIFF_BUDGET {
            shown.push_str(file);
        } else {
            left.push(
                file.lines()
                    .next()
                    .and_then(|header| header.rsplit(" b/").next())
                    .unwrap_or_default()
                    .to_string(),
            );
        }
    }
    (shown, left)
}

fn split_files(text: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = text
        .match_indices("diff --git ")
        .map(|(index, _)| index)
        .filter(|&index| index == 0 || text.as_bytes()[index - 1] == b'\n')
        .collect();
    if starts.first() != Some(&0) {
        starts.insert(0, 0);
    }
    starts
        .iter()
        .enumerate()
        .map(|(position, &start)| {
            let end = starts.get(position + 1).copied().unwrap_or(text.len());
            &text[start..end]
        })
        .filter(|file| !file.is_empty())
        .collect()
}

pub async fn call(
    context: &Context,
    client: Option<Provider>,
    name: &str,
    input: Value,
) -> Result<String, ToolError> {
    match name {
        "get_diff" => get_diff(context, arguments(input)?).await,
        "open_diff" => open_diff(context, arguments(input)?).await,
        "leave_comments" => leave_comments(context, client, arguments(input)?).await,
        "get_review" => get_review(context, arguments(input)?).await,
        _ => Err(format!("No tool called {name}.").into()),
    }
}

async fn get_diff(context: &Context, subject: Subject) -> Result<String, ToolError> {
    let target = subject.target()?;
    let (repo, _) = repository(context, subject.repository.as_deref()).await?;
    let scope = scope(&repo, &target).await?;
    let mut out = format!("{} · {}\n", scope.label, stat(&scope));
    if !scope.commits.is_empty() {
        out += "\nCommits:\n";
        for commit in &scope.commits {
            out += &format!("  {commit}\n");
        }
    }
    let (diff, left) = bounded(&scope.diff_text);
    out += &format!("\n{diff}");
    if !left.is_empty() {
        out += &format!(
            "\n[{} more file{} not shown here; ask again with paths to read them: {}]\n",
            left.len(),
            if left.len() == 1 { "" } else { "s" },
            left.join(", ")
        );
    }
    Ok(out)
}

fn show(context: &Context, repo: &Repo, git_dir: &Path, request: Request) -> Result<(), ToolError> {
    handoff::send(git_dir, &request).map_err(|error| error.to_string())?;
    (context.open)(&repo.root).map_err(ToolError)
}

async fn open_diff(context: &Context, subject: Subject) -> Result<String, ToolError> {
    let target = subject.target()?;
    let (repo, git_dir) = repository(context, subject.repository.as_deref()).await?;
    let scope = scope(&repo, &target).await?;
    show(
        context,
        &repo,
        &git_dir,
        Request::new(Action::Open, target, None),
    )?;
    Ok(format!(
        "Opened {} ({}) in Seam's Review tab.",
        scope.label,
        stat(&scope)
    ))
}

async fn leave_comments(
    context: &Context,
    client: Option<Provider>,
    input: Comments,
) -> Result<String, ToolError> {
    let target = input.subject.target()?;
    let (repo, git_dir) = repository(context, input.subject.repository.as_deref()).await?;
    let scope = scope(&repo, &target).await?;
    let files: Vec<&str> = scope
        .diffs
        .iter()
        .flat_map(|diff| [diff.new_path.as_deref(), diff.old_path.as_deref()])
        .flatten()
        .collect();
    let outside: Vec<&str> = input
        .comments
        .iter()
        .map(|comment| comment.file.as_str())
        .filter(|file| !files.contains(file))
        .collect();
    if !outside.is_empty() {
        return Err(format!(
            "Not in {}: {}. Comment on these files: {}.",
            scope.label,
            outside.join(", "),
            files.join(", ")
        )
        .into());
    }
    let findings: Vec<Finding> = input
        .comments
        .into_iter()
        .enumerate()
        .map(|(id, comment)| Finding {
            id,
            file: comment.file,
            line: comment.line.filter(|&line| line > 0),
            severity: comment.severity.unwrap_or(Severity::Note),
            title: comment.title,
            body: comment.body,
            suggestion: comment.suggestion.filter(|text| !text.trim().is_empty()),
        })
        .collect();
    let count = |severity: Severity| {
        findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .count()
    };
    let tally = format!(
        "{} risk, {} check, {} note",
        count(Severity::Risk),
        count(Severity::Check),
        count(Severity::Note)
    );
    let comments = findings.len();
    let label = scope.label.clone();
    let review = Review {
        provider: client.unwrap_or(context.fallback_provider),
        scope: scope.label,
        head_sha: scope.head_sha,
        commits: scope.commits.len(),
        summary: input.summary,
        findings,
        diffs: scope.diffs,
        truncated: false,
        created_at: review::now_secs(),
        duration_ms: 0,
        target: target.clone(),
    };
    review::save(&git_dir, &review);
    show(
        context,
        &repo,
        &git_dir,
        Request::new(Action::ShowReview, target, None),
    )?;
    Ok(format!(
        "Left {comments} comment{} on {label} ({tally}) and opened them in Seam's Review tab.",
        if comments == 1 { "" } else { "s" }
    ))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReviewQuery {
    repository: Option<String>,
}

async fn get_review(context: &Context, query: ReviewQuery) -> Result<String, ToolError> {
    let (repo, git_dir) = repository(context, query.repository.as_deref()).await?;
    let saved = review::load(&git_dir)
        .ok_or("Seam has no review for this repository yet; run one in Seam, or leave_comments.")?;
    let head = repo.head().await;
    let findings: Vec<Value> = saved
        .findings
        .iter()
        .map(|finding| {
            json!({
                "file": finding.file,
                "line": finding.line,
                "severity": finding.severity,
                "title": finding.title,
                "body": finding.body,
                "suggestion": finding.suggestion,
            })
        })
        .collect();
    let answer = json!({
        "scope": saved.scope,
        "by": saved.provider.label(),
        "createdAt": saved.created_at,
        "headMovedSince": saved.head_sha.is_some() && head.is_some() && saved.head_sha != head,
        "summary": saved.summary,
        "findings": findings,
    });
    Ok(serde_json::to_string_pretty(&answer).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_one_change_at_a_time() {
        let subject = |value: Value| serde_json::from_value::<Subject>(value).unwrap();
        assert_eq!(subject(json!({})).target().unwrap(), Target::Auto);
        assert_eq!(
            subject(json!({ "range": "main..HEAD", "paths": ["src"] }))
                .target()
                .unwrap(),
            Target::Range {
                range: "main..HEAD".into(),
                paths: vec!["src".into()]
            }
        );
        assert_eq!(
            subject(json!({ "pull_request": 12 })).target().unwrap(),
            Target::PullRequest { number: 12 }
        );
        assert!(subject(json!({ "staged": true, "commit": "HEAD" }))
            .target()
            .is_err());
        assert!(subject(json!({ "commit": "HEAD", "paths": ["a"] }))
            .target()
            .is_err());
    }

    #[test]
    fn keeps_a_large_diff_to_whole_files() {
        let small = "diff --git a/a.txt b/a.txt\n+a\n";
        let large = format!(
            "diff --git a/big.txt b/big.txt\n{}",
            "+x\n".repeat(DIFF_BUDGET)
        );
        let text = format!("{small}{large}diff --git a/c.txt b/c.txt\n+c\n");
        let (shown, left) = bounded(&text);
        assert_eq!(shown, small);
        assert_eq!(left, ["big.txt", "c.txt"]);
    }

    #[test]
    fn every_tool_has_an_object_schema() {
        for tool in TOOLS {
            let schema = input_schema(tool.name);
            assert_eq!(schema["type"], "object", "{}", tool.name);
        }
        assert_eq!(
            input_schema("leave_comments")["required"],
            json!(["summary", "comments"])
        );
        assert!(input_schema("get_review")["properties"]
            .get("range")
            .is_none());
    }
}
