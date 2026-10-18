//! The Overview tab: what a repository is, read from git and its manifests, and an
//! architecture overview the configured AI writes on top, refreshed section by section.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::ai::{self, AiConfig, Kind, Provider};
use crate::git::{Repo, Result};
use crate::review;
use crate::visual::{self, Block};

const MAX_SAVED: usize = 20;
const WEEKS: usize = 12;
const WEEK_SECS: i64 = 7 * 24 * 60 * 60;
const MAX_HOT_PATHS: usize = 6;
const MAX_SCRIPTS: usize = 10;
const MAX_LANGUAGES: usize = 6;
const MAX_DESCRIPTION_CHARS: usize = 400;
const MAX_CHANGED_LISTED: usize = 300;
const MAX_CONTEXT_CHARS: usize = 6000;
/// The sections other AI features are told about; "start" is for people opening the project.
const CONTEXT_SECTIONS: &[&str] = &["architecture", "modules"];
/// How much a section's paths must change before it is out of date: each changed line counts
/// one, each added or deleted file `FILE_WEIGHT`.
const STALE_SCORE: u32 = 300;
const FILE_WEIGHT: u32 = 100;
/// The sections a new top-level folder of code makes out of date, since they map the layout.
const LAYOUT_SECTIONS: &[&str] = &["architecture", "modules"];

/// The sections an overview holds, in the order they are shown.
pub const SECTIONS: [(&str, &str); 3] = [
    ("architecture", "Architecture"),
    ("modules", "What lives where"),
    ("start", "Where to start"),
];

// MARK: Facts

/// What Seam reads from git and the manifests alone: instant, and free.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    pub name: String,
    pub description: Option<Description>,
    pub stack: Vec<Tech>,
    pub languages: Vec<Language>,
    pub commits: usize,
    pub contributors: usize,
    pub latest_tag: Option<String>,
    /// Commits per week, oldest first, the last one being this week.
    pub weekly: Vec<usize>,
    pub hot_paths: Vec<HotPath>,
    pub scripts: Vec<Script>,
    pub head_sha: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    pub text: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tech {
    pub name: String,
    pub version: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    pub name: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotPath {
    pub path: String,
    /// File changes under the path in the last 30 days.
    pub changes: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    pub name: String,
    pub command: String,
    /// How to run it, e.g. `pnpm app:dev` or `make test`.
    pub run: String,
}

const LANGUAGES: &[(&str, &str)] = &[
    ("ts", "TypeScript"),
    ("tsx", "TypeScript"),
    ("mts", "TypeScript"),
    ("js", "JavaScript"),
    ("jsx", "JavaScript"),
    ("mjs", "JavaScript"),
    ("cjs", "JavaScript"),
    ("rs", "Rust"),
    ("go", "Go"),
    ("py", "Python"),
    ("rb", "Ruby"),
    ("java", "Java"),
    ("kt", "Kotlin"),
    ("kts", "Kotlin"),
    ("swift", "Swift"),
    ("m", "Objective-C"),
    ("mm", "Objective-C"),
    ("c", "C"),
    ("h", "C"),
    ("cc", "C++"),
    ("cpp", "C++"),
    ("hpp", "C++"),
    ("cs", "C#"),
    ("php", "PHP"),
    ("scala", "Scala"),
    ("dart", "Dart"),
    ("ex", "Elixir"),
    ("exs", "Elixir"),
    ("erl", "Erlang"),
    ("hs", "Haskell"),
    ("lua", "Lua"),
    ("sh", "Shell"),
    ("bash", "Shell"),
    ("zsh", "Shell"),
    ("css", "CSS"),
    ("scss", "SCSS"),
    ("less", "Less"),
    ("html", "HTML"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("astro", "Astro"),
    ("sql", "SQL"),
    ("zig", "Zig"),
    ("clj", "Clojure"),
    ("ml", "OCaml"),
    ("r", "R"),
    ("jl", "Julia"),
    ("tf", "HCL"),
    ("proto", "Protocol Buffers"),
    ("graphql", "GraphQL"),
    ("gd", "GDScript"),
    ("nim", "Nim"),
];

/// Dependencies worth naming, by package name. A name with a dot is a Go module path,
/// matched as a prefix so `/v2` and the like still count.
const KNOWN: &[(&str, &str)] = &[
    ("react", "React"),
    ("next", "Next.js"),
    ("vue", "Vue"),
    ("nuxt", "Nuxt"),
    ("svelte", "Svelte"),
    ("@sveltejs/kit", "SvelteKit"),
    ("@angular/core", "Angular"),
    ("solid-js", "Solid"),
    ("astro", "Astro"),
    ("@remix-run/react", "Remix"),
    ("react-native", "React Native"),
    ("expo", "Expo"),
    ("electron", "Electron"),
    ("@tauri-apps/api", "Tauri"),
    ("express", "Express"),
    ("fastify", "Fastify"),
    ("@nestjs/core", "NestJS"),
    ("hono", "Hono"),
    ("koa", "Koa"),
    ("vite", "Vite"),
    ("webpack", "webpack"),
    ("tailwindcss", "Tailwind CSS"),
    ("typescript", "TypeScript"),
    ("zustand", "Zustand"),
    ("redux", "Redux"),
    ("@reduxjs/toolkit", "Redux"),
    ("@tanstack/react-query", "TanStack Query"),
    ("prisma", "Prisma"),
    ("drizzle-orm", "Drizzle"),
    ("mongoose", "Mongoose"),
    ("graphql", "GraphQL"),
    ("jest", "Jest"),
    ("vitest", "Vitest"),
    ("@playwright/test", "Playwright"),
    ("cypress", "Cypress"),
    ("storybook", "Storybook"),
    ("three", "three.js"),
    ("d3", "D3"),
    ("tauri", "Tauri"),
    ("tokio", "Tokio"),
    ("axum", "Axum"),
    ("actix-web", "Actix Web"),
    ("rocket", "Rocket"),
    ("serde", "Serde"),
    ("reqwest", "reqwest"),
    ("sqlx", "SQLx"),
    ("diesel", "Diesel"),
    ("clap", "clap"),
    ("bevy", "Bevy"),
    ("leptos", "Leptos"),
    ("yew", "Yew"),
    ("wasm-bindgen", "wasm-bindgen"),
    ("django", "Django"),
    ("flask", "Flask"),
    ("fastapi", "FastAPI"),
    ("pytest", "pytest"),
    ("numpy", "NumPy"),
    ("pandas", "pandas"),
    ("torch", "PyTorch"),
    ("tensorflow", "TensorFlow"),
    ("sqlalchemy", "SQLAlchemy"),
    ("pydantic", "Pydantic"),
    ("github.com/gin-gonic/gin", "Gin"),
    ("github.com/labstack/echo", "Echo"),
    ("github.com/gofiber/fiber", "Fiber"),
    ("github.com/spf13/cobra", "Cobra"),
    ("gorm.io/gorm", "GORM"),
    ("rails", "Rails"),
    ("sinatra", "Sinatra"),
    ("rspec", "RSpec"),
    ("phoenix", "Phoenix"),
    ("laravel/framework", "Laravel"),
    ("symfony/framework-bundle", "Symfony"),
    ("spring-boot-starter", "Spring Boot"),
    ("flutter", "Flutter"),
    ("@vitejs/plugin-react", "Vite"),
    ("@builder.io/qwik", "Qwik"),
    ("htmx.org", "htmx"),
    ("alpinejs", "Alpine.js"),
];

/// Files whose presence alone names a tool, checked at the top level.
const TOOLS: &[(&str, &str)] = &[
    ("pnpm-lock.yaml", "pnpm"),
    ("yarn.lock", "Yarn"),
    ("bun.lock", "Bun"),
    ("bun.lockb", "Bun"),
    ("package-lock.json", "npm"),
    ("Dockerfile", "Docker"),
    ("docker-compose.yml", "Docker Compose"),
    ("compose.yaml", "Docker Compose"),
    (".gitlab-ci.yml", "GitLab CI"),
    ("flake.nix", "Nix"),
];

const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "requirements.txt",
    "Gemfile",
    "composer.json",
    "pubspec.yaml",
];

/// Folders whose manifests describe something other than the project itself.
const NOT_THE_PROJECT: &[&str] = &[
    "node_modules",
    "vendor",
    "fixtures",
    "examples",
    "test",
    "tests",
    "testdata",
    "third_party",
];

fn language_of(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next()?;
    let extension = name.rsplit_once('.')?.1.to_lowercase();
    LANGUAGES
        .iter()
        .find(|(known, _)| *known == extension)
        .map(|(_, language)| *language)
}

fn known(name: &str) -> Option<&'static str> {
    let name = name.to_lowercase();
    KNOWN
        .iter()
        .find(|(dep, _)| *dep == name || (dep.contains('.') && name.starts_with(dep)))
        .map(|(_, label)| *label)
}

