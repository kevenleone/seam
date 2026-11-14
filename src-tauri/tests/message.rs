use std::path::PathBuf;

use seam_lib::ai::{AiConfig, Provider};
use seam_lib::git::Repo;
use seam_lib::message::{self, CommitMessage};

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

    async fn stage(&self, path: &str, content: &str) {
        std::fs::write(self.root.join(path), content).unwrap();
        self.repo.run(&["add", "--", path]).await.unwrap();
    }

    async fn commit(&self, path: &str, content: &str, message: &str) {
        self.stage(path, content).await;
        self.repo
            .run(&["commit", "-q", "-m", message])
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn staged_diff_holds_only_what_is_staged() {
    let t = T::new().await;
    t.commit("a.txt", "one\n", "Add a").await;
    t.stage("a.txt", "two\n").await;
    std::fs::write(t.root.join("b.txt"), "unstaged\n").unwrap();

    let diff = message::staged_diff(&t.repo, false).await.unwrap();
    assert!(diff.contains("+two"));
    assert!(!diff.contains("b.txt"));
}

#[tokio::test]
async fn staged_diff_for_amend_includes_the_last_commit() {
    let t = T::new().await;
    t.commit("a.txt", "one\n", "Add a").await;
    t.commit("b.txt", "bee\n", "Add b").await;
    t.stage("c.txt", "sea\n").await;

    let diff = message::staged_diff(&t.repo, true).await.unwrap();
    assert!(diff.contains("b.txt"));
    assert!(diff.contains("c.txt"));
    assert!(!diff.contains("a.txt"));
}

#[tokio::test]
async fn staged_diff_works_before_the_first_commit() {
    let t = T::new().await;
    t.stage("a.txt", "one\n").await;

    assert!(message::staged_diff(&t.repo, false)
        .await
        .unwrap()
        .contains("+one"));
    assert!(message::staged_diff(&t.repo, true)
        .await
        .unwrap()
        .contains("+one"));
}

#[tokio::test]
async fn nothing_staged_is_an_error_before_asking() {
    let t = T::new().await;
    t.commit("a.txt", "one\n", "Add a").await;
    let config = AiConfig {
        provider: Provider::OpenAiCompatible,
        base_url: "http://127.0.0.1:9".into(),
        ..Default::default()
    };

    let error = message::run(&config, &t.root, false, "").await.unwrap_err();
    assert!(error.to_string().contains("Nothing staged"));
}

#[test]
fn parse_tolerates_fences_and_tidies_the_summary() {
    let answer = "Here you go:\n```json\n{\"summary\": \"fix: keep the header pinned.\\nextra\", \"body\": \"  Why.  \"}\n```";
    assert_eq!(
        message::parse(answer).unwrap(),
        CommitMessage {
            summary: "fix: keep the header pinned".into(),
            body: "Why.".into(),
        }
    );
}

#[test]
fn parse_rejects_an_empty_summary() {
    assert!(message::parse("{\"summary\": \"  \", \"body\": \"x\"}").is_err());
    assert!(message::parse("no json here").is_err());
}

/// `SEAM_LIVE_AI=claude cargo test --test message -- --ignored`
#[tokio::test]
#[ignore]
async fn live_message() {
    let provider = std::env::var("SEAM_LIVE_AI").unwrap_or_else(|_| "claude".into());
    let provider: Provider = serde_json::from_value(serde_json::json!(provider)).unwrap();
    let t = T::new().await;
    t.commit(
        "greet.py",
        "def greet():\n    print('hi')\n",
        "feat: add greet",
    )
    .await;
    t.stage("greet.py", "def greet(name):\n    print(f'hi {name}')\n")
        .await;
    let config = AiConfig {
        provider,
        ..Default::default()
    };
    let message = message::run(&config, &t.root, false, "").await.unwrap();
    println!("{message:?}");
    assert!(!message.summary.is_empty());
    assert!(message.summary.len() <= 100);
}
