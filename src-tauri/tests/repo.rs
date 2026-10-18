//! Integration tests: real git, throwaway repositories.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use seam_lib::git::models::*;
use seam_lib::git::patch::Selection;
use seam_lib::git::process::{git, RunOptions};
use seam_lib::git::{GitError, Repo};

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

    fn write(&self, path: &str, content: &str) {
        let p = self.root.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).unwrap()
    }

    fn exists(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }

    async fn commit(&self, path: &str, content: &str, message: &str) -> String {
        self.write(path, content);
        self.repo.run(&["add", "--", path]).await.unwrap();
        self.repo
            .run(&["commit", "-q", "-m", message])
            .await
            .unwrap();
        self.repo.head().await.unwrap()
    }

    async fn git(&self, args: &[&str]) -> String {
        self.repo.run(args).await.unwrap().text().trim().to_string()
    }

    async fn subjects(&self) -> Vec<String> {
        self.repo
            .log("HEAD", 0, 100)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.subject)
            .collect()
    }
}

fn select(hunk: usize, lines: &[usize]) -> Selection {
    HashMap::from([(hunk, lines.iter().copied().collect())])
}

fn line_id(diff: &FileDiff, kind: LineKind, text: &str) -> (usize, usize) {
    for h in &diff.hunks {
        if let Some(l) = h.lines.iter().find(|l| l.kind == kind && l.text == text) {
            return (h.id, l.id);
        }
    }
    panic!("no {kind:?} line {text:?}");
}

#[tokio::test]
async fn stage_selected_lines_only() {
    let t = T::new().await;
    t.commit("a.txt", "one\ntwo\nthree\nfour\n", "Initial")
        .await;
    t.write("a.txt", "one\nTWO\nthree\nfour\nfive\n");
    let diff = t
        .repo
        .diff("a.txt", DiffSource::Unstaged, false)
        .await
        .unwrap()
        .unwrap();
    let (h, l) = line_id(&diff, LineKind::Addition, "five");
    let undo = t
        .repo
        .apply_lines(&diff, Some(&select(h, &[l])), LineAction::Stage)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        t.git(&["show", ":a.txt"]).await,
        "one\ntwo\nthree\nfour\nfive"
    );
    assert_eq!(t.read("a.txt"), "one\nTWO\nthree\nfour\nfive\n");
    t.repo.undo(&undo.action).await.unwrap();
    assert_eq!(t.git(&["diff", "--cached", "--name-only"]).await, "");
}

#[tokio::test]
async fn stage_deletion_line_only() {
    let t = T::new().await;
    t.commit("a.txt", "a\nb\nc\nd\n", "Initial").await;
    t.write("a.txt", "a\nc\nD\n");
    let diff = t
        .repo
        .diff("a.txt", DiffSource::Unstaged, false)
        .await
        .unwrap()
        .unwrap();
    let (h, l) = line_id(&diff, LineKind::Deletion, "b");
    t.repo
        .apply_lines(&diff, Some(&select(h, &[l])), LineAction::Stage)
        .await
        .unwrap();
    assert_eq!(t.git(&["show", ":a.txt"]).await, "a\nc\nd");
}

#[tokio::test]
async fn unstage_and_discard_lines() {
    let t = T::new().await;
    t.commit("a.txt", "1\n2\n3\n", "Initial").await;
    t.write("a.txt", "1\n2a\n3\n4\n");
    t.repo.run(&["add", "a.txt"]).await.unwrap();

    let staged = t
        .repo
        .diff("a.txt", DiffSource::Staged, false)
        .await
        .unwrap()
        .unwrap();
    let (h, l) = line_id(&staged, LineKind::Addition, "4");
    t.repo
        .apply_lines(&staged, Some(&select(h, &[l])), LineAction::Unstage)
        .await
        .unwrap();
    assert_eq!(t.git(&["show", ":a.txt"]).await, "1\n2a\n3");

    let unstaged = t
        .repo
        .diff("a.txt", DiffSource::Unstaged, false)
        .await
        .unwrap()
        .unwrap();
    let undo = t
        .repo
        .apply_lines(&unstaged, None, LineAction::Discard)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t.read("a.txt"), "1\n2a\n3\n");
    t.repo.undo(&undo.action).await.unwrap();
    assert_eq!(t.read("a.txt"), "1\n2a\n3\n4\n");
}

#[tokio::test]
async fn stage_part_of_untracked_file() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "Initial").await;
    t.write("new.txt", "x\ny\n");
    let diff = t
        .repo
        .diff("new.txt", DiffSource::Untracked, false)
        .await
        .unwrap()
        .unwrap();
    let (h, l) = line_id(&diff, LineKind::Addition, "x");
    t.repo
        .apply_lines(&diff, Some(&select(h, &[l])), LineAction::Stage)
        .await
        .unwrap();
    assert_eq!(t.git(&["show", ":new.txt"]).await, "x");
}