/// `^19.3.1` → `19.3`; anything that isn't a version (a path, `workspace:*`) → none.
fn short_version(version: &str) -> Option<String> {
    let cleaned = version
        .trim()
        .trim_start_matches(|c: char| "^~>=<v ".contains(c));
    if !cleaned.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<&str> = cleaned
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .next()
        .unwrap_or_default()
        .split('.')
        .filter(|part| !part.is_empty())
        .take(2)
        .collect();
    Some(parts.join("."))
}

/// The dependency a manifest line starts with, for line-based formats: requirements,
/// pyproject, Gemfile, go.mod and the dependency tables of Cargo.toml.
fn line_dependency(line: &str) -> Option<(&'static str, Option<String>)> {
    let mut rest = line.trim();
    for prefix in ["require ", "gem "] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped.trim_start();
        }
    }
    let rest = rest.trim_start_matches(['"', '\'']);
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || "-_./@".contains(*c))
        .collect();
    let label = known(&name)?;
    let version = rest[name.len()..]
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
        .find(|token| {
            token
                .trim_start_matches('v')
                .starts_with(|c: char| c.is_ascii_digit())
        })
        .and_then(short_version);
    Some((label, version))
}

fn package_json_dependencies(text: &str) -> Vec<(&'static str, Option<String>)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return vec![];
    };
    ["dependencies", "devDependencies", "peerDependencies"]
        .iter()
        .filter_map(|table| json.get(table)?.as_object())
        .flatten()
        .filter_map(|(name, version)| {
            Some((known(name)?, version.as_str().and_then(short_version)))
        })
        .collect()
}

fn cargo_dependencies(text: &str) -> Vec<(&'static str, Option<String>)> {
    let mut in_dependencies = false;
    let mut found = vec![];
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed.trim_end_matches(']').ends_with("dependencies");
            continue;
        }
        if in_dependencies {
            found.extend(line_dependency(trimmed));
        }
    }
    found
}

fn dependencies(manifest: &str, text: &str) -> Vec<(&'static str, Option<String>)> {
    match manifest.rsplit('/').next().unwrap_or(manifest) {
        "package.json" | "composer.json" => package_json_dependencies(text)
            .into_iter()
            .chain(
                serde_json::from_str::<serde_json::Value>(text)
                    .ok()
                    .and_then(|json| json.get("require")?.as_object().cloned())
                    .into_iter()
                    .flatten()
                    .filter_map(|(name, version)| {
                        Some((known(&name)?, version.as_str().and_then(short_version)))
                    }),
            )
            .collect(),
        "Cargo.toml" => cargo_dependencies(text),
        _ => text.lines().filter_map(line_dependency).collect(),
    }
}

/// The value of `key = "…"` in a TOML table, e.g. Cargo's `[package] description`.
fn toml_string(text: &str, table: &str, key: &str) -> Option<String> {
    let mut in_table = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_table = trimmed == format!("[{table}]");
            continue;
        }
        if !in_table {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        if name.trim() == key {
            let value = value.trim().trim_matches('"').trim();
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

/// The README's first paragraph of prose, with headings, badges and markup left out.
fn readme_paragraph(text: &str) -> Option<String> {
    let mut paragraph: Vec<&str> = vec![];
    let mut fenced = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        let prose = !fenced
            && !trimmed.is_empty()
            && !trimmed.starts_with(['#', '<', '!', '|', '>', '-', '*', '='])
            && !trimmed.starts_with("[![")
            && !trimmed.chars().all(|c| "-=_*".contains(c));
        if prose {
            paragraph.push(trimmed);
        } else if !paragraph.is_empty() {
            break;
        }
    }
    let joined = strip_markdown(&paragraph.join(" "));
    if joined.is_empty() {
        return None;
    }
    if joined.chars().count() <= MAX_DESCRIPTION_CHARS {
        return Some(joined);
    }
    let cut: String = joined.chars().take(MAX_DESCRIPTION_CHARS).collect();
    Some(format!("{}…", cut.trim_end()))
}

/// `[text](url)` → `text`, and emphasis markers dropped; code spans stay.
fn strip_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|at| open + at) else {
            break;
        };
        let Some(end) = rest[close..].find(')').map(|at| close + at) else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out.replace("**", "").replace("__", "")
}

fn is_project_path(path: &str) -> bool {
    !path.split('/').any(|part| NOT_THE_PROJECT.contains(&part))
}

/// Manifests at most two folders deep, the top level first.
fn manifests(files: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = files
        .iter()
        .filter(|path| path.matches('/').count() <= 2 && is_project_path(path))
        .filter(|path| MANIFESTS.contains(&path.rsplit('/').next().unwrap_or(path)))
        .map(|path| path.to_string())
        .collect();
    found.sort_by_key(|path| (path.matches('/').count(), path.clone()));
    found
}

fn read(root: &Path, path: &str) -> Option<String> {
    std::fs::read_to_string(root.join(path)).ok()
}

