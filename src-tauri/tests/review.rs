//! What a review sends to the agent, and (opt-in) a real agent run.

use std::path::PathBuf;

use seam_lib::ai::{AiConfig, Provider};
use seam_lib::git::Repo;
use seam_lib::review::{self, Target};

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
async fn scope_is_the_branch_against_main() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "Base").await;
    t.repo
        .run(&["switch", "-q", "-c", "feature"])
        .await
        .unwrap();
    t.commit("b.txt", "b\n", "Add b").await;
    t.commit("c.txt", "c\n", "Add c").await;
    std::fs::write(t.root.join("dirty.txt"), "not part of the branch\n").unwrap();

    let scope = review::scope(&t.repo).await.unwrap();
    assert_eq!(scope.label, "main…feature");
    assert_eq!(scope.commits.len(), 2);
    let files: Vec<&str> = scope.diffs.iter().map(|d| d.path()).collect();
    assert_eq!(files, ["b.txt", "c.txt"]);
}

#[tokio::test]
async fn scope_falls_back_to_uncommitted_changes_on_main() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "Base").await;
    std::fs::write(t.root.join("a.txt"), "changed\n").unwrap();

    let scope = review::scope(&t.repo).await.unwrap();
    assert_eq!(scope.label, "Uncommitted changes");
    assert_eq!(scope.diffs.len(), 1);

    t.repo.run(&["checkout", "--", "a.txt"]).await.unwrap();
    assert!(review::scope(&t.repo).await.is_err());
}

fn files(scope: &review::Scope) -> Vec<&str> {
    scope.diffs.iter().map(|d| d.path()).collect()
}

