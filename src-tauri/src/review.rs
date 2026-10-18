//! AI review of a branch, run through a coding agent the user already has installed.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ai::{self, AiConfig, Kind, Level, Provider};
use crate::git::models::FileDiff;
use crate::git::repo;
use crate::git::{parse, GitError, Repo, Result};
use crate::overview;

const MAX_DIFF_CHARS: usize = 400_000;
const BASE_CANDIDATES: [&str; 4] = ["main", "master", "develop", "trunk"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Risk,
    Check,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: usize,
    pub file: String,
    pub line: Option<u32>,
    pub severity: Severity,
    pub title: String,
    pub body: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    #[serde(alias = "agent")]
    pub provider: Provider,
    /// What was reviewed, e.g. `main…feature/x` or `Uncommitted changes`.
    pub scope: String,
    pub head_sha: Option<String>,
    pub commits: usize,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub diffs: Vec<FileDiff>,
    pub truncated: bool,
    pub created_at: i64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ReviewState {
    Idle,
    /// Carries the change being read, so it can be shown while the model works.
    Running {
        provider: Provider,
        scope: String,
        files: usize,
        started_at: i64,
        commits: Vec<String>,
        diffs: Vec<FileDiff>,
    },
    Done {
        review: Review,
    },
    Failed {
        provider: Provider,
        message: String,
        previous: Option<Review>,
    },
}

/// The change a review looks at: the branch against its base, or uncommitted work.
pub struct Scope {
    pub label: String,
    pub head_sha: Option<String>,
    pub commits: Vec<String>,
    pub diff_text: String,
    pub diffs: Vec<FileDiff>,
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn text(repo: &Repo, args: &[&str]) -> Result<String> {
    Ok(repo.run(args).await?.text())
}

async fn base_ref(repo: &Repo, head: Option<&str>) -> Option<String> {
    let branches = repo.branches().await.ok()?;
    let has = |name: &str| branches.iter().any(|b| b.name == name);
    BASE_CANDIDATES
        .iter()
        .map(|c| c.to_string())
        .find(|c| has(c) && Some(c.as_str()) != head)
        .or_else(|| {
            BASE_CANDIDATES
                .iter()
                .map(|c| format!("origin/{c}"))
                .find(|c| {
                    has(c) && head.map(|h| format!("origin/{h}")).as_deref() != Some(c.as_str())
                })
        })
}

/// The branch's commits since it left its base; uncommitted changes when there are none.
pub async fn scope(repo: &Repo) -> Result<Scope> {
    let head_sha = repo.head().await;
    let branch = repo.current_branch().await;
    if let (Some(head), Some(base)) = (head_sha.as_deref(), base_ref(repo, branch.as_deref()).await)
    {
        if let Some(merge_base) = repo.merge_base(&base, head).await {
            let range = format!("{merge_base}..{head}");
            let commits: Vec<String> = text(repo, &["log", "--format=%h %s", &range])
                .await?
                .lines()
                .map(String::from)
                .collect();
            if !commits.is_empty() {
                let diff_text = text(
                    repo,
                    &[
                        "diff",
                        "--no-color",
                        "--no-ext-diff",
                        "-M",
                        &merge_base,
                        head,
                    ],
                )
                .await?;
                return Ok(Scope {
                    label: format!("{base}…{}", branch.as_deref().unwrap_or(&head[..7])),
                    head_sha: Some(head.to_string()),
                    commits,
                    diffs: parse::diff(&diff_text),
                    diff_text,
                });
            }
        }
    }
    let against = if head_sha.is_some() {
        "HEAD"
    } else {
        repo::EMPTY_TREE
    };
    let diff_text = text(
        repo,
        &["diff", "--no-color", "--no-ext-diff", "-M", against],
    )
    .await?;
    if diff_text.trim().is_empty() {
        return Err(GitError::new(
            "Nothing to review: this branch has no commits beyond its base and no uncommitted changes.",
        ));
    }
    Ok(Scope {
        label: "Uncommitted changes".into(),
        head_sha,
        commits: vec![],
        diffs: parse::diff(&diff_text),
        diff_text,
    })
}

const FOCUS: [(&str, &str); 6] = [
    (
        "bugs",
        "bugs and wrong logic: conditions, edge cases, null handling",
    ),
    (
        "security",
        "security problems: untrusted input, secrets, missing auth checks",
    ),
    ("tests", "changed behaviour with no test covering it"),
    (
        "breaking",
        "breaking changes to public APIs, response shapes, configs or data",
    ),
    (
        "performance",
        "performance: extra queries, loops, blocking calls",
    ),
    ("style", "readability: naming, dead code, long functions"),
];

fn prompt(config: &AiConfig, scope: &Scope, diff: &str, truncated: bool, project: &str) -> String {
    let commits = if scope.commits.is_empty() {
        "(uncommitted changes, no commits)".to_string()
    } else {
        scope.commits.join("\n")
    };
    let style = &config.review;
    let focus: Vec<&str> = FOCUS
        .iter()
        .filter(|(id, _)| style.focus.iter().any(|f| f == id))
        .map(|(_, text)| *text)
        .collect();
    let focus = if focus.is_empty() {
        FOCUS[..4].iter().map(|(_, t)| *t).collect()
    } else {
        focus
    };
    let level = match style.level {
        Level::Blockers => "Only report what should stop a merge. Expect zero to two findings on most changes.",
        Level::Balanced => "Report risks and things worth checking, plus short notes on behaviour changes. No style nitpicks. At most 15 findings.",
        Level::Thorough => "Be thorough: everything above plus suggestions and readability notes. At most 25 findings.",
    };
    let context = match config.provider.kind() {
        Kind::Harness => "Read surrounding code in the repository when it helps, but do not modify any file.",
        Kind::Api => "You only have the diff below; say so in a finding when a conclusion depends on code you cannot see.",
    };
    let language = if style.language.trim().is_empty() {
        "English"
    } else {
        style.language.trim()
    };
    format!(
        r#"You are reviewing a code change in this repository ({label}). {context}

Look for:
- {focus}

{level} Skip anything a formatter or linter would catch.

Severity:
- "risk": likely to cause a bug, data loss or a security problem; should block a merge.
- "check": worth verifying before merging.
- "note": useful context for the reviewer, e.g. a behaviour change.

Write the summary, titles and bodies in {language}.

Answer with ONLY one JSON object, no prose before or after it and no code fence:
{{
  "summary": "One or two sentences on what the change does and how risky it is.",
  "findings": [
    {{
      "file": "path/relative/to/repo/root",
      "line": 42,
      "severity": "risk",
      "title": "Short title",
      "body": "What is wrong and why it matters, in plain sentences.",
      "suggestion": "Optional replacement code for that line, or null"
    }}
  ]
}}

"line" is a line number in the NEW version of the file (the + side of the diff), or null if the finding is about the whole file.

{project}Commits:
{commits}

Diff{trunc}:
{diff}"#,
        label = scope.label,
        project = if project.is_empty() {
            String::new()
        } else {
            format!("{project}\n\n")
        },
        focus = focus.join("\n- "),
        trunc = if truncated {
            " (truncated: large change)"
        } else {
            ""
        },
    )
}

#[derive(Deserialize)]
struct RawFinding {
    file: Option<String>,
    line: Option<serde_json::Value>,
    severity: Option<String>,
    title: Option<String>,
    body: Option<String>,
    suggestion: Option<String>,
}

#[derive(Deserialize)]
struct RawReview {
    summary: Option<String>,
    #[serde(default)]
    findings: Vec<RawFinding>,
}

fn parse_answer(answer: &str) -> Result<(String, Vec<Finding>)> {
    let raw: RawReview = ai::json_object(answer).ok_or_else(|| ai::not_json(answer))?;
    let findings = raw
        .findings
        .into_iter()
        .enumerate()
        .map(|(id, f)| Finding {
            id,
            file: f
                .file
                .unwrap_or_default()
                .trim_start_matches("./")
                .to_string(),
            line: f
                .line
                .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
                .map(|n| n as u32),
            severity: match f.severity.as_deref().map(str::to_lowercase).as_deref() {
                Some("risk") | Some("high") | Some("blocker") => Severity::Risk,
                Some("note") | Some("info") | Some("low") => Severity::Note,
                _ => Severity::Check,
            },
            title: f.title.unwrap_or_default(),
            body: f.body.unwrap_or_default(),
            suggestion: f.suggestion.filter(|s| !s.trim().is_empty()),
        })
        .collect();
    Ok((raw.summary.unwrap_or_default(), findings))
}

/// The diff as sent to a model, cut at `MAX_DIFF_CHARS`, and whether it was cut.
pub fn clip(diff_text: &str) -> (&str, bool) {
    if diff_text.len() <= MAX_DIFF_CHARS {
        return (diff_text, false);
    }
    let mut cut = MAX_DIFF_CHARS;
    while !diff_text.is_char_boundary(cut) {
        cut -= 1;
    }
    (&diff_text[..cut], true)
}

pub async fn run(config: &AiConfig, root: &Path, scope: Scope) -> Result<Review> {
    let started = Instant::now();
    let (diff, truncated) = clip(&scope.diff_text);
    let project = match Repo::new(root).git_dir().await {
        Ok(dir) => overview::project_context(&dir),
        Err(_) => String::new(),
    };
    let answer = ai::ask(
        config,
        root,
        &prompt(config, &scope, diff, truncated, &project),
    )
    .await?;
    let (summary, findings) = parse_answer(&answer)?;
    Ok(Review {
        provider: config.provider,
        scope: scope.label,
        head_sha: scope.head_sha,
        commits: scope.commits.len(),
        summary,
        findings,
        diffs: scope.diffs,
        truncated,
        created_at: now_secs(),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

/// The last review is kept in the git directory, so it survives a restart and is never committed.
pub fn saved_path(git_dir: &Path) -> PathBuf {
    git_dir.join("seam").join("review.json")
}

pub fn save(git_dir: &Path, review: &Review) {
    let path = saved_path(git_dir);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(review) {
        let _ = std::fs::write(path, json);
    }
}

pub fn load(git_dir: &Path) -> Option<Review> {
    serde_json::from_str(&std::fs::read_to_string(saved_path(git_dir)).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_carries_the_project_overview_when_there_is_one() {
        let scope = Scope {
            label: "main…feature".into(),
            head_sha: None,
            commits: vec!["abc1234 Add a thing".into()],
            diff_text: String::new(),
            diffs: vec![],
        };
        let config = AiConfig::default();
        let with = prompt(&config, &scope, "diff", false, "About this project: P");
        assert!(with.contains("About this project: P\n\nCommits:"));
        let without = prompt(&config, &scope, "diff", false, "");
        assert!(!without.contains("About this project"));
    }

    #[test]
    fn parses_plain_fenced_and_wrapped_answers() {
        let json = r#"{"summary":"Adds a store.","findings":[{"file":"./src/a.ts","line":"12","severity":"RISK","title":"T","body":"B","suggestion":""}]}"#;
        for answer in [
            json.to_string(),
            format!("```json\n{json}\n```"),
            format!("Here you go:\n{json}\nDone."),
        ] {
            let (summary, findings) = parse_answer(&answer).unwrap();
            assert_eq!(summary, "Adds a store.");
            assert_eq!(findings[0].file, "src/a.ts");
            assert_eq!(findings[0].line, Some(12));
            assert_eq!(findings[0].severity, Severity::Risk);
            assert_eq!(findings[0].suggestion, None);
        }
        assert!(parse_answer("no json here").is_err());
    }
}