fn stack(root: &Path, files: &[&str]) -> Vec<Tech> {
    let mut seen = HashSet::new();
    let mut stack = vec![];
    for manifest in manifests(files) {
        let Some(text) = read(root, &manifest) else {
            continue;
        };
        for (name, version) in dependencies(&manifest, &text) {
            if seen.insert(name) {
                stack.push(Tech {
                    name: name.to_string(),
                    version,
                    source: manifest.clone(),
                });
            }
        }
    }
    let workflows = files
        .iter()
        .any(|path| path.starts_with(".github/workflows/"));
    let tools = TOOLS
        .iter()
        .filter(|(file, _)| files.contains(file))
        .map(|(file, name)| (*name, file.to_string()))
        .chain(workflows.then(|| ("GitHub Actions", ".github/workflows/".to_string())));
    for (name, source) in tools {
        if seen.insert(name) {
            stack.push(Tech {
                name: name.to_string(),
                version: None,
                source,
            });
        }
    }
    stack
}

fn description(root: &Path, files: &[&str]) -> Option<Description> {
    let readme = files
        .iter()
        .filter(|path| !path.contains('/'))
        .find(|path| path.to_lowercase().starts_with("readme"));
    if let Some(text) = readme
        .and_then(|path| read(root, path))
        .as_deref()
        .and_then(readme_paragraph)
    {
        return Some(Description {
            text,
            source: readme.unwrap_or(&"README").to_string(),
        });
    }
    let from_package = read(root, "package.json")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get("description")?.as_str().map(String::from))
        .map(|text| ("package.json", text));
    let from_cargo = || {
        read(root, "Cargo.toml")
            .and_then(|text| toml_string(&text, "package", "description"))
            .map(|text| ("Cargo.toml", text))
    };
    let from_pyproject = || {
        read(root, "pyproject.toml")
            .and_then(|text| toml_string(&text, "project", "description"))
            .map(|text| ("pyproject.toml", text))
    };
    from_package
        .or_else(from_cargo)
        .or_else(from_pyproject)
        .filter(|(_, text)| !text.trim().is_empty())
        .map(|(source, text)| Description {
            text,
            source: source.to_string(),
        })
}

fn package_runner(files: &[&str]) -> &'static str {
    if files.contains(&"pnpm-lock.yaml") {
        "pnpm"
    } else if files.contains(&"yarn.lock") {
        "yarn"
    } else if files.contains(&"bun.lock") || files.contains(&"bun.lockb") {
        "bun"
    } else {
        "npm run"
    }
}

fn scripts(root: &Path, files: &[&str]) -> Vec<Script> {
    let runner = package_runner(files);
    let from_package = read(root, "package.json")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get("scripts")?.as_object().cloned())
        .into_iter()
        .flatten()
        .filter_map(|(name, command)| {
            Some(Script {
                run: format!("{runner} {name}"),
                command: command.as_str()?.to_string(),
                name,
            })
        });
    let from_make = files
        .iter()
        .find(|path| **path == "Makefile" || **path == "makefile")
        .and_then(|path| read(root, path))
        .map(|text| make_targets(&text))
        .unwrap_or_default()
        .into_iter()
        .map(|name| Script {
            run: format!("make {name}"),
            command: String::new(),
            name,
        });
    from_package.chain(from_make).take(MAX_SCRIPTS).collect()
}

fn make_targets(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with(['\t', ' ', '.', '#']))
        .filter_map(|line| line.split_once(':'))
        .filter(|(_, rest)| !rest.starts_with('='))
        .map(|(name, _)| name.trim())
        .filter(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .map(String::from)
        .collect()
}

/// Tracked bytes per language, from the blob sizes git already knows.
fn languages(tree: &str) -> Vec<Language> {
    let mut bytes: HashMap<&str, u64> = HashMap::new();
    for line in tree.lines() {
        let Some((meta, path)) = line.split_once('\t') else {
            continue;
        };
        let size = meta
            .split_whitespace()
            .nth(3)
            .and_then(|size| size.parse::<u64>().ok());
        if let (Some(size), Some(language), true) = (size, language_of(path), is_project_path(path))
        {
            *bytes.entry(language).or_default() += size;
        }
    }
    let mut list: Vec<Language> = bytes
        .into_iter()
        .map(|(name, bytes)| Language {
            name: name.to_string(),
            bytes,
        })
        .collect();
    list.sort_by(|first, second| {
        second
            .bytes
            .cmp(&first.bytes)
            .then(first.name.cmp(&second.name))
    });
    if list.len() > MAX_LANGUAGES {
        let other: u64 = list[MAX_LANGUAGES..].iter().map(|each| each.bytes).sum();
        list.truncate(MAX_LANGUAGES);
        list.push(Language {
            name: "Other".into(),
            bytes: other,
        });
    }
    list
}

/// The folder a changed file is counted under: its first two levels.
fn hot_key(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.len() {
        1 => path.to_string(),
        2 => format!("{}/", parts[0]),
        _ => format!("{}/{}/", parts[0], parts[1]),
    }
}

fn hot_paths(names: &str) -> Vec<HotPath> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for path in names.lines().filter(|line| !line.trim().is_empty()) {
        *counts.entry(hot_key(path.trim())).or_default() += 1;
    }
    let mut list: Vec<HotPath> = counts
        .into_iter()
        .map(|(path, changes)| HotPath { path, changes })
        .collect();
    list.sort_by(|first, second| {
        second
            .changes
            .cmp(&first.changes)
            .then(first.path.cmp(&second.path))
    });
    list.truncate(MAX_HOT_PATHS);
    list
}

fn weekly(timestamps: &str, now: i64) -> Vec<usize> {
    let mut weeks = vec![0; WEEKS];
    for stamp in timestamps
        .lines()
        .filter_map(|line| line.trim().parse::<i64>().ok())
    {
        let age = ((now - stamp).max(0) / WEEK_SECS) as usize;
        if age < WEEKS {
            weeks[WEEKS - 1 - age] += 1;
        }
    }
    weeks
}

async fn text(repo: &Repo, args: &[&str]) -> String {
    repo.run(args)
        .await
        .map(|output| output.text())
        .unwrap_or_default()
}

pub async fn facts(repo: &Repo) -> Result<Facts> {
    let root = repo.root.clone();
    let listed = repo.run(&["ls-files"]).await?.text();
    let files: Vec<&str> = listed.lines().collect();
    let head_sha = repo.head().await;
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut facts = Facts {
        name,
        description: description(&root, &files),
        stack: stack(&root, &files),
        scripts: scripts(&root, &files),
        weekly: vec![0; WEEKS],
        head_sha: head_sha.clone(),
        ..Facts::default()
    };
    if head_sha.is_none() {
        return Ok(facts);
    }
    let since = format!("--since={WEEKS} weeks ago");
    let tree_args = ["ls-tree", "-r", "-l", "--full-tree", "HEAD"];
    let count_args = ["rev-list", "--count", "HEAD"];
    let author_args = ["shortlog", "-sn", "HEAD"];
    let tag_args = ["describe", "--tags", "--abbrev=0"];
    let stamp_args = ["log", since.as_str(), "--format=%ct", "HEAD"];
    let name_args = [
        "log",
        "--since=30 days ago",
        "--name-only",
        "--format=",
        "HEAD",
    ];
    let (tree, count, authors, tag, stamps, names) = tokio::join!(
        text(repo, &tree_args),
        text(repo, &count_args),
        text(repo, &author_args),
        text(repo, &tag_args),
        text(repo, &stamp_args),
        text(repo, &name_args),
    );
    facts.languages = languages(&tree);
    facts.commits = count.trim().parse().unwrap_or(0);
    facts.contributors = authors
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    facts.latest_tag = Some(tag.trim().to_string()).filter(|tag| !tag.is_empty());
    facts.weekly = weekly(&stamps, review::now_secs());
    facts.hot_paths = hot_paths(&names);
    Ok(facts)
}

