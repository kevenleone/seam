//! What the Overview reads from a repository on its own, how it tells an overview is out of
//! date, and (opt-in) a real agent writing one.

use std::path::PathBuf;

use seam_lib::ai::{AiConfig, Provider};
use seam_lib::git::Repo;
use seam_lib::overview::{self, Overview, Section};

struct T {
    _dir: tempfile::TempDir,
    root: PathBuf,
    repo: Repo,
}

impl T {
    async fn new() -> T {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let repo = Repo::new(&root);
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "/dev/null"],
        ] {
            repo.run(&args).await.unwrap();
        }
        T {
            _dir: dir,
            root,
            repo,
        }
    }

    async fn commit(&self, files: &[(&str, &str)], message: &str) {
        for (path, content) in files {
            let path = self.root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        self.repo.run(&["add", "-A"]).await.unwrap();
        self.repo
            .run(&["commit", "-q", "-m", message])
            .await
            .unwrap();
    }
}

const PACKAGE: &str = r#"{
  "description": "Not this one: the README comes first.",
  "scripts": { "dev": "vite", "test": "vitest" },
  "dependencies": { "react": "^19.3.1" },
  "devDependencies": { "vite": "^6.1.0", "left-pad": "1.0.0" }
}"#;

#[tokio::test]
async fn facts_come_from_git_and_the_manifests() {
    let t = T::new().await;
    let empty = overview::facts(&t.repo).await.unwrap();
    assert_eq!(empty.commits, 0);
    assert!(empty.head_sha.is_none());

    t.commit(
        &[
            ("README.md", "# Demo\n\nDemo shows **things**.\n"),
            ("package.json", PACKAGE),
            ("pnpm-lock.yaml", "lockfileVersion: 9\n"),
            ("src/app/main.ts", "export const main = 1;\n"),
            (
                "tests/fixtures/package.json",
                r#"{"dependencies":{"vue":"3"}}"#,
            ),
        ],
        "Start",
    )
    .await;
    t.commit(&[("src/app/main.ts", "export const main = 2;\n")], "Change")
        .await;
    t.repo.run(&["tag", "v0.1.0"]).await.unwrap();

    let facts = overview::facts(&t.repo).await.unwrap();
    let description = facts.description.unwrap();
    assert_eq!(
        (description.text.as_str(), description.source.as_str()),
        ("Demo shows things.", "README.md")
    );
    let stack: Vec<(&str, Option<&str>)> = facts
        .stack
        .iter()
        .map(|tech| (tech.name.as_str(), tech.version.as_deref()))
        .collect();
    assert_eq!(
        stack,
        [
            ("React", Some("19.3")),
            ("Vite", Some("6.1")),
            ("pnpm", None)
        ]
    );
    let scripts: Vec<&str> = facts.scripts.iter().map(|s| s.run.as_str()).collect();
    assert_eq!(scripts, ["pnpm dev", "pnpm test"]);
    assert_eq!((facts.commits, facts.contributors), (2, 1));
    assert_eq!(facts.latest_tag.as_deref(), Some("v0.1.0"));
    assert_eq!(facts.weekly.last(), Some(&2));
    assert_eq!(facts.hot_paths[0].path, "src/app/");
    assert_eq!(facts.hot_paths[0].changes, 2);
    assert_eq!(facts.languages[0].name, "TypeScript");
    assert_eq!(facts.head_sha, t.repo.head().await);
}

fn written_at(head: Option<String>) -> Overview {
    let section = |id: &str, paths: &[&str]| Section {
        id: id.into(),
        title: id.into(),
        paths: paths.iter().map(|p| p.to_string()).collect(),
        blocks: vec![],
    };
    Overview {
        id: "o1".into(),
        provider: Provider::Claude,
        head_sha: head,
        branch: Some("main".into()),
        purpose: "Demo.".into(),
        sections: vec![
            section("architecture", &["server/"]),
            section("modules", &["src/", "README.md"]),
        ],
        created_at: 0,
        duration_ms: 0,
    }
}

#[tokio::test]
async fn an_overview_drifts_only_where_the_code_moved() {
    let t = T::new().await;
    t.commit(&[("src/a.ts", "a\n"), ("server/b.rs", "b\n")], "Start")
        .await;
    let saved = written_at(t.repo.head().await);

    let fresh = overview::drift(&t.repo, &saved).await;
    assert_eq!((fresh.commits, fresh.files, fresh.missing), (0, 0, false));
    assert!(fresh.sections.is_empty());

    t.commit(&[("docs/notes.md", "notes\n")], "Docs").await;
    let untouched = overview::drift(&t.repo, &saved).await;
    assert_eq!((untouched.commits, untouched.files), (1, 1));
    assert!(untouched.sections.is_empty());

    t.commit(&[("src/a.ts", "changed\n")], "Change src").await;
    let moved = overview::drift(&t.repo, &saved).await;
    assert_eq!((moved.commits, moved.files), (2, 2));
    assert_eq!(moved.sections, ["modules"]);

    let gone = overview::drift(&t.repo, &written_at(Some("0".repeat(40)))).await;
    assert!(gone.missing);
    assert_eq!(gone.sections, ["architecture", "modules"]);
}

/// Runs a real model: `SEAM_LIVE_AI=claude cargo test --test overview -- --ignored` writes an
/// overview of this repository, then rewrites one section of it; `SEAM_LIVE_OUT=<dir>` keeps both.
#[tokio::test]
#[ignore]
async fn a_real_agent_writes_an_overview_and_updates_a_section() {
    let id = std::env::var("SEAM_LIVE_AI").unwrap_or_else(|_| "claude".into());
    let provider: Provider = serde_json::from_str(&format!("\"{id}\"")).expect("a provider id");
    let config = AiConfig {
        provider,
        model: std::env::var("SEAM_LIVE_MODEL").unwrap_or_default(),
        base_url: std::env::var("SEAM_LIVE_BASE_URL").unwrap_or_default(),
        ..AiConfig::default()
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let repo = Repo::new(&root);
    let facts = overview::facts(&repo).await.unwrap();

    let written = overview::run(&config, &root, &facts, None, &[])
        .await
        .unwrap();
    assert!(!written.purpose.is_empty());
    assert!(written.sections.iter().any(|s| s.id == "architecture"));

    let only = vec!["start".to_string()];
    let mut anchored = written.clone();
    anchored.head_sha = repo
        .run(&["rev-parse", "HEAD~3"])
        .await
        .ok()
        .map(|o| o.text().trim().to_string());
    let updated = overview::run(&config, &root, &facts, Some(&anchored), &only)
        .await
        .unwrap();
    let kept = |id: &str| {
        serde_json::to_string(
            &written
                .sections
                .iter()
                .find(|s| s.id == id)
                .map(|s| &s.blocks),
        )
        .unwrap()
    };
    let now = |id: &str| {
        serde_json::to_string(
            &updated
                .sections
                .iter()
                .find(|s| s.id == id)
                .map(|s| &s.blocks),
        )
        .unwrap()
    };
    assert_eq!(kept("architecture"), now("architecture"));

    if let Ok(dir) = std::env::var("SEAM_LIVE_OUT") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("overview.json"),
            serde_json::to_string_pretty(&written).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join("overview-updated.json"),
            serde_json::to_string_pretty(&updated).unwrap(),
        )
        .unwrap();
    }
}
