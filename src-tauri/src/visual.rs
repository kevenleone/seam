//! Visual recaps of a change and visual plans for one, drawn by the configured AI.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Deserializer, Serialize};

use crate::ai::{self, AiConfig, Kind, Provider};
use crate::git::models::FileDiff;
use crate::git::{parse, Repo, Result};
use crate::review::{self, Scope};

const MAX_PLANS: usize = 20;
const MAX_LISTED_FILES: usize = 1500;
const MAX_DOC_CHARS: usize = 8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisualKind {
    Recap,
    Plan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visual {
    pub id: String,
    pub kind: VisualKind,
    pub provider: Provider,
    pub scope: String,
    pub prompt: Option<String>,
    pub head_sha: Option<String>,
    pub commits: usize,
    pub title: String,
    pub brief: String,
    pub canvas: Canvas,
    pub blocks: Vec<Block>,
    pub diffs: Vec<FileDiff>,
    pub truncated: bool,
    pub created_at: i64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VisualState {
    Idle,
    Running {
        kind: VisualKind,
        provider: Provider,
        scope: String,
        prompt: Option<String>,
        started_at: i64,
    },
    Done {
        visual: Visual,
    },
    Failed {
        kind: VisualKind,
        provider: Provider,
        message: String,
        previous: Option<Visual>,
    },
}

// MARK: Document

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Canvas {
    pub screens: Vec<Screen>,
    pub connectors: Vec<Connector>,
    pub pins: Vec<Pin>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Screen {
    pub id: String,
    pub title: String,
    pub surface: String,
    pub state: Option<String>,
    pub html: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Connector {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pin {
    pub screen: String,
    pub target: String,
    pub number: u32,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Text(TextBlock),
    Diagram(DiagramBlock),
    Compare(CompareBlock),
    Decision(DecisionBlock),
    FileTree(FileTreeBlock),
    Diff(DiffBlock),
    Code(CodeBlock),
    Contract(ContractBlock),
    Verify(VerifyBlock),
    Questions(QuestionsBlock),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextBlock {
    #[serde(alias = "content", alias = "body")]
    pub markdown: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagramBlock {
    pub title: String,
    pub direction: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub kind: String,
    pub group: Option<String>,
    pub change: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub change: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompareBlock {
    pub title: String,
    pub before: Side,
    pub after: Side,
}

/// One half of a comparison: prose, code, or one of the canvas's screens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Side {
    pub text: Option<String>,
    pub code: Option<String>,
    pub language: Option<String>,
    pub screen: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DecisionBlock {
    pub title: String,
    pub context: String,
    pub decision: String,
    pub alternatives: Vec<Alternative>,
    pub consequences: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Alternative {
    pub option: String,
    #[serde(alias = "why_not")]
    pub why_not: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileTreeBlock {
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileEntry {
    pub path: String,
    pub change: String,
    pub additions: u32,
    pub deletions: u32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiffBlock {
    pub file: String,
    pub summary: String,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Annotation {
    #[serde(deserialize_with = "lenient_line")]
    pub line: Option<u32>,
    pub side: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodeBlock {
    pub file: String,
    pub language: String,
    pub code: String,
    pub summary: String,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContractBlock {
    pub title: String,
    pub endpoints: Vec<Endpoint>,
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Endpoint {
    pub method: String,
    pub path: String,
    pub change: String,
    pub summary: String,
    pub request: Option<String>,
    pub response: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entity {
    pub name: String,
    pub change: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub change: String,
    pub was: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VerifyBlock {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Step {
    pub title: String,
    pub how: String,
    pub expect: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QuestionsBlock {
    pub items: Vec<Question>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Question {
    pub question: String,
    pub recommendation: String,
}

fn lenient_line<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u32>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
        .map(|n| n as u32))
}

// MARK: Scope

/// A plan's context: the branch it starts from, and any uncommitted work already there.
pub async fn plan_scope(repo: &Repo) -> Result<Scope> {
    let head_sha = repo.head().await;
    let branch = repo.current_branch().await;
    let diff_text = if head_sha.is_some() {
        repo.run(&["diff", "--no-color", "--no-ext-diff", "-M", "HEAD"])
            .await?
            .text()
    } else {
        String::new()
    };
    Ok(Scope {
        label: branch.unwrap_or_else(|| "HEAD".into()),
        head_sha,
        commits: vec![],
        diffs: parse::diff(&diff_text),
        diff_text,
    })
}

/// What an API model is told about the repository, since it cannot open it.
async fn repository_context(repo: &Repo, root: &Path) -> String {
    let files = repo
        .run(&["ls-files"])
        .await
        .map(|output| output.text())
        .unwrap_or_default();
    let listed: Vec<&str> = files.lines().take(MAX_LISTED_FILES).collect();
    let mut context = format!(
        "Tracked files{}:\n{}\n",
        if files.lines().count() > MAX_LISTED_FILES {
            " (first ones only)"
        } else {
            ""
        },
        listed.join("\n")
    );
    for name in ["AGENTS.md", "CLAUDE.md", "README.md"] {
        if let Ok(text) = std::fs::read_to_string(root.join(name)) {
            let excerpt: String = text.chars().take(MAX_DOC_CHARS).collect();
            context.push_str(&format!("\n{name}:\n{excerpt}\n"));
        }
    }
    context
}

// MARK: Prompt

const SCHEMA: &str = r##"Answer with ONLY one JSON object, no prose before or after it and no code fence:
{
  "title": "Under 70 characters, names the change",
  "brief": "One to three sentences: what changes for the user or the codebase, and why.",
  "canvas": {
    "screens": [
      {
        "id": "settings-before",
        "title": "Settings › General",
        "surface": "desktop | mobile | panel | dialog | popover",
        "state": "before | after | proposed | null",
        "html": "<div class=\"wf-col\">…</div>",
        "note": "One or two sentences under the artboard: what to notice."
      }
    ],
    "connectors": [{ "from": "screen-id", "to": "screen-id", "label": "Step 1" }],
    "pins": [{ "screen": "screen-id", "target": "pin-name", "title": "Short", "body": "What changed here" }]
  },
  "blocks": [
    { "type": "text", "markdown": "Short paragraphs. `code`, **bold** and lists are fine." },
    { "type": "diagram", "title": "…", "direction": "right | down",
      "nodes": [{ "id": "a", "label": "Name", "detail": "one short line", "kind": "ui | module | service | store | external | file", "group": "optional layer name", "change": "added | modified | removed | (empty)" }],
      "edges": [{ "from": "a", "to": "b", "label": "optional verb", "change": "added | removed | (empty)" }] },
    { "type": "compare", "title": "…", "before": { "text": "…" }, "after": { "code": "…", "language": "ts" } },
    { "type": "decision", "title": "…", "context": "…", "decision": "…", "alternatives": [{ "option": "…", "whyNot": "…" }], "consequences": "…" },
    { "type": "fileTree", "files": [{ "path": "src/a.ts", "change": "added | modified | removed | renamed", "note": "what this file now does" }] },
    { "type": "diff", "file": "src/a.ts", "summary": "One line on what to look at.", "annotations": [{ "line": 42, "side": "after | before", "note": "…" }] },
    { "type": "code", "file": "src/new.ts", "language": "ts", "code": "…", "summary": "…", "annotations": [{ "line": 3, "note": "…" }] },
    { "type": "contract", "title": "…", "endpoints": [{ "method": "POST", "path": "/api/x", "change": "added", "summary": "…", "request": "{ \"a\": 1 }", "response": "{ \"ok\": true }" }],
      "entities": [{ "name": "User", "change": "modified", "fields": [{ "name": "email", "type": "string", "change": "added", "was": null }] }] },
    { "type": "verify", "steps": [{ "title": "…", "how": "What to do by hand", "expect": "What should happen" }] },
    { "type": "questions", "items": [{ "question": "…", "recommendation": "The default you would pick, and why" }] }
  ]
}

Each "side" of a compare holds exactly one of "text", "code" (with "language") or "screen" (a screen id).

Wireframes (canvas screens):
- Only draw screens when rendered UI is involved; otherwise leave "screens" empty.
- "html" is static semantic HTML for a low-fidelity sketch: no <script>, <style>, <img>, <iframe>, event handlers or URLs. Inline style="" is allowed only for layout (display, flex, gap, width, height, padding, margin).
- Use these classes: wf-row, wf-col (flex rows and columns), wf-sidebar, wf-nav, wf-card, wf-title, wf-text, wf-muted, wf-line (a grey bar standing in for text), wf-button, wf-button primary, wf-input, wf-select, wf-checkbox, wf-toggle, wf-toggle on, wf-chip, wf-badge, wf-list, wf-item, wf-avatar, wf-image, wf-divider, wf-changed (outlines what this change touches).
- Real labels for what matters, wf-line for filler. Keep each screen under about 60 elements.
- Put data-pin="name" on an element a pin points at; each pin's "target" is that name.
- Show a before/after pair as two screens with "state" set, side by side, joined by a connector."##;

fn language(config: &AiConfig) -> &str {
    let language = config.review.language.trim();
    if language.is_empty() {
        "English"
    } else {
        language
    }
}

fn recap_prompt(
    config: &AiConfig,
    scope: &Scope,
    diff: &str,
    truncated: bool,
    context: &str,
) -> String {
    let commits = if scope.commits.is_empty() {
        "(uncommitted changes, no commits)".to_string()
    } else {
        scope.commits.join("\n")
    };
    let access = match config.provider.kind() {
        Kind::Harness => "Read the changed files and the code around them in the repository to understand the change, but do not modify any file.",
        Kind::Api => "You only have the diff and the repository overview below; mark anything you infer beyond them as inferred.",
    };
    format!(
        r#"Write a visual recap of a code change in this repository ({label}) for a reviewer who has not seen it. {access}

A recap is the map a reviewer reads before the raw diff: what changed and why, how the pieces fit, and how to check it works. Build it from the actual diff; never describe changes the diff does not contain.

Include, in this order, what applies:
1. Canvas screens when rendered UI changed: each changed screen or state as a "before" and "after" pair, reconstructed from the removed and added lines (say so in the note when the before is inferred), with pins on what changed.
2. A "text" block: one to three short paragraphs on the outcome and the reason for it.
3. A "diagram" of how the touched modules, components, services and stores relate, marking added and modified pieces with "change". Skip it for a change inside one file.
4. A "decision" block for each real design decision visible in the change (two at most).
5. "contract" blocks for changed APIs, IPC commands, schemas or data shapes.
6. A "fileTree" block with a one-line note for the files that matter (paths exactly as in the diff).
7. Three to eight "diff" blocks for the key files, each with a one-line summary and a few annotations on the lines a reviewer should read closely. Lines are in the NEW file for side "after", the OLD file for side "before".
8. A "verify" block: concrete steps to test the change by hand, with what should happen.

No intros, disclaimers, file counts or branch names in prose; the title and brief carry that. Never include secrets or tokens. Write all prose in {language}.

{SCHEMA}

Commits:
{commits}
{context}
Diff{trunc}:
{diff}"#,
        label = scope.label,
        language = language(config),
        trunc = if truncated {
            " (truncated: large change)"
        } else {
            ""
        },
    )
}

fn plan_prompt(
    config: &AiConfig,
    scope: &Scope,
    request: &str,
    diff: &str,
    context: &str,
) -> String {
    let access = match config.provider.kind() {
        Kind::Harness => "Explore the repository read-only to ground the plan in its real files, modules, types and conventions. Do not modify any file.",
        Kind::Api => "You cannot open the repository; the overview below is all you have. Mark anything you infer beyond it as inferred.",
    };
    let work = if diff.trim().is_empty() {
        String::new()
    } else {
        format!("\nUncommitted work already in the working tree, which the plan should build on:\n{diff}\n")
    };
    format!(
        r#"Write a visual plan for a change to this repository (on {label}), for someone to approve before any code is written. {access}

The change requested:
{request}

A plan is a proposal a reviewer can approve in a few minutes: the outcome first, then how it will be built. Reuse existing modules, components and conventions before proposing new ones, and decide the hard-to-reverse choices (data shapes, public APIs, storage) explicitly.

Include, in this order, what applies:
1. Canvas screens when the change touches rendered UI: the current screen as "before" (from the code as it is) and the proposed one as "proposed", joined by connectors for multi-step flows, with pins on what changes.
2. A "text" block: the outcome in one to three short paragraphs.
3. A "diagram" of the proposed architecture, marking new and modified pieces with "change".
4. "decision" blocks for the main design decisions, each with the alternatives considered.
5. "contract" blocks for new or changed APIs, commands, schemas or data shapes.
6. A "fileTree" of the files to add, modify or remove, each with a note on what changes there.
7. "code" blocks for the few key pieces of new code (signatures and the core logic, not whole files).
8. A "verify" block: how the finished change will be tested.
9. One "questions" block with the open questions that would change the design, each with a recommended default. Leave it out when there are none.

Do not use "diff" blocks in a plan. No intros or disclaimers. Never include secrets or tokens. Write all prose in {language}.

{SCHEMA}
{context}{work}"#,
        label = scope.label,
        language = language(config),
    )
}

// MARK: Parsing

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawVisual {
    title: String,
    brief: String,
    canvas: Canvas,
    blocks: Vec<serde_json::Value>,
}

fn normalized_change(change: &str) -> String {
    match change.trim().to_lowercase().as_str() {
        "added" | "add" | "new" | "created" => "added",
        "modified" | "changed" | "updated" | "modify" => "modified",
        "removed" | "deleted" | "remove" | "delete" => "removed",
        "renamed" | "moved" => "renamed",
        _ => "",
    }
    .to_string()
}

fn diff_change(diff: &FileDiff) -> &'static str {
    match (&diff.old_path, &diff.new_path) {
        (None, Some(_)) => "added",
        (Some(_), None) => "removed",
        (Some(old), Some(new)) if old != new => "renamed",
        _ => "modified",
    }
}

fn clean_path(path: &str) -> String {
    path.trim().trim_start_matches("./").to_string()
}

fn tidy_canvas(canvas: &mut Canvas) {
    let mut seen = HashSet::new();
    canvas.screens.retain(|screen| {
        !screen.id.trim().is_empty()
            && !screen.html.trim().is_empty()
            && seen.insert(screen.id.clone())
    });
    for screen in &mut canvas.screens {
        screen.state = screen
            .state
            .take()
            .map(|state| state.trim().to_lowercase())
            .filter(|state| matches!(state.as_str(), "before" | "after" | "proposed"));
    }
    canvas
        .connectors
        .retain(|connector| seen.contains(&connector.from) && seen.contains(&connector.to));
    canvas
        .pins
        .retain(|pin| seen.contains(&pin.screen) && !pin.target.trim().is_empty());
    let mut numbers = std::collections::HashMap::new();
    for pin in &mut canvas.pins {
        let next = numbers.entry(pin.screen.clone()).or_insert(0);
        *next += 1;
        pin.number = *next;
    }
}

/// Keeps what the model got right and grounds the rest in the diff: file stats come from
/// git, diff blocks must name a changed file, and references must point at something.
fn tidy_blocks(
    kind: VisualKind,
    blocks: Vec<serde_json::Value>,
    canvas: &Canvas,
    diffs: &[FileDiff],
) -> Vec<Block> {
    let screens: HashSet<&str> = canvas.screens.iter().map(|s| s.id.as_str()).collect();
    let changed: HashSet<&str> = diffs.iter().map(FileDiff::path).collect();
    let mut tidy: Vec<Block> = Vec::new();
    for value in blocks {
        let Ok(block) = serde_json::from_value::<Block>(value) else {
            continue;
        };
        let keep = match block {
            Block::Text(text) => (!text.markdown.trim().is_empty()).then_some(Block::Text(text)),
            Block::Diagram(mut diagram) => {
                let mut ids = HashSet::new();
                diagram
                    .nodes
                    .retain(|node| !node.id.trim().is_empty() && ids.insert(node.id.clone()));
                for node in &mut diagram.nodes {
                    node.change = normalized_change(&node.change);
                    if node.label.trim().is_empty() {
                        node.label = node.id.clone();
                    }
                }
                diagram
                    .edges
                    .retain(|edge| ids.contains(&edge.from) && ids.contains(&edge.to));
                for edge in &mut diagram.edges {
                    edge.change = normalized_change(&edge.change);
                }
                (!diagram.nodes.is_empty()).then_some(Block::Diagram(diagram))
            }
            Block::Compare(mut compare) => {
                for side in [&mut compare.before, &mut compare.after] {
                    side.screen = side
                        .screen
                        .take()
                        .filter(|id| screens.contains(id.as_str()));
                }
                Some(Block::Compare(compare))
            }
            Block::FileTree(mut tree) => {
                for file in &mut tree.files {
                    file.path = clean_path(&file.path);
                    file.change = normalized_change(&file.change);
                }
                Some(Block::FileTree(tree))
            }
            Block::Diff(mut diff) => {
                diff.file = clean_path(&diff.file);
                (kind == VisualKind::Recap && changed.contains(diff.file.as_str()))
                    .then_some(Block::Diff(diff))
            }
            Block::Contract(mut contract) => {
                for endpoint in &mut contract.endpoints {
                    endpoint.change = normalized_change(&endpoint.change);
                    endpoint.method = endpoint.method.trim().to_uppercase();
                }
                for entity in &mut contract.entities {
                    entity.change = normalized_change(&entity.change);
                    for field in &mut entity.fields {
                        field.change = normalized_change(&field.change);
                    }
                }
                (!contract.endpoints.is_empty() || !contract.entities.is_empty())
                    .then_some(Block::Contract(contract))
            }
            Block::Questions(questions) => (kind == VisualKind::Plan
                && !questions.items.is_empty())
            .then_some(Block::Questions(questions)),
            other => Some(other),
        };
        tidy.extend(keep);
    }
    if kind == VisualKind::Recap && !diffs.is_empty() {
        ground_file_tree(&mut tidy, diffs);
    }
    tidy
}

/// A recap's file tree lists every changed file with git's own numbers, and the model's notes.
fn ground_file_tree(blocks: &mut Vec<Block>, diffs: &[FileDiff]) {
    let position = blocks
        .iter()
        .position(|block| matches!(block, Block::FileTree(_)));
    let notes: Vec<FileEntry> = blocks
        .iter()
        .filter_map(|block| match block {
            Block::FileTree(tree) => Some(tree.files.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    blocks.retain(|block| !matches!(block, Block::FileTree(_)));
    let files = diffs
        .iter()
        .map(|diff| FileEntry {
            path: diff.path().to_string(),
            change: diff_change(diff).to_string(),
            additions: diff.additions,
            deletions: diff.deletions,
            note: notes
                .iter()
                .find(|entry| entry.path == diff.path())
                .and_then(|entry| entry.note.clone())
                .filter(|note| !note.trim().is_empty()),
        })
        .collect();
    let at = position
        .or_else(|| {
            blocks
                .iter()
                .position(|block| matches!(block, Block::Diff(_)))
        })
        .unwrap_or(blocks.len())
        .min(blocks.len());
    blocks.insert(at, Block::FileTree(FileTreeBlock { files }));
}

fn parse_answer(
    kind: VisualKind,
    answer: &str,
    diffs: &[FileDiff],
) -> Result<(String, String, Canvas, Vec<Block>)> {
    let raw: RawVisual = ai::json_object(answer).ok_or_else(|| ai::not_json(answer))?;
    let mut canvas = raw.canvas;
    tidy_canvas(&mut canvas);
    let blocks = tidy_blocks(kind, raw.blocks, &canvas, diffs);
    if blocks.is_empty() && canvas.screens.is_empty() {
        return Err(ai::not_json(answer));
    }
    Ok((
        raw.title.trim().to_string(),
        raw.brief.trim().to_string(),
        canvas,
        blocks,
    ))
}

/// Why the JSON object in an answer doesn't parse, when the answer holds one at all.
fn json_problem(answer: &str) -> Option<String> {
    let cleaned = ai::strip_ansi(answer);
    let (start, end) = (cleaned.find('{')?, cleaned.rfind('}')?);
    if end <= start {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(&cleaned[start..=end])
        .err()
        .map(|error| error.to_string())
}

fn repair_prompt(answer: &str, problem: &str) -> String {
    format!(
        "This JSON object does not parse ({problem}). Return the same object, corrected to valid JSON: escape quotes and newlines inside strings, and change nothing else. Answer with ONLY the JSON object, no prose and no code fence. Do not read or change any file.\n\n{answer}"
    )
}

// MARK: Running

pub async fn run(
    config: &AiConfig,
    root: &Path,
    kind: VisualKind,
    scope: Scope,
    request: Option<String>,
) -> Result<Visual> {
    let started = Instant::now();
    let repo = Repo::new(root);
    let context = match config.provider.kind() {
        Kind::Api => format!(
            "\nRepository overview:\n{}",
            repository_context(&repo, root).await
        ),
        Kind::Harness => String::new(),
    };
    let (diff, truncated) = review::clip(&scope.diff_text);
    let prompt = match kind {
        VisualKind::Recap => recap_prompt(config, &scope, diff, truncated, &context),
        VisualKind::Plan => plan_prompt(
            config,
            &scope,
            request.as_deref().unwrap_or_default(),
            diff,
            &context,
        ),
    };
    let answer = ai::ask(config, root, &prompt).await?;
    let grounding: &[FileDiff] = match kind {
        VisualKind::Recap => &scope.diffs,
        VisualKind::Plan => &[],
    };
    // Long answers full of HTML sometimes come back with one bad escape; one repair round
    // costs far less than starting over.
    let (title, brief, canvas, blocks) = match parse_answer(kind, &answer, grounding) {
        Ok(parsed) => parsed,
        Err(first) => {
            let Some(problem) = json_problem(&answer) else {
                return Err(first);
            };
            let repaired = ai::ask(config, root, &repair_prompt(&answer, &problem)).await?;
            parse_answer(kind, &repaired, grounding)?
        }
    };
    let fallback = match kind {
        VisualKind::Recap => format!("Recap of {}", scope.label),
        VisualKind::Plan => request
            .as_deref()
            .unwrap_or("Plan")
            .lines()
            .next()
            .unwrap_or("Plan")
            .chars()
            .take(70)
            .collect(),
    };
    Ok(Visual {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        provider: config.provider,
        scope: scope.label,
        prompt: request,
        head_sha: scope.head_sha,
        commits: scope.commits.len(),
        title: if title.is_empty() { fallback } else { title },
        brief,
        canvas,
        blocks,
        diffs: match kind {
            VisualKind::Recap => scope.diffs,
            VisualKind::Plan => vec![],
        },
        truncated,
        created_at: review::now_secs(),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

// MARK: Saving

fn recap_path(git_dir: &Path) -> PathBuf {
    git_dir.join("seam").join("recap.json")
}

fn plans_dir(git_dir: &Path) -> PathBuf {
    git_dir.join("seam").join("plans")
}

fn plan_path(git_dir: &Path, id: &str) -> Option<PathBuf> {
    let safe = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    safe.then(|| plans_dir(git_dir).join(format!("{id}.json")))
}

fn write_json(path: &Path, visual: &Visual) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(visual) {
        let _ = std::fs::write(path, json);
    }
}

fn read_json(path: &Path) -> Option<Visual> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// The last recap and the last plans are kept in the git directory, never committed.
pub fn save(git_dir: &Path, visual: &Visual) {
    match visual.kind {
        VisualKind::Recap => write_json(&recap_path(git_dir), visual),
        VisualKind::Plan => {
            if let Some(path) = plan_path(git_dir, &visual.id) {
                write_json(&path, visual);
            }
            for old in plans(git_dir).iter().skip(MAX_PLANS) {
                if let Some(path) = plan_path(git_dir, &old.id) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

pub fn load_recap(git_dir: &Path) -> Option<Visual> {
    read_json(&recap_path(git_dir))
}

pub fn load_plan(git_dir: &Path, id: &str) -> Option<Visual> {
    read_json(&plan_path(git_dir, id)?)
}

/// Saved plans, newest first.
pub fn plans(git_dir: &Path) -> Vec<PlanSummary> {
    let Ok(entries) = std::fs::read_dir(plans_dir(git_dir)) else {
        return vec![];
    };
    let mut list: Vec<PlanSummary> = entries
        .flatten()
        .filter_map(|entry| read_json(&entry.path()))
        .map(|visual| PlanSummary {
            id: visual.id,
            title: visual.title,
            prompt: visual.prompt.unwrap_or_default(),
            created_at: visual.created_at,
        })
        .collect();
    list.sort_by_key(|plan| std::cmp::Reverse(plan.created_at));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/src/a.ts b/src/a.ts\nindex 1..2 100644\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1,2 @@\n-old\n+new\n+more\ndiff --git a/src/b.ts b/src/b.ts\nnew file mode 100644\nindex 0..3\n--- /dev/null\n+++ b/src/b.ts\n@@ -0,0 +1 @@\n+b\n";

    #[test]
    fn parses_plain_fenced_and_wrapped_answers_and_skips_unknown_blocks() {
        let json = r#"{"title":"T","brief":"B","blocks":[{"type":"text","markdown":"Hi"},{"type":"hologram"},{"type":"verify","steps":[{"title":"Open it"}]}]}"#;
        for answer in [
            json.to_string(),
            format!("```json\n{json}\n```"),
            format!("Here it is:\n{json}\nDone."),
        ] {
            let (title, brief, _, blocks) = parse_answer(VisualKind::Plan, &answer, &[]).unwrap();
            assert_eq!((title.as_str(), brief.as_str()), ("T", "B"));
            assert_eq!(blocks.len(), 2);
            assert!(matches!(&blocks[1], Block::Verify(verify) if verify.steps[0].how.is_empty()));
        }
        assert!(parse_answer(VisualKind::Plan, "no json here", &[]).is_err());
        assert!(parse_answer(VisualKind::Plan, r#"{"title":"Empty"}"#, &[]).is_err());
    }

    #[test]
    fn a_recap_is_grounded_in_the_diff() {
        let diffs = parse::diff(DIFF);
        let answer = r#"{"title":"T","blocks":[
            {"type":"fileTree","files":[{"path":"./src/a.ts","change":"added","additions":99,"note":"Now says new"}]},
            {"type":"diff","file":"./src/a.ts","summary":"S","annotations":[{"line":"2","note":"N"}]},
            {"type":"diff","file":"src/invented.ts","summary":"S"},
            {"type":"questions","items":[{"question":"Q"}]}
        ]}"#;
        let (_, _, _, blocks) = parse_answer(VisualKind::Recap, answer, &diffs).unwrap();
        assert_eq!(blocks.len(), 2);
        let Block::FileTree(tree) = &blocks[0] else {
            panic!("the file tree comes first");
        };
        let files: Vec<(&str, &str, u32, Option<&str>)> = tree
            .files
            .iter()
            .map(|f| {
                (
                    f.path.as_str(),
                    f.change.as_str(),
                    f.additions,
                    f.note.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            files,
            [
                ("src/a.ts", "modified", 2, Some("Now says new")),
                ("src/b.ts", "added", 1, None)
            ]
        );
        let Block::Diff(diff) = &blocks[1] else {
            panic!("the real diff block stays");
        };
        assert_eq!(
            (diff.file.as_str(), diff.annotations[0].line),
            ("src/a.ts", Some(2))
        );
    }

    #[test]
    fn canvas_references_must_point_at_something() {
        let answer = r#"{"canvas":{
            "screens":[{"id":"a","html":"<div></div>","state":"Before"},{"id":"a","html":"<p></p>"},{"id":"b","html":"<p></p>","state":"sideways"},{"id":"c","html":""}],
            "connectors":[{"from":"a","to":"b"},{"from":"a","to":"c"}],
            "pins":[{"screen":"a","target":"x"},{"screen":"b","target":"y"},{"screen":"a","target":"z"},{"screen":"c","target":"w"}]
        },"blocks":[{"type":"compare","before":{"screen":"a"},"after":{"screen":"gone"}},
                   {"type":"diagram","nodes":[{"id":"n","change":"New"}],"edges":[{"from":"n","to":"m"}]}]}"#;
        let (_, _, canvas, blocks) = parse_answer(VisualKind::Plan, answer, &[]).unwrap();
        let ids: Vec<&str> = canvas.screens.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(canvas.screens[0].state.as_deref(), Some("before"));
        assert_eq!(canvas.screens[1].state, None);
        assert_eq!(canvas.connectors.len(), 1);
        let pins: Vec<(&str, u32)> = canvas
            .pins
            .iter()
            .map(|p| (p.screen.as_str(), p.number))
            .collect();
        assert_eq!(pins, [("a", 1), ("b", 1), ("a", 2)]);
        let Block::Compare(compare) = &blocks[0] else {
            panic!("compare first");
        };
        assert_eq!(compare.before.screen.as_deref(), Some("a"));
        assert_eq!(compare.after.screen, None);
        let Block::Diagram(diagram) = &blocks[1] else {
            panic!("then the diagram");
        };
        assert_eq!(
            (
                diagram.nodes[0].label.as_str(),
                diagram.nodes[0].change.as_str()
            ),
            ("n", "added")
        );
        assert!(diagram.edges.is_empty());
    }

    #[test]
    fn only_a_broken_object_is_worth_repairing() {
        assert!(json_problem(r#"{"title":"say "hi""}"#).is_some());
        assert!(json_problem(r#"{"title":"fine"}"#).is_none());
        assert!(json_problem("no object at all").is_none());
    }

    #[test]
    fn plan_ids_cannot_leave_the_plans_folder() {
        let dir = Path::new("/repo/.git");
        assert!(plan_path(dir, "../../etc/passwd").is_none());
        assert!(plan_path(dir, "").is_none());
        assert_eq!(
            plan_path(dir, "1b2c-3d").unwrap(),
            dir.join("seam/plans/1b2c-3d.json")
        );
    }
}