// MARK: Overview

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub id: String,
    pub provider: Provider,
    /// The commit it was written at; drift is measured from here.
    pub head_sha: Option<String>,
    pub branch: Option<String>,
    pub purpose: String,
    pub sections: Vec<Section>,
    pub created_at: i64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub title: String,
    /// The folders and files it describes; a change under one makes it out of date.
    pub paths: Vec<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewSummary {
    pub id: String,
    pub purpose: String,
    pub head_sha: Option<String>,
    pub branch: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OverviewState {
    Idle,
    Running {
        provider: Provider,
        started_at: i64,
        /// The sections being rewritten; empty when the whole overview is.
        sections: Vec<String>,
        previous: Option<Overview>,
    },
    Done {
        overview: Overview,
    },
    Failed {
        provider: Provider,
        message: String,
        previous: Option<Overview>,
    },
}

/// How far HEAD has moved from the commit an overview was written at.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    pub commits: usize,
    pub files: usize,
    /// Sections the new commits changed enough to be out of date.
    pub sections: Vec<String>,
    /// Sections with only small edits under their paths, still worth reading as they are.
    pub minor: Vec<String>,
    /// The commit it was written at is gone (garbage-collected, or never fetched here).
    pub missing: bool,
}

/// Every tracked file and every folder holding one, folders ending in `/`.
pub struct Tracked {
    entries: HashSet<String>,
}

impl Tracked {
    pub fn new(listed: &str) -> Self {
        let mut entries = HashSet::new();
        for file in listed.lines().filter(|line| !line.is_empty()) {
            let mut at = 0;
            while let Some(slash) = file[at..].find('/') {
                at += slash + 1;
                entries.insert(file[..at].to_string());
            }
            entries.insert(file.to_string());
        }
        Tracked { entries }
    }

    /// The path as tracked (a folder gains its `/`), or none when nothing is there.
    fn resolve(&self, path: &str) -> Option<String> {
        let cleaned = path.trim().trim_start_matches("./").to_string();
        if cleaned.is_empty() {
            return None;
        }
        if self.entries.contains(&cleaned) {
            return Some(cleaned);
        }
        let folder = format!("{}/", cleaned.trim_end_matches('/'));
        self.entries.contains(&folder).then_some(folder)
    }
}

fn under(file: &str, path: &str) -> bool {
    if path.ends_with('/') {
        file.starts_with(path)
    } else {
        file == path
    }
}

/// One file changed since an overview was written.
struct Change<'a> {
    path: &'a str,
    /// `A`, `D`, `M` or `T`, as `git diff --name-status --no-renames` prints it.
    status: char,
    lines: u32,
}

fn changes<'a>(name_status: &'a str, numstat: &'a str) -> Vec<Change<'a>> {
    let lines: HashMap<&str, u32> = numstat
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let added = parts.next()?.parse::<u32>().unwrap_or(0);
            let deleted = parts.next()?.parse::<u32>().unwrap_or(0);
            Some((parts.next()?, added + deleted))
        })
        .collect();
    name_status
        .lines()
        .filter_map(|line| {
            let (status, path) = line.split_once('\t')?;
            Some(Change {
                path,
                status: status.chars().next()?,
                lines: lines.get(path).copied().unwrap_or(0),
            })
        })
        .collect()
}

/// The sections out of date, and those with only small edits.
fn classify(
    overview: &Overview,
    changed: &[Change],
    new_folders: bool,
) -> (Vec<String>, Vec<String>) {
    let mut stale = vec![];
    let mut minor = vec![];
    for section in &overview.sections {
        let touched: Vec<&Change> = changed
            .iter()
            .filter(|change| {
                section.paths.is_empty()
                    || section.paths.iter().any(|path| under(change.path, path))
            })
            .collect();
        let layout = new_folders && LAYOUT_SECTIONS.contains(&section.id.as_str());
        if touched.is_empty() && !layout {
            continue;
        }
        let named_file_gone = touched
            .iter()
            .any(|change| change.status == 'D' && section.paths.iter().any(|p| p == change.path));
        let files = touched
            .iter()
            .filter(|change| matches!(change.status, 'A' | 'D'))
            .count() as u32;
        let score = touched.iter().map(|change| change.lines).sum::<u32>() + files * FILE_WEIGHT;
        if layout || named_file_gone || score >= STALE_SCORE {
            stale.push(section.id.clone());
        } else {
            minor.push(section.id.clone());
        }
    }
    (stale, minor)
}

fn top_folders(listing: &str) -> HashSet<&str> {
    listing
        .lines()
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .collect()
}

pub async fn drift(repo: &Repo, overview: &Overview) -> Drift {
    let everything = || Drift {
        missing: true,
        sections: overview.sections.iter().map(|s| s.id.clone()).collect(),
        ..Drift::default()
    };
    let Some(anchor) = overview.head_sha.as_deref() else {
        return everything();
    };
    if repo.head().await.as_deref() == Some(anchor) {
        return Drift::default();
    }
    let commit = format!("{anchor}^{{commit}}");
    if repo.run(&["cat-file", "-e", &commit]).await.is_err() {
        return everything();
    }
    let range = format!("{anchor}..HEAD");
    let count_args = ["rev-list", "--count", range.as_str()];
    let status_args = ["diff", "--name-status", "--no-renames", anchor, "HEAD"];
    let numstat_args = ["diff", "--numstat", "--no-renames", anchor, "HEAD"];
    let before_args = ["ls-tree", "-d", "--name-only", anchor];
    let after_args = ["ls-tree", "-d", "--name-only", "HEAD"];
    let (count, name_status, numstat, before, after) = tokio::join!(
        text(repo, &count_args),
        text(repo, &status_args),
        text(repo, &numstat_args),
        text(repo, &before_args),
        text(repo, &after_args),
    );
    let changed = changes(&name_status, &numstat);
    let existing = top_folders(&before);
    let new_folders: HashSet<&str> = top_folders(&after)
        .into_iter()
        .filter(|folder| !existing.contains(folder))
        .collect();
    let new_code_folder = changed.iter().any(|change| {
        change.status == 'A'
            && language_of(change.path).is_some()
            && change
                .path
                .split_once('/')
                .is_some_and(|(folder, _)| new_folders.contains(folder))
    });
    let (sections, minor) = classify(overview, &changed, new_code_folder);
    Drift {
        commits: count.trim().parse().unwrap_or(0),
        files: changed.len(),
        sections,
        minor,
        missing: false,
    }
}