#[tokio::test]
async fn commit_amend_and_undo() {
    let t = T::new().await;
    t.write("a.txt", "a\n");
    t.repo.stage_all().await.unwrap();
    let first = t.repo.commit("First", false).await.unwrap();
    assert_eq!(t.subjects().await, ["First"]);

    t.write("a.txt", "b\n");
    t.repo.stage_all().await.unwrap();
    let amend = t.repo.commit("First, amended", true).await.unwrap();
    assert_eq!(t.subjects().await, ["First, amended"]);

    t.repo.undo(&amend.action).await.unwrap();
    assert_eq!(t.subjects().await, ["First"]);
    assert_eq!(t.git(&["diff", "--cached", "--name-only"]).await, "a.txt");

    t.repo.undo(&first.action).await.unwrap();
    assert_eq!(t.repo.head().await, None);
    let staged: Vec<String> = t
        .repo
        .status()
        .await
        .unwrap()
        .staged
        .into_iter()
        .map(|c| c.path)
        .collect();
    assert_eq!(staged, ["a.txt"]);
}

#[tokio::test]
async fn reword_drop_squash_older_commits() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "A").await;
    let b = t.commit("b.txt", "b\n", "B").await;
    let c = t.commit("c.txt", "c\n", "C").await;
    t.commit("d.txt", "d\n", "D").await;
    t.write("dirty.txt", "local edit\n");

    let reword = t.repo.reword(&b, "B renamed").await.unwrap();
    assert_eq!(t.subjects().await, ["D", "C", "B renamed", "A"]);
    assert!(t.exists("dirty.txt"));
    t.repo.undo(&reword.action).await.unwrap();
    assert_eq!(t.subjects().await, ["D", "C", "B", "A"]);

    let drop = t.repo.drop_commits(&HashSet::from([c])).await.unwrap();
    assert_eq!(t.subjects().await, ["D", "B", "A"]);
    assert!(!t.exists("c.txt"));
    t.repo.undo(&drop.action).await.unwrap();
    assert_eq!(t.subjects().await, ["D", "C", "B", "A"]);

    let shas: HashSet<String> = t
        .repo
        .log("HEAD", 0, 10)
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.subject == "B" || c.subject == "D")
        .map(|c| c.sha)
        .collect();
    t.repo.squash(&shas, "B and D").await.unwrap();
    assert_eq!(t.subjects().await, ["C", "B and D", "A"]);
    assert!(t.exists("d.txt") && t.exists("b.txt"));
}

#[tokio::test]
async fn reword_root_and_head() {
    let t = T::new().await;
    let root = t.commit("a.txt", "a\n", "Root").await;
    t.commit("b.txt", "b\n", "Head").await;
    t.repo.reword(&root, "Root renamed").await.unwrap();
    assert_eq!(t.subjects().await, ["Head", "Root renamed"]);
    let head = t.repo.head().await.unwrap();
    t.repo.reword(&head, "Head renamed").await.unwrap();
    assert_eq!(t.subjects().await, ["Head renamed", "Root renamed"]);
}

#[tokio::test]
async fn stash_round_trip_and_undo() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "A").await;
    t.write("a.txt", "changed\n");
    t.write("new.txt", "new\n");

    t.repo
        .stash_push(Some("try sliding window"), true, &[])
        .await
        .unwrap();
    assert!(t.repo.status().await.unwrap().is_clean());
    let stashes = t.repo.stashes().await.unwrap();
    assert_eq!(
        stashes
            .iter()
            .map(|s| s.message.as_str())
            .collect::<Vec<_>>(),
        ["try sliding window"]
    );

    let drop = t.repo.stash_drop(&stashes[0]).await.unwrap();
    assert!(t.repo.stashes().await.unwrap().is_empty());
    t.repo.undo(&drop.action).await.unwrap();
    let stashes = t.repo.stashes().await.unwrap();
    assert_eq!(stashes.len(), 1);

    let pop = t.repo.stash_apply(&stashes[0], true).await.unwrap();
    assert_eq!(t.read("a.txt"), "changed\n");
    assert!(t.exists("new.txt"));
    t.repo.undo(&pop.action).await.unwrap();
    assert_eq!(t.read("a.txt"), "a\n");
    assert!(!t.exists("new.txt"));
    assert_eq!(t.repo.stashes().await.unwrap().len(), 1);
}

