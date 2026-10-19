use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use seam_lib::ai::Provider;
use seam_lib::git::Repo;
use seam_lib::mcp::tools::Context;
use seam_lib::mcp::Session;
use seam_lib::review;

async fn repository() -> (tempfile::TempDir, PathBuf, Repo) {
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
    std::fs::write(root.join("limit.js"), "export const max = 100;\n").unwrap();
    repo.run(&["add", "."]).await.unwrap();
    repo.run(&["commit", "-q", "-m", "Base"]).await.unwrap();
    repo.run(&["switch", "-q", "-c", "feature"]).await.unwrap();
    std::fs::write(
        root.join("limit.js"),
        "export const max = 100;\nexport const unlimited = max == 0;\n",
    )
    .unwrap();
    repo.run(&["commit", "-q", "-am", "Treat 0 as unlimited"])
        .await
        .unwrap();
    (dir, root, repo)
}

fn session(root: &Path, opened: Arc<Mutex<Vec<PathBuf>>>) -> Session {
    Session::new(Context {
        cwd: root.to_path_buf(),
        open: Box::new(move |root| {
            opened.lock().unwrap().push(root.to_path_buf());
            Ok(())
        }),
        fallback_provider: Provider::Codex,
    })
}

async fn request(session: &mut Session, id: u64, method: &str, params: Value) -> Value {
    session
        .handle(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
        .await
        .expect("a reply")
}

async fn tool(session: &mut Session, name: &str, arguments: Value) -> (String, bool) {
    let reply = request(
        session,
        9,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
    .await;
    let result = &reply["result"];
    (
        result["content"][0]["text"].as_str().unwrap().to_string(),
        result["isError"].as_bool().unwrap_or(false),
    )
}

#[tokio::test]
async fn an_agent_reads_a_change_comments_on_it_and_reads_the_comments_back() {
    let (_dir, root, repo) = repository().await;
    let opened = Arc::new(Mutex::new(vec![]));
    let mut session = session(&root, opened.clone());

    let hello = request(
        &mut session,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": { "name": "claude-code", "version": "2" }
        }),
    )
    .await;
    assert_eq!(hello["result"]["protocolVersion"], "2025-03-26");
    assert!(hello["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("leave_comments"));
    assert!(session
        .handle(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
        .await
        .is_none());

    let listed = request(&mut session, 2, "tools/list", json!({})).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["get_diff", "open_diff", "leave_comments", "get_review"]
    );

    let (diff, failed) = tool(&mut session, "get_diff", json!({})).await;
    assert!(!failed);
    assert!(diff.starts_with("main…feature · 1 file +1 -0"), "{diff}");
    assert!(diff.contains("+export const unlimited = max == 0;"));

    let (refused, failed) = tool(
        &mut session,
        "leave_comments",
        json!({
            "summary": "s",
            "comments": [{ "file": "elsewhere.js", "title": "t", "body": "b" }]
        }),
    )
    .await;
    assert!(failed);
    assert!(refused.contains("limit.js"), "{refused}");
    assert!(opened.lock().unwrap().is_empty());

    let (left, failed) = tool(
        &mut session,
        "leave_comments",
        json!({
            "summary": "Adds an unlimited flag.",
            "comments": [{
                "file": "limit.js",
                "line": 2,
                "severity": "risk",
                "title": "0 now means unlimited",
                "body": "A caller passing 0 to block everything now allows everything."
            }]
        }),
    )
    .await;
    assert!(!failed, "{left}");
    assert!(
        left.starts_with("Left 1 comment on main…feature (1 risk"),
        "{left}"
    );
    assert_eq!(*opened.lock().unwrap(), vec![root.clone()]);

    let git_dir = repo.git_dir().await.unwrap();
    let saved = review::load(&git_dir).unwrap();
    assert_eq!(saved.provider, Provider::Claude);
    assert_eq!(saved.findings[0].line, Some(2));
    assert_eq!(saved.diffs.len(), 1);
    let requests = std::fs::read_dir(git_dir.join("seam/requests")).unwrap();
    assert_eq!(requests.count(), 1);

    let (read, failed) = tool(&mut session, "get_review", json!({})).await;
    assert!(!failed);
    let read: Value = serde_json::from_str(&read).unwrap();
    assert_eq!(read["by"], "Claude Code");
    assert_eq!(read["findings"][0]["severity"], "risk");
    assert_eq!(read["headMovedSince"], false);
}

#[tokio::test]
async fn it_answers_mistakes_without_falling_over() {
    let (_dir, root, _repo) = repository().await;
    let mut session = session(&root, Arc::new(Mutex::new(vec![])));

    let unknown = request(&mut session, 1, "resources/list", json!({})).await;
    assert_eq!(unknown["error"]["code"], -32601);

    let no_tool = request(&mut session, 2, "tools/call", json!({ "name": "rm_rf" })).await;
    assert_eq!(no_tool["error"]["code"], -32602);

    let (message, failed) = tool(
        &mut session,
        "get_diff",
        json!({ "staged": true, "commit": "HEAD" }),
    )
    .await;
    assert!(failed);
    assert!(message.contains("not several"));

    let (message, failed) = tool(&mut session, "get_review", json!({})).await;
    assert!(failed);
    assert!(message.contains("no review"));
}