// MARK: Prompt

const SCHEMA: &str = r##"Answer with ONLY one JSON object, no prose before or after it and no code fence:
{
  "purpose": "Two or three sentences: what the project does, for whom, and the main technology it is built on.",
  "sections": [
    { "id": "architecture", "paths": ["src/", "server/api/"], "blocks": [
      { "type": "diagram", "title": "…", "direction": "right | down",
        "nodes": [{ "id": "a", "label": "Name", "detail": "one short line", "kind": "ui | module | service | store | external | file", "group": "layer name" }],
        "edges": [{ "from": "a", "to": "b", "label": "optional verb" }] },
      { "type": "text", "markdown": "Short paragraphs. `code`, **bold** and lists are fine." }
    ] },
    { "id": "modules", "paths": ["src/", "lib/"], "blocks": [
      { "type": "fileTree", "files": [{ "path": "src/lib/", "note": "What lives here and its job" }] }
    ] },
    { "id": "start", "paths": ["src/main.ts"], "blocks": [
      { "type": "text", "markdown": "…" },
      { "type": "code", "file": "src/types.ts", "language": "ts", "code": "…", "summary": "…" }
    ] }
  ]
}"##;

const WHAT_EACH_SECTION_HOLDS: &str = r#"Sections, with these ids:
- "architecture": one "diagram" of the main layers, modules, processes and external systems and how they call each other (8 to 16 nodes, grouped by layer, with a "kind" for each), then one "text" block of one or two short paragraphs on how the design works and the constraints it keeps.
- "modules": one "fileTree" listing the 6 to 15 folders and files that matter most, each with a one-line note on its job. Paths exactly as tracked; folders end in "/".
- "start": one "text" block naming the entry points and the three to five files to read first, in order, each with why. Optionally one short "code" block with a central type or function, copied from the repository.

Each section's "paths" lists the folders and files its content describes, so Seam can tell when a change makes it out of date. Use the narrowest folders that hold what the section describes ("src/lib/" and "src/views/" rather than "src/"), and name a single file only when the section is about that file."#;

fn language(config: &AiConfig) -> &str {
    let language = config.review.language.trim();
    if language.is_empty() {
        "English"
    } else {
        language
    }
}

/// The facts Seam read itself, so the model starts from them instead of rediscovering them.
fn facts_text(facts: &Facts) -> String {
    let mut lines = vec![format!("Name: {}", facts.name)];
    if let Some(description) = &facts.description {
        lines.push(format!(
            "Description ({}): {}",
            description.source, description.text
        ));
    }
    if !facts.stack.is_empty() {
        let stack: Vec<String> = facts
            .stack
            .iter()
            .map(|tech| match &tech.version {
                Some(version) => format!("{} {version}", tech.name),
                None => tech.name.clone(),
            })
            .collect();
        lines.push(format!("Stack: {}", stack.join(", ")));
    }
    if !facts.languages.is_empty() {
        let names: Vec<&str> = facts.languages.iter().map(|l| l.name.as_str()).collect();
        lines.push(format!("Languages, largest first: {}", names.join(", ")));
    }
    if !facts.scripts.is_empty() {
        let scripts: Vec<&str> = facts.scripts.iter().map(|s| s.run.as_str()).collect();
        lines.push(format!("Scripts: {}", scripts.join(", ")));
    }
    if !facts.hot_paths.is_empty() {
        let hot: Vec<&str> = facts.hot_paths.iter().map(|h| h.path.as_str()).collect();
        lines.push(format!(
            "Most changed in the last 30 days: {}",
            hot.join(", ")
        ));
    }
    lines.join("\n")
}

fn access(config: &AiConfig) -> &'static str {
    match config.provider.kind() {
        Kind::Harness => "Explore the repository read-only: its layout, manifests, entry points and the code that ties them together. Do not modify any file.",
        Kind::Api => "You cannot open the repository; the file list and documents below are all you have. Say so in the text when a conclusion is inferred from names alone.",
    }
}

fn full_prompt(config: &AiConfig, facts: &Facts, context: &str) -> String {
    format!(
        r#"Write an overview of this repository for a developer opening it for the first time. {access}

An overview answers: what the project does and for whom, how it is put together, what lives where, and where to start reading. Ground every statement in the repository; never invent files, modules or features. No intros or disclaimers. Never include secrets or tokens. Write all prose in {language}.

{WHAT_EACH_SECTION_HOLDS}

{SCHEMA}

What Seam already read from git and the manifests:
{facts}
{context}"#,
        access = access(config),
        language = language(config),
        facts = facts_text(facts),
    )
}

fn update_prompt(
    config: &AiConfig,
    facts: &Facts,
    previous: &Overview,
    ids: &[String],
    since: &str,
    context: &str,
) -> String {
    let current: Vec<&Section> = previous
        .sections
        .iter()
        .filter(|section| ids.contains(&section.id))
        .collect();
    format!(
        r#"This repository's overview was written at commit {anchor}, and the code has moved since. Update it. {access}

Rewrite only these sections: {ids}. Keep what is still true and change what the new commits made wrong; add what they introduced. Return "purpose" only when it is no longer accurate, else "". Ground every statement in the repository; never invent files, modules or features. Write all prose in {language}.

{WHAT_EACH_SECTION_HOLDS}

{SCHEMA}

Return only the sections named above.

The overview's purpose today: {purpose}

Those sections today:
{current}

{since}

What Seam already read from git and the manifests:
{facts}
{context}"#,
        anchor = previous.head_sha.as_deref().unwrap_or("an unknown commit"),
        access = access(config),
        ids = ids.join(", "),
        language = language(config),
        purpose = previous.purpose,
        current = serde_json::to_string_pretty(&current).unwrap_or_default(),
        facts = facts_text(facts),
    )
}