#[tokio::test]
async fn hard_reset_undo_restores_local_changes() {
    let t = T::new().await;
    let a = t.commit("a.txt", "a\n", "A").await;
    t.commit("b.txt", "b\n", "B").await;
    t.write("a.txt", "edited\n");
    t.write("u.txt", "untracked\n");
    let undo = t.repo.reset(&a, ResetMode::Hard).await.unwrap();
    assert_eq!(t.subjects().await, ["A"]);
    assert_eq!(t.read("a.txt"), "a\n");
    t.repo.undo(&undo.action).await.unwrap();
    assert_eq!(t.subjects().await, ["B", "A"]);
    assert_eq!(t.read("a.txt"), "edited\n");
    assert_eq!(t.read("u.txt"), "untracked\n");
}

#[tokio::test]
async fn discard_undo_brings_back_untracked_files() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "A").await;
    t.write("a.txt", "b\n");
    t.write("u.txt", "u\n");
    let status = t.repo.status().await.unwrap();
    let changes: Vec<FileChange> = status
        .unstaged
        .into_iter()
        .chain(status.untracked)
        .collect();
    let undo = t.repo.discard(&changes).await.unwrap().unwrap();
    assert!(t.repo.status().await.unwrap().is_clean());
    t.repo.undo(&undo.action).await.unwrap();
    assert_eq!(t.read("a.txt"), "b\n");
    assert_eq!(t.read("u.txt"), "u\n");
}

#[tokio::test]
async fn branches_and_push_to_bare_remote() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "A").await;
    let remote = tempfile::tempdir().unwrap();
    let remote_path = remote.path().to_string_lossy().to_string();
    git(
        Path::new("/"),
        &["init", "-q", "--bare", &remote_path],
        RunOptions::default(),
    )
    .await
    .unwrap();
    t.repo
        .run(&["remote", "add", "origin", &remote_path])
        .await
        .unwrap();

    t.repo.create_branch("feature", None, true).await.unwrap();
    assert_eq!(t.repo.current_branch().await.as_deref(), Some("feature"));
    t.commit("b.txt", "b\n", "B").await;
    assert_eq!(t.repo.unpushed().await.len(), 2);

    t.repo.push(false, &|_| {}).await.unwrap();
    assert_eq!(
        t.repo.status().await.unwrap().branch.upstream.as_deref(),
        Some("origin/feature")
    );
    assert!(t.repo.unpushed().await.is_empty());

    t.repo.run(&["switch", "main"]).await.unwrap();
    let delete = t.repo.delete_branch("feature", true).await.unwrap();
    assert!(!t
        .repo
        .branches()
        .await
        .unwrap()
        .iter()
        .any(|b| b.name == "feature"));
    t.repo.undo(&delete.action).await.unwrap();
    assert!(t
        .repo
        .branches()
        .await
        .unwrap()
        .iter()
        .any(|b| b.name == "feature" && b.upstream.as_deref() == Some("origin/feature")));
}

#[tokio::test]
async fn branches_know_their_worktree() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "A").await;
    let tree = t.root.with_extension("tree");
    t.repo
        .run(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            tree.to_str().unwrap(),
        ])
        .await
        .unwrap();
    let branches = t.repo.branches().await.unwrap();
    let worktree = |name: &str| {
        branches
            .iter()
            .find(|b| b.name == name)
            .unwrap()
            .worktree
            .clone()
    };
    assert_eq!(worktree("side"), Some(tree.to_string_lossy().into_owned()));
    assert_eq!(worktree("main"), None);
    assert_eq!(
        Repo::new(&tree).main_worktree().await.unwrap(),
        Some(t.root.clone())
    );
    assert_eq!(t.repo.main_worktree().await.unwrap(), None);
    let _ = std::fs::remove_dir_all(&tree);
}

#[tokio::test]
async fn checkout_detached_and_undo() {
    let t = T::new().await;
    let first = t.commit("a.txt", "a\n", "A").await;
    t.repo.run(&["tag", "v1"]).await.unwrap();
    t.commit("a.txt", "b\n", "B").await;
    let detach = t.repo.checkout_detached("v1").await.unwrap();
    assert_eq!(t.repo.current_branch().await, None);
    assert_eq!(t.repo.head().await.as_deref(), Some(first.as_str()));
    t.repo.undo(&detach.action).await.unwrap();
    assert_eq!(t.repo.current_branch().await.as_deref(), Some("main"));
}

#[tokio::test]
async fn rebase_conflict_is_reported() {
    let t = T::new().await;
    t.commit("a.txt", "base\n", "A").await;
    let b = t.commit("a.txt", "from B\n", "B").await;
    t.commit("a.txt", "from C\n", "C").await;
    // Dropping B makes C conflict, because C's change was made on top of B.
    let err: GitError = t.repo.drop_commits(&HashSet::from([b])).await.unwrap_err();
    assert!(!err.message.is_empty());
    let op = t
        .repo
        .in_progress()
        .await
        .unwrap()
        .expect("rebase in progress");
    assert!(matches!(op, InProgress::Rebase { .. }));
    let conflicted: Vec<String> = t
        .repo
        .status()
        .await
        .unwrap()
        .conflicted
        .into_iter()
        .map(|c| c.path)
        .collect();
    assert_eq!(conflicted, ["a.txt"]);
    t.repo.abort(&op).await.unwrap();
    assert_eq!(t.repo.in_progress().await.unwrap(), None);
    assert_eq!(t.subjects().await, ["C", "B", "A"]);
}