#[tokio::test]
async fn a_named_target_scopes_exactly_that_change() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "Base").await;
    t.commit("b.txt", "b\n", "Add b").await;
    t.commit("c.txt", "c\n", "Add c").await;
    std::fs::write(t.root.join("a.txt"), "staged\n").unwrap();
    t.repo.run(&["add", "a.txt"]).await.unwrap();
    std::fs::write(t.root.join("c.txt"), "unstaged\n").unwrap();

    let staged = review::scope_of(&t.repo, &Target::Staged { paths: vec![] })
        .await
        .unwrap();
    assert_eq!(staged.label, "Staged changes");
    assert_eq!(files(&staged), ["a.txt"]);

    let working = review::scope_of(
        &t.repo,
        &Target::WorkingTree {
            paths: vec!["c.txt".into()],
        },
    )
    .await
    .unwrap();
    assert_eq!(files(&working), ["c.txt"]);

    let last_two = review::scope_of(
        &t.repo,
        &Target::Range {
            range: "HEAD~2".into(),
            paths: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(last_two.label, "HEAD~2..HEAD");
    assert_eq!(last_two.commits.len(), 2);
    assert_eq!(files(&last_two), ["b.txt", "c.txt"]);

    let commit = review::scope_of(
        &t.repo,
        &Target::Commit {
            rev: "HEAD~1".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(files(&commit), ["b.txt"]);
    assert!(commit.commits[0].ends_with("Add b"));
    assert_eq!(
        commit.target,
        Target::Commit {
            rev: "HEAD~1".into()
        }
    );

    let patch = t.root.join("change.patch");
    let text = t
        .repo
        .run(&["diff", "HEAD~1", "HEAD"])
        .await
        .unwrap()
        .text();
    std::fs::write(&patch, text).unwrap();
    let from_patch = review::scope_of(
        &t.repo,
        &Target::Patch {
            file: patch,
            label: "Patch from stdin".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(from_patch.label, "Patch from stdin");
    assert_eq!(files(&from_patch), ["c.txt"]);
}

#[tokio::test]
async fn a_named_target_with_nothing_in_it_says_so() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "Base").await;

    let empty = review::scope_of(&t.repo, &Target::Staged { paths: vec![] }).await;
    assert!(empty
        .err()
        .unwrap()
        .message
        .contains("Staged changes has no changes"));
    let unknown = review::scope_of(
        &t.repo,
        &Target::Range {
            range: "nope..HEAD".into(),
            paths: vec![],
        },
    )
    .await;
    assert!(unknown.is_err());
}

/// Runs a real model: `SEAM_LIVE_AI=claude cargo test --test review -- --ignored`.
/// Any provider id works (`codex`, `opencode`, `anthropic`, `openai`, `google`,
/// `openai-compatible`); API providers use the key saved in Settings, and
/// `SEAM_LIVE_MODEL` / `SEAM_LIVE_BASE_URL` fill in the model and endpoint.
#[tokio::test]
#[ignore]
async fn live_review_with_a_real_model() {
    let id = std::env::var("SEAM_LIVE_AI").unwrap_or_else(|_| "claude".into());
    let provider: Provider = serde_json::from_str(&format!("\"{id}\"")).expect("a provider id");
    let config = AiConfig {
        provider,
        model: std::env::var("SEAM_LIVE_MODEL").unwrap_or_default(),
        base_url: std::env::var("SEAM_LIVE_BASE_URL").unwrap_or_default(),
        ..AiConfig::default()
    };
    let t = T::new().await;
    t.commit(
        "limit.js",
        "export function allow(hits, max) {\n  return hits <= max;\n}\n",
        "Base",
    )
    .await;
    t.repo
        .run(&["switch", "-q", "-c", "feature"])
        .await
        .unwrap();
    t.commit(
        "limit.js",
        "export function allow(hits, max) {\n  // max 0 means unlimited\n  return hits < max || max == 0;\n}\n",
        "Treat max 0 as unlimited",
    )
    .await;

    let scope = review::scope(&t.repo).await.unwrap();
    let result = review::run(&config, &t.root, scope).await.unwrap();
    println!(
        "{} finding(s) in {} ms: {}",
        result.findings.len(),
        result.duration_ms,
        result.summary
    );
    for finding in &result.findings {
        println!(
            "  {:?} {}:{:?} {}",
            finding.severity, finding.file, finding.line, finding.title
        );
    }
    assert!(!result.summary.is_empty());
}

/// An OpenAI-compatible endpoint (what Ollama, LM Studio and OpenRouter speak), served
/// from this test, answering one chat completion with a canned review.
#[tokio::test]
async fn review_through_an_openai_compatible_endpoint() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 65536];
        // Read until the JSON body is complete (headers + Content-Length bytes).
        loop {
            let n = socket.read(&mut buffer).await.unwrap();
            request.extend_from_slice(&buffer[..n]);
            let text = String::from_utf8_lossy(&request);
            if let Some(end) = text.find("\r\n\r\n") {
                let length: usize = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let answer = r#"{"summary":"Tightens the limit by one.","findings":[{"file":"a.txt","line":1,"severity":"risk","title":"Off by one","body":"B","suggestion":null}]}"#;
        let body = serde_json::json!({ "choices": [{ "message": { "role": "assistant", "content": answer } }] }).to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8_lossy(&request).into_owned()
    });

    let t = T::new().await;
    t.commit("a.txt", "a\n", "Base").await;
    std::fs::write(t.root.join("a.txt"), "b\n").unwrap();
    let config = AiConfig {
        provider: Provider::OpenAiCompatible,
        model: "local-model".into(),
        base_url: format!("http://127.0.0.1:{port}/v1/"),
        ..AiConfig::default()
    };

    let scope = review::scope(&t.repo).await.unwrap();
    let result = review::run(&config, &t.root, scope).await.unwrap();
    let request = server.await.unwrap();

    assert!(request.starts_with("POST /v1/chat/completions"));
    assert!(request.contains("\"model\":\"local-model\""));
    assert_eq!(result.summary, "Tightens the limit by one.");
    assert_eq!(result.findings[0].title, "Off by one");
    assert_eq!(result.provider, Provider::OpenAiCompatible);
}