// MARK: Parsing

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawOverview {
    purpose: String,
    sections: Vec<RawSection>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSection {
    id: String,
    title: String,
    paths: Vec<String>,
    blocks: Vec<serde_json::Value>,
}

/// Keeps the blocks an overview uses, every path pointing at something tracked.
fn tidy_blocks(blocks: Vec<serde_json::Value>, tracked: &Tracked) -> Vec<Block> {
    blocks
        .into_iter()
        .filter_map(|value| serde_json::from_value::<Block>(value).ok())
        .filter_map(|block| match block {
            Block::Text(text) => (!text.markdown.trim().is_empty()).then_some(Block::Text(text)),
            Block::Diagram(mut diagram) => {
                visual::tidy_diagram(&mut diagram);
                (!diagram.nodes.is_empty()).then_some(Block::Diagram(diagram))
            }
            Block::FileTree(mut tree) => {
                let mut seen = HashSet::new();
                tree.files
                    .retain_mut(|file| match tracked.resolve(&file.path) {
                        Some(path) if seen.insert(path.clone()) => {
                            file.path = path;
                            file.change.clear();
                            file.additions = 0;
                            file.deletions = 0;
                            true
                        }
                        _ => false,
                    });
                (!tree.files.is_empty()).then_some(Block::FileTree(tree))
            }
            Block::Code(mut code) => {
                let file = code.file.trim();
                let grounded = file.is_empty() || tracked.resolve(file).is_some();
                code.annotations.clear();
                (grounded && !code.code.trim().is_empty()).then_some(Block::Code(code))
            }
            _ => None,
        })
        .collect()
}

fn parse_answer(answer: &str, tracked: &Tracked) -> Result<(String, Vec<Section>)> {
    let raw: RawOverview = ai::json_object(answer).ok_or_else(|| ai::not_json(answer))?;
    let mut sections: Vec<Section> = vec![];
    for raw_section in raw.sections {
        let id = raw_section.id.trim().to_lowercase();
        let Some((_, title)) = SECTIONS.iter().find(|(known, _)| *known == id) else {
            continue;
        };
        if sections.iter().any(|section| section.id == id) {
            continue;
        }
        let blocks = tidy_blocks(raw_section.blocks, tracked);
        if blocks.is_empty() {
            continue;
        }
        let mut paths: Vec<String> = raw_section
            .paths
            .iter()
            .filter_map(|path| tracked.resolve(path))
            .collect();
        paths.dedup();
        sections.push(Section {
            title: if raw_section.title.trim().is_empty() {
                title.to_string()
            } else {
                raw_section.title.trim().to_string()
            },
            id,
            paths,
            blocks,
        });
    }
    let purpose = raw.purpose.trim().to_string();
    if sections.is_empty() && purpose.is_empty() {
        return Err(ai::not_json(answer));
    }
    Ok((purpose, sections))
}

/// The rewritten sections in place of the old ones, the rest kept word for word.
fn merge(previous: &Overview, purpose: String, fresh: Vec<Section>) -> (String, Vec<Section>) {
    let mut sections: Vec<Section> = SECTIONS
        .iter()
        .filter_map(|(id, _)| {
            fresh
                .iter()
                .find(|section| section.id == *id)
                .or_else(|| previous.sections.iter().find(|section| section.id == *id))
                .cloned()
        })
        .collect();
    sections.dedup_by(|first, second| first.id == second.id);
    let purpose = if purpose.is_empty() {
        previous.purpose.clone()
    } else {
        purpose
    };
    (purpose, sections)
}

// MARK: Running

/// Writes the whole overview, or with `previous`, rewrites only the sections in `only`.
pub async fn run(
    config: &AiConfig,
    root: &Path,
    facts: &Facts,
    previous: Option<&Overview>,
    only: &[String],
) -> Result<Overview> {
    let started = Instant::now();
    let repo = Repo::new(root);
    let tracked = Tracked::new(&repo.run(&["ls-files"]).await?.text());
    let context = match config.provider.kind() {
        Kind::Api => format!(
            "\nRepository overview:\n{}",
            visual::repository_context(&repo, root).await
        ),
        Kind::Harness => String::new(),
    };
    let update = previous
        .filter(|_| !only.is_empty())
        .and_then(|previous| Some((previous, previous.head_sha.clone()?)));
    let prompt = match &update {
        Some((previous, anchor)) => {
            let names = text(&repo, &["diff", "--name-status", anchor.as_str(), "HEAD"]).await;
            let changes: Vec<&str> = names.lines().take(MAX_CHANGED_LISTED).collect();
            let paths: Vec<String> = previous
                .sections
                .iter()
                .filter(|section| only.contains(&section.id))
                .flat_map(|section| section.paths.clone())
                .collect();
            let mut args = vec![
                "diff",
                "--no-color",
                "--no-ext-diff",
                "-M",
                anchor.as_str(),
                "HEAD",
            ];
            if !paths.is_empty() {
                args.push("--");
                args.extend(paths.iter().map(String::as_str));
            }
            let diff_text = text(&repo, &args).await;
            let (diff, truncated) = review::clip(&diff_text);
            let since = format!(
                "Files changed since {anchor}:\n{}\n\nDiff of the files under those sections{}:\n{diff}",
                changes.join("\n"),
                if truncated {
                    " (truncated: large change)"
                } else {
                    ""
                },
            );
            update_prompt(config, facts, previous, only, &since, &context)
        }
        None => full_prompt(config, facts, &context),
    };
    let answer = ai::ask(config, root, &prompt).await?;
    let (purpose, sections) = match parse_answer(&answer, &tracked) {
        Ok(parsed) => parsed,
        Err(first) => {
            let Some(problem) = visual::json_problem(&answer) else {
                return Err(first);
            };
            let repaired = ai::ask(config, root, &visual::repair_prompt(&answer, &problem)).await?;
            parse_answer(&repaired, &tracked)?
        }
    };
    let (purpose, sections) = match &update {
        Some((previous, _)) => merge(previous, purpose, sections),
        None => (purpose, sections),
    };
    Ok(Overview {
        id: uuid::Uuid::new_v4().to_string(),
        provider: config.provider,
        head_sha: repo.head().await,
        branch: repo.current_branch().await,
        purpose,
        sections,
        created_at: review::now_secs(),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

// MARK: Context for other features

/// What Review and Visual are told about the project, from its newest saved overview; empty
/// before one is written.
pub fn project_context(git_dir: &Path) -> String {
    latest(git_dir)
        .map(|overview| context_of(&overview))
        .unwrap_or_default()
}

fn context_of(overview: &Overview) -> String {
    let written_at = overview
        .head_sha
        .as_deref()
        .map(|sha| format!(" at {}", &sha[..sha.len().min(7)]))
        .unwrap_or_default();
    let mut lines = vec![
        format!("About this project, from the overview Seam saved{written_at} (the code may have moved since):"),
        overview.purpose.clone(),
    ];
    for section in overview
        .sections
        .iter()
        .filter(|section| CONTEXT_SECTIONS.contains(&section.id.as_str()))
    {
        lines.push(format!("\n{}:", section.title));
        for block in &section.blocks {
            match block {
                Block::Text(text) => lines.push(text.markdown.trim().to_string()),
                Block::FileTree(tree) => lines.extend(tree.files.iter().map(|file| {
                    match file.note.as_deref().filter(|note| !note.trim().is_empty()) {
                        Some(note) => format!("- {}: {note}", file.path),
                        None => format!("- {}", file.path),
                    }
                })),
                Block::Diagram(diagram) => {
                    let label = |id: &str| {
                        diagram
                            .nodes
                            .iter()
                            .find(|node| node.id == id)
                            .map_or(id.to_string(), |node| node.label.clone())
                    };
                    lines.extend(diagram.nodes.iter().map(|node| {
                        match node
                            .detail
                            .as_deref()
                            .filter(|detail| !detail.trim().is_empty())
                        {
                            Some(detail) => format!("- {}: {detail}", node.label),
                            None => format!("- {}", node.label),
                        }
                    }));
                    lines.extend(diagram.edges.iter().map(|edge| match &edge.label {
                        Some(verb) if !verb.trim().is_empty() => {
                            format!("- {} → {} ({verb})", label(&edge.from), label(&edge.to))
                        }
                        _ => format!("- {} → {}", label(&edge.from), label(&edge.to)),
                    }));
                }
                _ => {}
            }
        }
    }
    let text = lines.join("\n");
    if text.chars().count() <= MAX_CONTEXT_CHARS {
        return text;
    }
    let cut: String = text.chars().take(MAX_CONTEXT_CHARS).collect();
    format!("{}…", cut.trim_end())
}

// MARK: Saving

fn dir(git_dir: &Path) -> PathBuf {
    git_dir.join("seam").join("overviews")
}

fn overview_path(git_dir: &Path, id: &str) -> Option<PathBuf> {
    let safe = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    safe.then(|| dir(git_dir).join(format!("{id}.json")))
}

fn write_json(path: &Path, value: &impl Serialize) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(value) {
        let _ = std::fs::write(path, json);
    }
}

fn summary(overview: &Overview) -> OverviewSummary {
    OverviewSummary {
        id: overview.id.clone(),
        purpose: overview.purpose.clone(),
        head_sha: overview.head_sha.clone(),
        branch: overview.branch.clone(),
        created_at: overview.created_at,
    }
}

/// Every saved overview, newest first.
pub fn history(git_dir: &Path) -> Vec<OverviewSummary> {
    std::fs::read_to_string(dir(git_dir).join("index.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Keeps an overview in the git directory (never committed), the newest `MAX_SAVED`.
pub fn save(git_dir: &Path, overview: &Overview) {
    let Some(path) = overview_path(git_dir, &overview.id) else {
        return;
    };
    write_json(&path, overview);
    let mut list = history(git_dir);
    list.retain(|entry| entry.id != overview.id);
    list.insert(0, summary(overview));
    for old in list.iter().skip(MAX_SAVED) {
        if let Some(path) = overview_path(git_dir, &old.id) {
            let _ = std::fs::remove_file(path);
        }
    }
    list.truncate(MAX_SAVED);
    write_json(&dir(git_dir).join("index.json"), &list);
}

pub fn load(git_dir: &Path, id: &str) -> Option<Overview> {
    serde_json::from_str(&std::fs::read_to_string(overview_path(git_dir, id)?).ok()?).ok()
}

pub fn latest(git_dir: &Path) -> Option<Overview> {
    history(git_dir)
        .first()
        .and_then(|entry| load(git_dir, &entry.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_dependencies_from_each_kind_of_manifest() {
        let package = r#"{"dependencies":{"react":"^19.3.1","left-pad":"1"},"devDependencies":{"vite":"workspace:*"}}"#;
        assert_eq!(
            dependencies("package.json", package),
            [("React", Some("19.3".into())), ("Vite", None)]
        );
        let cargo = "[package]\nname = \"tokio\"\n\n[dependencies]\ntauri = { version = \"2\", features = [] }\nserde_json = \"1\"\n\n[dev-dependencies]\ntokio = \"1.40\"\n";
        assert_eq!(
            dependencies("src-tauri/Cargo.toml", cargo),
            [("Tauri", Some("2".into())), ("Tokio", Some("1.40".into()))]
        );
        let python = "description = \"A flask app\"\ndjango>=4.2\n\"fastapi==0.110.1\",\n";
        assert_eq!(
            dependencies("pyproject.toml", python),
            [
                ("Django", Some("4.2".into())),
                ("FastAPI", Some("0.110".into()))
            ]
        );
        let go = "module x\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1\n\tgithub.com/labstack/echo/v4 v4.11.0\n)\n";
        assert_eq!(
            dependencies("go.mod", go),
            [("Gin", Some("1.9".into())), ("Echo", Some("4.11".into()))]
        );
        assert_eq!(
            dependencies("Gemfile", "gem 'rails', '~> 7.1'\n"),
            [("Rails", Some("7.1".into()))]
        );
    }

    #[test]
    fn the_readme_paragraph_skips_headings_badges_and_markup() {
        let readme = "# Seam\n\n[![CI](https://x/badge.svg)](https://x)\n<p align=\"center\"><img src=\"x\"></p>\n\nSeam is a **desktop** git client\nbuilt on [Tauri](https://tauri.app).\n\nSecond paragraph.\n";
        assert_eq!(
            readme_paragraph(readme).as_deref(),
            Some("Seam is a desktop git client built on Tauri.")
        );
        assert_eq!(readme_paragraph("# Only a title\n"), None);
        let long = readme_paragraph(&format!("{}\n", "word ".repeat(200))).unwrap();
        assert!(long.ends_with('…') && long.chars().count() <= MAX_DESCRIPTION_CHARS + 1);
    }

    #[test]
    fn languages_hot_paths_and_weeks_are_counted_from_git_output() {
        let tree = "100644 blob a 300\tsrc/a.ts\n100644 blob b 200\tsrc/b.rs\n100644 blob c 900\tnode_modules/x/index.js\n100644 blob d 50\tREADME.md\n";
        let names: Vec<(String, u64)> = languages(tree)
            .into_iter()
            .map(|language| (language.name, language.bytes))
            .collect();
        assert_eq!(names, [("TypeScript".into(), 300), ("Rust".into(), 200)]);

        let hot =
            hot_paths("src/lib/a.ts\nsrc/lib/b.ts\n\nsrc/main.ts\nREADME.md\nsrc/lib/c/d.ts\n");
        let hot: Vec<(&str, usize)> = hot.iter().map(|h| (h.path.as_str(), h.changes)).collect();
        assert_eq!(hot, [("src/lib/", 3), ("README.md", 1), ("src/", 1)]);

        let now = 100 * WEEK_SECS;
        let stamps = format!(
            "{}\n{}\n{}\n{}\n",
            now - 10,
            now - WEEK_SECS - 1,
            now - 50 * WEEK_SECS,
            now
        );
        let weeks = weekly(&stamps, now);
        assert_eq!(weeks.len(), WEEKS);
        assert_eq!((weeks[WEEKS - 1], weeks[WEEKS - 2]), (2, 1));
        assert_eq!(weeks.iter().sum::<usize>(), 3);
    }

    #[test]
    fn make_targets_leave_out_variables_and_special_targets() {
        let makefile =
            "CC := gcc\n.PHONY: build\nbuild: deps\n\tcc main.c\ntest:\n# lint: off\nVERSION=1\n";
        assert_eq!(make_targets(makefile), ["build", "test"]);
    }

    const TRACKED: &str = "src/main.ts\nsrc/lib/store.ts\nsrc-tauri/src/lib.rs\nREADME.md\n";

    #[test]
    fn an_overview_is_grounded_in_the_tracked_files() {
        let tracked = Tracked::new(TRACKED);
        let answer = r#"Here: {"purpose":"Does things.","sections":[
            {"id":"Modules","paths":["./src","nowhere/","src/lib/store.ts"],"blocks":[
                {"type":"fileTree","files":[{"path":"src/lib","note":"State","change":"added","additions":4},{"path":"src/invented.ts","note":"x"},{"path":"src/lib/"}]},
                {"type":"diff","file":"src/main.ts"}
            ]},
            {"id":"architecture","blocks":[{"type":"diagram","nodes":[{"id":"a"},{"id":"a"},{"id":"b"}],"edges":[{"from":"a","to":"c"}]}]},
            {"id":"modules","blocks":[{"type":"text","markdown":"duplicate"}]},
            {"id":"hologram","blocks":[{"type":"text","markdown":"unknown section"}]},
            {"id":"start","blocks":[{"type":"code","file":"src/gone.ts","code":"x"},{"type":"text","markdown":"  "}]}
        ]}"#;
        let (purpose, sections) = parse_answer(answer, &tracked).unwrap();
        assert_eq!(purpose, "Does things.");
        let ids: Vec<&str> = sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["modules", "architecture"]);
        assert_eq!(sections[0].title, "What lives where");
        assert_eq!(sections[0].paths, ["src/", "src/lib/store.ts"]);
        assert_eq!(sections[0].blocks.len(), 1);
        let Block::FileTree(tree) = &sections[0].blocks[0] else {
            panic!("the file tree stays");
        };
        let files: Vec<(&str, &str, u32)> = tree
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.change.as_str(), file.additions))
            .collect();
        assert_eq!(files, [("src/lib/", "", 0)]);
        let Block::Diagram(diagram) = &sections[1].blocks[0] else {
            panic!("then the diagram");
        };
        assert_eq!(diagram.nodes.len(), 2);
        assert!(diagram.edges.is_empty());

        assert!(parse_answer("no json", &tracked).is_err());
        assert!(parse_answer(r#"{"sections":[]}"#, &tracked).is_err());
    }

    fn overview(sections: &[(&str, &[&str])]) -> Overview {
        Overview {
            id: "o1".into(),
            provider: Provider::Claude,
            head_sha: Some("abc".into()),
            branch: Some("main".into()),
            purpose: "Old purpose.".into(),
            sections: sections
                .iter()
                .map(|(id, paths)| Section {
                    id: id.to_string(),
                    title: id.to_string(),
                    paths: paths.iter().map(|p| p.to_string()).collect(),
                    blocks: vec![],
                })
                .collect(),
            created_at: 1,
            duration_ms: 0,
        }
    }

    #[test]
    fn a_section_is_out_of_date_only_when_its_paths_change_enough() {
        let saved = overview(&[
            ("architecture", &["src-tauri/src/"]),
            ("modules", &["src/", "README.md"]),
            ("start", &["src/main.ts"]),
        ]);
        let classified = |status: &str, numstat: &str, new_folders: bool| {
            classify(&saved, &changes(status, numstat), new_folders)
        };
        let none: Vec<String> = vec![];

        assert_eq!(classified("", "", false), (none.clone(), none.clone()));
        assert_eq!(
            classified("M\tsrc/lib/store.ts\n", "12\t3\tsrc/lib/store.ts\n", false),
            (none.clone(), vec!["modules".to_string()])
        );
        assert_eq!(
            classified(
                "M\tsrc/a.ts\nM\tsrc/b.ts\n",
                "200\t40\tsrc/a.ts\n50\t10\tsrc/b.ts\n",
                false
            ),
            (vec!["modules".to_string()], none.clone())
        );
        let added = "A\tsrc-tauri/src/a.rs\nA\tsrc-tauri/src/b.rs\nA\tsrc-tauri/src/c.rs\n";
        let added_lines =
            "10\t0\tsrc-tauri/src/a.rs\n10\t0\tsrc-tauri/src/b.rs\n-\t-\tsrc-tauri/src/c.rs\n";
        assert_eq!(
            classified(added, added_lines, false),
            (vec!["architecture".to_string()], none.clone())
        );
        assert_eq!(
            classified("D\tsrc/main.ts\n", "0\t4\tsrc/main.ts\n", false),
            (vec!["start".to_string()], vec!["modules".to_string()])
        );
        assert_eq!(
            classified("M\tdocs/notes.md\n", "1\t0\tdocs/notes.md\n", true),
            (
                vec!["architecture".to_string(), "modules".to_string()],
                none
            )
        );
    }

    #[test]
    fn an_update_replaces_only_the_sections_it_rewrote() {
        let previous = overview(&[("architecture", &["a/"]), ("modules", &["b/"])]);
        let fresh = overview(&[("modules", &["c/"]), ("start", &["d/"])]).sections;
        let (purpose, sections) = merge(&previous, String::new(), fresh);
        assert_eq!(purpose, "Old purpose.");
        let shown: Vec<(&str, &str)> = sections
            .iter()
            .map(|s| (s.id.as_str(), s.paths[0].as_str()))
            .collect();
        assert_eq!(
            shown,
            [("architecture", "a/"), ("modules", "c/"), ("start", "d/")]
        );
        let (purpose, _) = merge(&previous, "New purpose.".into(), vec![]);
        assert_eq!(purpose, "New purpose.");
    }

    #[test]
    fn other_features_are_told_the_purpose_architecture_and_modules() {
        let mut saved = overview(&[]);
        saved.head_sha = Some("abcdef1234".into());
        saved.sections = parse_answer(
            r#"{"purpose":"P","sections":[
                {"id":"architecture","blocks":[
                    {"type":"diagram","nodes":[{"id":"ui","label":"Views","detail":"React"},{"id":"host","label":"Host"}],"edges":[{"from":"ui","to":"host","label":"invoke"}]},
                    {"type":"text","markdown":"Two processes."}]},
                {"id":"modules","blocks":[{"type":"fileTree","files":[{"path":"src/lib/","note":"State"}]}]},
                {"id":"start","blocks":[{"type":"text","markdown":"Read main.ts first."}]}
            ]}"#,
            &Tracked::new(TRACKED),
        )
        .unwrap()
        .1;
        let context = context_of(&saved);
        assert!(context.starts_with("About this project, from the overview Seam saved at abcdef1"));
        for expected in [
            "\nOld purpose.\n",
            "- Views: React",
            "- Views → Host (invoke)",
            "Two processes.",
            "- src/lib/: State",
        ] {
            assert!(context.contains(expected), "{expected:?} in {context}");
        }
        assert!(!context.contains("Read main.ts first."));

        let dir = tempfile::tempdir().unwrap();
        assert_eq!(project_context(dir.path()), "");
        saved.purpose = "word ".repeat(2000);
        assert!(context_of(&saved).chars().count() <= MAX_CONTEXT_CHARS + 1);
    }

    #[test]
    fn saved_overviews_keep_the_newest_and_stay_in_their_folder() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        assert!(latest(git_dir).is_none());
        for index in 0..(MAX_SAVED as i64 + 2) {
            let mut saved = overview(&[]);
            saved.id = format!("o{index}");
            saved.created_at = index;
            save(git_dir, &saved);
        }
        assert_eq!(history(git_dir).len(), MAX_SAVED);
        assert_eq!(latest(git_dir).unwrap().id, format!("o{}", MAX_SAVED + 1));
        assert!(load(git_dir, "o0").is_none());
        assert!(overview_path(git_dir, "../../etc/passwd").is_none());
    }
}