#[tokio::test]
async fn revert_and_cherry_pick() {
    let t = T::new().await;
    t.commit("a.txt", "a\n", "A").await;
    t.repo.run(&["switch", "-q", "-c", "side"]).await.unwrap();
    let s = t.commit("s.txt", "s\n", "Side").await;
    t.repo.run(&["switch", "-q", "main"]).await.unwrap();

    let pick = t.repo.cherry_pick(&[s]).await.unwrap();
    assert_eq!(t.subjects().await, ["Side", "A"]);
    t.repo.undo(&pick.action).await.unwrap();
    assert_eq!(t.subjects().await, ["A"]);

    let b = t.commit("b.txt", "b\n", "B").await;
    t.repo.revert(&b).await.unwrap();
    assert_eq!(t.subjects().await[0], "Revert \"B\"");
    assert!(!t.exists("b.txt"));
}

#[tokio::test]
async fn ops_history_undo_redo() {
    use seam_lib::git::ops::{perform, History, Op};
    let t = T::new().await;
    t.commit("a.txt", "a\n", "A").await;
    let mut history = History::default();
    let op = Op::Commit {
        message: "Empty".into(),
        amend: false,
    };
    t.repo
        .run(&["commit", "--allow-empty", "-q", "-m", "seed"])
        .await
        .unwrap();
    t.write("b.txt", "b\n");
    let (u, _) = perform(&t.repo, &Op::StageAll, &|_| {}).await.unwrap();
    history.record(Op::StageAll, u);
    let (u, _) = perform(&t.repo, &op, &|_| {}).await.unwrap();
    history.record(op, u);
    assert_eq!(t.subjects().await[0], "Empty");

    let (op, undo) = history.pop_undo().unwrap();
    t.repo.undo(&undo.action).await.unwrap();
    history.push_redo(op);
    assert_eq!(t.subjects().await[0], "seed");

    let op = history.pop_redo().unwrap();
    let (u, _) = perform(&t.repo, &op, &|_| {}).await.unwrap();
    history.record_redo(op, u);
    assert_eq!(t.subjects().await[0], "Empty");
}

fn blob_ids(diff: &FileDiff) -> (Option<String>, Option<String>) {
    let line = diff
        .header_lines
        .iter()
        .find_map(|line| line.strip_prefix("index "))
        .unwrap();
    let range = line.split(' ').next().unwrap();
    let (before, after) = range.split_once("..").unwrap();
    let present = |id: &str| (!id.chars().all(|c| c == '0')).then(|| id.to_string());
    (present(before), present(after))
}

#[tokio::test]
async fn blob_reads_both_sides_of_a_binary_diff() {
    let t = T::new().await;
    let old = b"\x89PNG\0old".to_vec();
    let new = b"\x89PNG\0new".to_vec();
    std::fs::write(t.root.join("img.png"), &old).unwrap();
    t.repo.run(&["add", "--", "img.png"]).await.unwrap();
    t.repo.run(&["commit", "-q", "-m", "Image"]).await.unwrap();

    std::fs::write(t.root.join("img.png"), &new).unwrap();
    let diff = t
        .repo
        .diff("img.png", DiffSource::Unstaged, false)
        .await
        .unwrap()
        .unwrap();
    assert!(diff.is_binary);
    let (before, after) = blob_ids(&diff);
    assert_eq!(t.repo.blob(&before.unwrap(), None).await.unwrap(), old);
    let after = after.unwrap();
    assert!(t.repo.blob(&after, None).await.is_err());
    assert_eq!(t.repo.blob(&after, Some("img.png")).await.unwrap(), new);

    t.repo.run(&["add", "--", "img.png"]).await.unwrap();
    let staged = t
        .repo
        .diff("img.png", DiffSource::Staged, false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        t.repo
            .blob(&blob_ids(&staged).1.unwrap(), None)
            .await
            .unwrap(),
        new
    );

    std::fs::write(t.root.join("new.png"), &old).unwrap();
    let untracked = t
        .repo
        .diff("new.png", DiffSource::Untracked, false)
        .await
        .unwrap()
        .unwrap();
    let (before, after) = blob_ids(&untracked);
    assert!(before.is_none());
    assert_eq!(
        t.repo.blob(&after.unwrap(), Some("new.png")).await.unwrap(),
        old
    );
}
