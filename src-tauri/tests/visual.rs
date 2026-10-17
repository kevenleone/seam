//! What a visual recap or plan starts from, and (opt-in) a real agent run of each.

use std::path::PathBuf;

use seam_lib::ai::{AiConfig, Provider};
use seam_lib::git::Repo;
use seam_lib::review;
use seam_lib::visual::{self, Block, VisualKind};

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

    async fn commit(&self, path: &str, content: &str, message: &str) {
        std::fs::write(self.root.join(path), content).unwrap();
        self.repo.run(&["add", "--", path]).await.unwrap();
        self.repo
            .run(&["commit", "-q", "-m", message])
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn a_plan_starts_from_the_branch_and_its_uncommitted_work() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "Base").await;

    let clean = visual::plan_scope(&t.repo).await.unwrap();
    assert_eq!(clean.label, "main");
    assert!(clean.diffs.is_empty());

    std::fs::write(t.root.join("a.txt"), "started\n").unwrap();
    let dirty = visual::plan_scope(&t.repo).await.unwrap();
    assert_eq!(dirty.diffs.len(), 1);
}

fn live_config() -> AiConfig {
    let id = std::env::var("SEAM_LIVE_AI").unwrap_or_else(|_| "claude".into());
    let provider: Provider = serde_json::from_str(&format!("\"{id}\"")).expect("a provider id");
    AiConfig {
        provider,
        model: std::env::var("SEAM_LIVE_MODEL").unwrap_or_default(),
        base_url: std::env::var("SEAM_LIVE_BASE_URL").unwrap_or_default(),
        ..AiConfig::default()
    }
}

/// Runs a real model: `SEAM_LIVE_AI=claude cargo test --test visual -- --ignored`.
/// Takes the same provider variables as the live review test; `SEAM_LIVE_OUT=<dir>` also
/// saves both documents there as JSON, to look at in the renderer.
#[tokio::test]
#[ignore]
async fn live_recap_and_plan_with_a_real_model() {
    let config = live_config();
    let t = T::new().await;
    t.commit(
        "index.html",
        "<form><input name=\"email\"><button>Sign up</button></form>\n",
        "Base",
    )
    .await;
    t.repo
        .run(&["switch", "-q", "-c", "feature"])
        .await
        .unwrap();
    t.commit(
        "index.html",
        "<form><input name=\"email\"><input name=\"password\" type=\"password\"><label><input type=\"checkbox\" name=\"terms\"> I accept the terms</label><button disabled>Sign up</button></form>\n",
        "Ask for a password and the terms",
    )
    .await;

    let scope = review::scope(&t.repo).await.unwrap();
    let recap = visual::run(&config, &t.root, VisualKind::Recap, scope, None)
        .await
        .unwrap();
    println!(
        "recap in {} ms: {} — {} screen(s), {} block(s)",
        recap.duration_ms,
        recap.title,
        recap.canvas.screens.len(),
        recap.blocks.len()
    );
    assert!(!recap.title.is_empty());
    assert!(recap
        .blocks
        .iter()
        .any(|block| matches!(block, Block::FileTree(_))));

    let scope = visual::plan_scope(&t.repo).await.unwrap();
    let plan = visual::run(
        &config,
        &t.root,
        VisualKind::Plan,
        scope,
        Some("Show an inline error under each field when the form is submitted empty.".into()),
    )
    .await
    .unwrap();
    println!(
        "plan in {} ms: {} — {} screen(s), {} block(s)",
        plan.duration_ms,
        plan.title,
        plan.canvas.screens.len(),
        plan.blocks.len()
    );
    assert!(!plan.blocks.is_empty() || !plan.canvas.screens.is_empty());

    if let Ok(dir) = std::env::var("SEAM_LIVE_OUT") {
        for (name, visual) in [("recap", &recap), ("plan", &plan)] {
            let json = serde_json::to_string_pretty(visual).unwrap();
            std::fs::write(PathBuf::from(&dir).join(format!("{name}.json")), json).unwrap();
        }
    }
}
