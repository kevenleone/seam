use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::*;
use super::parse;
use super::patch::{self, Direction, Selection};
use super::process::{git, GitError, Output, Result, RunOptions};
use super::rewrite::{self, Step};

pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// How to reverse a git action. Mutating methods return one when the action can be undone.
#[derive(Debug, Clone)]
pub struct Undoable {
    pub title: String,
    pub action: UndoAction,
}

#[derive(Debug, Clone)]
pub enum UndoAction {
    /// Put the index back to a tree from `git write-tree`.
    ReadTree(String),
    /// Move the branch back, keeping the index (undoing commits).
    ResetSoft {
        to: String,
        backup: Option<String>,
    },
    /// Move the branch back, keeping local edits (undoing rewrites, pulls, cherry-picks).
    ResetKeep {
        to: String,
        backup: String,
    },
    /// Undo the very first commit: delete the branch ref, restore the index.
    UnbornBranch {
        branch_ref: String,
        index: Option<String>,
    },
    RestoreFiles(Vec<FileSnapshot>),
    /// Reset to `reset_to` (or HEAD), then put the index and changed files back.
    RestoreWorkingCopy {
        reset_to: Option<String>,
        snapshot: WorkingCopySnapshot,
    },
    /// Run these git commands in order.
    Git(Vec<Vec<String>>),
    Sequence(Vec<UndoAction>),
}

#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub path: String,
    /// None: the file did not exist.
    pub blob: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkingCopySnapshot {
    pub index: Option<String>,
    pub files: Vec<FileSnapshot>,
}

fn undoable(title: &str, action: UndoAction) -> Undoable {
    Undoable {
        title: title.to_string(),
        action,
    }
}

fn cmd(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

/// One git repository, driven through the `git` command line.
#[derive(Debug, Clone)]
pub struct Repo {
    pub root: PathBuf,
}

impl Repo {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Repo { root: root.into() }
    }

    // MARK: Running git

    pub async fn run(&self, args: &[&str]) -> Result<Output> {
        git(&self.root, args, RunOptions::default()).await
    }

    pub async fn run_with(&self, args: &[&str], opts: RunOptions<'_>) -> Result<Output> {
        git(&self.root, args, opts).await
    }

    async fn read(&self, args: &[&str]) -> Result<String> {
        Ok(git(
            &self.root,
            args,
            RunOptions {
                read: true,
                ..Default::default()
            },
        )
        .await?
        .text())
    }

    async fn line(&self, args: &[&str]) -> Result<String> {
        Ok(self.read(args).await?.trim().to_string())
    }

    async fn try_line(&self, args: &[&str]) -> Option<String> {
        let out = git(
            &self.root,
            args,
            RunOptions {
                read: true,
                allow_failure: true,
                ..Default::default()
            },
        )
        .await
        .ok()?;
        let text = out.text().trim().to_string();
        (out.code == 0 && !text.is_empty()).then_some(text)
    }

    // MARK: Discovery

    /// The top level of the work tree containing `path`, or None outside a repository.
    pub async fn top_level(path: &Path) -> Option<PathBuf> {
        let out = git(
            path,
            &["rev-parse", "--show-toplevel"],
            RunOptions {
                read: true,
                allow_failure: true,
                ..Default::default()
            },
        )
        .await
        .ok()?;
        let text = out.text().trim().to_string();
        (out.code == 0 && !text.is_empty()).then(|| PathBuf::from(text))
    }

    pub async fn init(path: &Path) -> Result<()> {
        git(path, &["init"], RunOptions::default())
            .await
            .map(|_| ())
    }

    pub async fn clone_into(
        url: &str,
        destination: &Path,
        progress: &(dyn Fn(String) + Send + Sync),
    ) -> Result<()> {
        let parent = destination
            .parent()
            .ok_or_else(|| GitError::new("Choose a folder to clone into."))?;
        let dest = destination.to_string_lossy();
        git(
            parent,
            &["clone", "--progress", url, &dest],
            RunOptions {
                progress: Some(progress),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    pub async fn git_dir(&self) -> Result<PathBuf> {
        Ok(PathBuf::from(
            self.line(&["rev-parse", "--absolute-git-dir"]).await?,
        ))
    }

    // MARK: Status and diffs

    pub async fn status(&self) -> Result<RepoStatus> {
        let out = self
            .run_with(
                &[
                    "status",
                    "--porcelain=v2",
                    "-z",
                    "--branch",
                    "--untracked-files=all",
                ],
                RunOptions {
                    read: true,
                    ..Default::default()
                },
            )
            .await?;
        Ok(parse::status(&out.stdout))
    }

    pub async fn in_progress(&self) -> Result<Option<InProgress>> {
        let dir = self.git_dir().await?;
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name))
                .ok()
                .map(|s| s.trim().to_string())
        };
        for folder in ["rebase-merge", "rebase-apply"] {
            if dir.join(folder).exists() {
                let num = |a: &str, b: &str| {
                    read(&format!("{folder}/{a}"))
                        .or_else(|| read(&format!("{folder}/{b}")))
                        .and_then(|s| s.parse().ok())
                };
                return Ok(Some(InProgress::Rebase {
                    step: num("msgnum", "next"),
                    total: num("end", "last"),
                    head_name: read(&format!("{folder}/head-name"))
                        .map(|s| s.trim_start_matches("refs/heads/").to_string()),
                }));
            }
        }
        if dir.join("MERGE_HEAD").exists() {
            return Ok(Some(InProgress::Merge));
        }
        if dir.join("CHERRY_PICK_HEAD").exists() {
            return Ok(Some(InProgress::CherryPick));
        }
        if dir.join("REVERT_HEAD").exists() {
            return Ok(Some(InProgress::Revert));
        }
        Ok(None)
    }

    /// A blob's bytes by the (possibly abbreviated) id a diff's `index` line names.
    pub async fn blob(&self, object: &str, path: Option<&str>) -> Result<Vec<u8>> {
        let read = RunOptions {
            read: true,
            allow_failure: true,
            ..Default::default()
        };
        let spec = format!("{object}^{{blob}}");
        let exists = self.run_with(&["cat-file", "-e", &spec], read).await?;
        if exists.code == 0 {
            return Ok(self
                .run_with(
                    &["cat-file", "blob", &spec],
                    RunOptions {
                        read: true,
                        ..Default::default()
                    },
                )
                .await?
                .stdout);
        }
        // An unstaged or untracked diff names the working-tree file's hash without writing the object.
        if let Some(path) = path {
            let hash = self
                .try_line(&["hash-object", "--no-filters", "--", path])
                .await;
            if hash.is_some_and(|hash| hash.starts_with(object)) {
                return Ok(tokio::fs::read(self.root.join(path)).await?);
            }
        }
        Err(GitError::new(format!("No object {object}")))
    }

    pub async fn diff(
        &self,
        path: &str,
        source: DiffSource,
        ignore_whitespace: bool,
    ) -> Result<Option<FileDiff>> {
        let mut args = vec!["diff", "--no-color", "--no-ext-diff", "-M", "-U3"];
        if ignore_whitespace {
            args.push("-w");
        }
        match source {
            DiffSource::Unstaged => args.extend(["--", path]),
            DiffSource::Staged => args.extend(["--cached", "--", path]),
            DiffSource::Untracked => args.extend(["--no-index", "--", "/dev/null", path]),
        }
        // --no-index exits 1 when the files differ.
        let out = self
            .run_with(
                &args,
                RunOptions {
                    read: true,
                    allow_failure: source == DiffSource::Untracked,
                    ..Default::default()
                },
            )
            .await?;
        if source == DiffSource::Untracked && out.code > 1 {
            return Err(GitError::new(out.stderr.trim().to_string()));
        }
        Ok(parse::diff(&out.text()).into_iter().next())
    }

    /// A commit against its first parent (or the empty tree for a root commit).
    pub async fn commit_diff(&self, sha: &str, ignore_whitespace: bool) -> Result<Vec<FileDiff>> {
        let mut args = vec![
            "show",
            "--format=",
            "--no-color",
            "--no-ext-diff",
            "-M",
            "--first-parent",
        ];
        if ignore_whitespace {
            args.push("-w");
        }
        args.push(sha);
        Ok(parse::diff(&self.read(&args).await?))
    }

    /// Everything from the parent of `oldest` to `newest`.
    pub async fn range_diff(&self, oldest: &str, newest: &str) -> Result<Vec<FileDiff>> {
        let parents = self.parents(oldest).await?;
        let base = parents.first().map(String::as_str).unwrap_or(EMPTY_TREE);
        Ok(parse::diff(
            &self
                .read(&["diff", "--no-color", "--no-ext-diff", "-M", base, newest])
                .await?,
        ))
    }

    pub async fn apply_patch(&self, patch: &str, cached: bool, reverse: bool) -> Result<()> {
        let mut args = vec!["apply", "--recount", "--whitespace=nowarn"];
        if cached {
            args.push("--cached");
        }
        if reverse {
            args.push("--reverse");
        }
        args.push("-");
        self.run_with(
            &args,
            RunOptions {
                stdin: Some(patch),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    // MARK: Staging

    pub async fn stage(&self, paths: &[String]) -> Result<Option<Undoable>> {
        if paths.is_empty() {
            return Ok(None);
        }
        let undo = self.index_undo("Stage").await;
        let mut args = vec!["add", "-A", "--"];
        args.extend(paths.iter().map(String::as_str));
        self.run(&args).await?;
        Ok(undo)
    }

    pub async fn stage_all(&self) -> Result<Option<Undoable>> {
        let undo = self.index_undo("Stage all").await;
        self.run(&["add", "-A"]).await?;
        Ok(undo)
    }

    pub async fn unstage(&self, paths: &[String]) -> Result<Option<Undoable>> {
        if paths.is_empty() {
            return Ok(None);
        }
        let undo = self.index_undo("Unstage").await;
        let mut args: Vec<&str> = if self.head().await.is_none() {
            vec!["rm", "--cached", "-r", "-q", "--"]
        } else {
            vec!["reset", "-q", "HEAD", "--"]
        };
        args.extend(paths.iter().map(String::as_str));
        self.run(&args).await?;
        Ok(undo)
    }

    pub async fn unstage_all(&self) -> Result<Option<Undoable>> {
        let undo = self.index_undo("Unstage all").await;
        if self.head().await.is_none() {
            self.run(&["rm", "--cached", "-r", "-q", "."]).await?;
        } else {
            self.run(&["reset", "-q"]).await?;
        }
        Ok(undo)
    }

    /// Stage, unstage or discard some lines. `selection` None means the whole file.
    pub async fn apply_lines(
        &self,
        file: &FileDiff,
        selection: Option<&Selection>,
        action: LineAction,
    ) -> Result<Option<Undoable>> {
        let direction = if action == LineAction::Stage {
            Direction::Forward
        } else {
            Direction::Reverse
        };
        let Some(patch) = patch::build(file, selection, direction) else {
            return Ok(None);
        };
        match action {
            LineAction::Stage | LineAction::Unstage => {
                let undo = self
                    .index_undo(if action == LineAction::Stage {
                        "Stage lines"
                    } else {
                        "Unstage lines"
                    })
                    .await;
                self.apply_patch(&patch, true, action == LineAction::Unstage)
                    .await?;
                Ok(undo)
            }
            LineAction::Discard => {
                let snapshot = self.snapshot_files(&[file.path().to_string()]).await?;
                self.apply_patch(&patch, false, true).await?;
                Ok(Some(undoable(
                    "Discard lines",
                    UndoAction::RestoreFiles(snapshot),
                )))
            }
        }
    }

    /// Throws away work-tree changes (and untracked files).
    pub async fn discard(&self, changes: &[FileChange]) -> Result<Option<Undoable>> {
        if changes.is_empty() {
            return Ok(None);
        }
        let mut paths: Vec<String> = changes.iter().map(|c| c.path.clone()).collect();
        paths.sort();
        paths.dedup();
        let snapshot = self.snapshot_files(&paths).await?;
        let untracked: HashSet<&str> = changes
            .iter()
            .filter(|c| c.area == ChangeArea::Untracked)
            .map(|c| c.path.as_str())
            .collect();
        let tracked: Vec<&str> = paths
            .iter()
            .map(String::as_str)
            .filter(|p| !untracked.contains(p))
            .collect();
        if !tracked.is_empty() {
            let mut args = vec!["restore", "--worktree", "--"];
            args.extend(tracked);
            self.run(&args).await?;
        }
        for path in untracked {
            let _ = std::fs::remove_file(self.root.join(path));
        }
        Ok(Some(undoable(
            "Discard changes",
            UndoAction::RestoreFiles(snapshot),
        )))
    }

    /// Marks a conflicted file resolved, optionally taking one side wholesale first.
    pub async fn resolve_conflict(
        &self,
        path: &str,
        side: Option<ConflictSide>,
    ) -> Result<Option<Undoable>> {
        let snapshot = self.snapshot_files(&[path.to_string()]).await?;
        if let Some(side) = side {
            self.run(&[
                "checkout",
                if side == ConflictSide::Ours {
                    "--ours"
                } else {
                    "--theirs"
                },
                "--",
                path,
            ])
            .await?;
        }
        self.run(&["add", "--", path]).await?;
        Ok(Some(undoable(
            "Resolve conflict",
            UndoAction::Sequence(vec![
                // Put the conflict back in the index, then the file as it was.
                UndoAction::Git(vec![cmd(&["checkout", "-m", "--", path])]),
                UndoAction::RestoreFiles(snapshot),
            ]),
        )))
    }

    // MARK: Committing

    pub async fn head(&self) -> Option<String> {
        self.try_line(&["rev-parse", "--verify", "-q", "HEAD"])
            .await
    }

    pub async fn last_commit(&self) -> Result<Option<Commit>> {
        if self.head().await.is_none() {
            return Ok(None);
        }
        Ok(self.log("HEAD", 0, 1).await?.into_iter().next())
    }

    pub async fn commit(&self, message: &str, amend: bool) -> Result<Undoable> {
        let old = self.head().await;
        let index = self.try_line(&["write-tree"]).await;
        let mut args = vec!["commit", "-F", "-", "--cleanup=strip"];
        if amend {
            args.push("--amend");
        }
        self.run_with(
            &args,
            RunOptions {
                stdin: Some(message),
                ..Default::default()
            },
        )
        .await?;
        let title = if amend { "Amend commit" } else { "Commit" };
        Ok(match old {
            Some(old) => undoable(
                title,
                UndoAction::ResetSoft {
                    to: old,
                    backup: None,
                },
            ),
            None => {
                let branch_ref = self.line(&["symbolic-ref", "HEAD"]).await?;
                undoable(title, UndoAction::UnbornBranch { branch_ref, index })
            }
        })
    }

    /// Undoes the last commit, keeping its changes staged.
    pub async fn uncommit(&self) -> Result<Undoable> {
        let head = self
            .head()
            .await
            .ok_or_else(|| GitError::new("There is no commit to undo."))?;
        let backup = self.save_backup_ref(&head).await?;
        match self.parents(&head).await?.first() {
            Some(parent) => {
                self.run(&["reset", "--soft", parent]).await?;
            }
            None => {
                let branch = self.line(&["symbolic-ref", "HEAD"]).await?;
                self.run(&["update-ref", "-d", &branch]).await?;
            }
        }
        Ok(undoable(
            "Undo commit",
            UndoAction::ResetSoft {
                to: head,
                backup: Some(backup),
            },
        ))
    }

    // MARK: History

    pub async fn log(&self, reference: &str, skip: usize, limit: usize) -> Result<Vec<Commit>> {
        if reference == "HEAD" && self.head().await.is_none() {
            return Ok(vec![]);
        }
        let format = format!("--format={}", parse::log_format());
        let skip = format!("--skip={skip}");
        let limit = limit.to_string();
        Ok(parse::log(
            &self
                .read(&[
                    "log",
                    &format,
                    "--topo-order",
                    &skip,
                    "-n",
                    &limit,
                    reference,
                ])
                .await?,
        ))
    }

    pub async fn commit_files(&self, sha: &str) -> Result<Vec<CommitFileStat>> {
        let out = self
            .run_with(
                &[
                    "show",
                    "--format=",
                    "--numstat",
                    "-z",
                    "-M",
                    "--first-parent",
                    sha,
                ],
                RunOptions {
                    read: true,
                    ..Default::default()
                },
            )
            .await?;
        Ok(parse::numstat(&out.stdout))
    }

    pub async fn parents(&self, sha: &str) -> Result<Vec<String>> {
        let out = self
            .line(&["rev-list", "--parents", "-n", "1", sha])
            .await?;
        Ok(out.split(' ').skip(1).map(String::from).collect())
    }

    pub async fn merge_base(&self, a: &str, b: &str) -> Option<String> {
        self.try_line(&["merge-base", a, b]).await
    }

    /// Commits on HEAD the upstream doesn't have; without an upstream, those on no remote.
    pub async fn unpushed(&self) -> HashSet<String> {
        let out = match self
            .try_line(&["rev-parse", "--abbrev-ref", "@{upstream}"])
            .await
        {
            Some(_) => self.try_line(&["rev-list", "@{upstream}..HEAD"]).await,
            None => {
                self.try_line(&["rev-list", "HEAD", "--not", "--remotes"])
                    .await
            }
        };
        out.map(|s| s.lines().map(String::from).collect())
            .unwrap_or_default()
    }

    // MARK: Refs

    pub async fn branches(&self) -> Result<Vec<Branch>> {
        let format = format!("--format={}", parse::branch_format());
        Ok(parse::branches(
            &self
                .read(&[
                    "for-each-ref",
                    &format,
                    "--sort=-committerdate",
                    "refs/heads",
                    "refs/remotes",
                ])
                .await?,
        ))
    }

    pub async fn tags(&self) -> Result<Vec<Tag>> {
        let format = format!("--format={}", parse::tag_format());
        Ok(parse::tags(
            &self
                .read(&["for-each-ref", &format, "--sort=-creatordate", "refs/tags"])
                .await?,
        ))
    }

    pub async fn remotes(&self) -> Result<Vec<Remote>> {
        let mut seen = HashSet::new();
        Ok(self
            .read(&["remote", "-v"])
            .await?
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let (name, url) = (parts.next()?, parts.next()?);
                seen.insert(name.to_string()).then(|| Remote {
                    name: name.into(),
                    url: url.into(),
                })
            })
            .collect())
    }

    pub async fn current_branch(&self) -> Option<String> {
        self.try_line(&["symbolic-ref", "--short", "-q", "HEAD"])
            .await
    }

    pub async fn checkout(&self, branch: &str) -> Result<Undoable> {
        let previous = self.current_branch().await;
        let previous_sha = self.head().await;
        let is_remote = self
            .branches()
            .await?
            .iter()
            .any(|b| b.is_remote && b.name == branch);
        if is_remote {
            // Checking out a remote branch creates a local tracking branch.
            self.run(&["switch", "--track", branch]).await?;
        } else {
            self.run(&["switch", branch]).await?;
        }
        let back = match (previous, previous_sha) {
            (Some(p), _) => vec![cmd(&["switch", &p])],
            (None, Some(sha)) => vec![cmd(&["switch", "--detach", &sha])],
            _ => vec![],
        };
        Ok(undoable("Switch branch", UndoAction::Git(back)))
    }

    pub async fn create_branch(
        &self,
        name: &str,
        start: Option<&str>,
        checkout: bool,
    ) -> Result<Undoable> {
        let previous = self.current_branch().await;
        let mut args = if checkout {
            vec!["switch", "-c", name]
        } else {
            vec!["branch", name]
        };
        if let Some(start) = start {
            args.push(start);
        }
        self.run(&args).await?;
        let mut back = Vec::new();
        if let (true, Some(p)) = (checkout, previous) {
            back.push(cmd(&["switch", &p]));
        }
        back.push(cmd(&["branch", "-D", name]));
        Ok(undoable("Create branch", UndoAction::Git(back)))
    }

    pub async fn delete_branch(&self, name: &str, force: bool) -> Result<Undoable> {
        let sha = self
            .line(&["rev-parse", "--verify", &format!("refs/heads/{name}")])
            .await?;
        let upstream = self
            .try_line(&["rev-parse", "--abbrev-ref", &format!("{name}@{{upstream}}")])
            .await;
        self.run(&["branch", if force { "-D" } else { "-d" }, name])
            .await?;
        let mut back = vec![cmd(&["branch", name, &sha])];
        if let Some(up) = upstream {
            back.push(cmd(&["branch", &format!("--set-upstream-to={up}"), name]));
        }
        Ok(undoable("Delete branch", UndoAction::Git(back)))
    }

    pub async fn rename_branch(&self, old: &str, new: &str) -> Result<Undoable> {
        self.run(&["branch", "-m", old, new]).await?;
        Ok(undoable(
            "Rename branch",
            UndoAction::Git(vec![cmd(&["branch", "-m", new, old])]),
        ))
    }

    pub async fn create_tag(
        &self,
        name: &str,
        sha: &str,
        message: Option<&str>,
    ) -> Result<Undoable> {
        match message.filter(|m| !m.is_empty()) {
            Some(m) => {
                self.run_with(
                    &["tag", "-a", name, sha, "-F", "-"],
                    RunOptions {
                        stdin: Some(m),
                        ..Default::default()
                    },
                )
                .await?
            }
            None => self.run(&["tag", name, sha]).await?,
        };
        Ok(undoable(
            "Create tag",
            UndoAction::Git(vec![cmd(&["tag", "-d", name])]),
        ))
    }

    pub async fn delete_tag(&self, name: &str) -> Result<Undoable> {
        let object = self
            .line(&["rev-parse", &format!("refs/tags/{name}")])
            .await?;
        self.run(&["tag", "-d", name]).await?;
        Ok(undoable(
            "Delete tag",
            UndoAction::Git(vec![cmd(&[
                "update-ref",
                &format!("refs/tags/{name}"),
                &object,
            ])]),
        ))
    }

    // MARK: Stashes

    pub async fn stashes(&self) -> Result<Vec<Stash>> {
        let format = format!("--format={}", parse::stash_format());
        let out = self
            .run_with(
                &["stash", "list", &format],
                RunOptions {
                    read: true,
                    allow_failure: true,
                    ..Default::default()
                },
            )
            .await?;
        Ok(parse::stashes(&out.text()))
    }

    pub async fn stash_push(
        &self,
        message: Option<&str>,
        include_untracked: bool,
        paths: &[String],
    ) -> Result<Undoable> {
        let mut args = vec!["stash", "push"];
        if include_untracked {
            args.push("--include-untracked");
        }
        if let Some(m) = message.filter(|m| !m.is_empty()) {
            args.extend(["-m", m]);
        }
        if !paths.is_empty() {
            args.push("--");
            args.extend(paths.iter().map(String::as_str));
        }
        self.run(&args).await?;
        Ok(undoable(
            "Stash changes",
            UndoAction::Git(vec![cmd(&["stash", "pop", "--index"])]),
        ))
    }

    pub async fn stash_apply(&self, stash: &Stash, pop: bool) -> Result<Undoable> {
        let snapshot = self.working_copy_snapshot().await?;
        self.run(&[
            "stash",
            if pop { "pop" } else { "apply" },
            "--index",
            &stash.reference(),
        ])
        .await?;
        let mut steps = vec![UndoAction::RestoreWorkingCopy {
            reset_to: None,
            snapshot,
        }];
        if pop {
            steps.push(UndoAction::Git(vec![cmd(&[
                "stash",
                "store",
                "-m",
                &stash.message,
                &stash.sha,
            ])]));
        }
        Ok(undoable(
            if pop { "Pop stash" } else { "Apply stash" },
            UndoAction::Sequence(steps),
        ))
    }

    pub async fn stash_drop(&self, stash: &Stash) -> Result<Undoable> {
        self.run(&["stash", "drop", &stash.reference()]).await?;
        Ok(undoable(
            "Drop stash",
            UndoAction::Git(vec![cmd(&[
                "stash",
                "store",
                "-m",
                &stash.message,
                &stash.sha,
            ])]),
        ))
    }

    pub async fn stash_diff(&self, stash: &Stash) -> Result<Vec<FileDiff>> {
        let r = stash.reference();
        let out = self
            .run_with(
                &[
                    "stash",
                    "show",
                    "-p",
                    "--include-untracked",
                    "--no-color",
                    &r,
                ],
                RunOptions {
                    read: true,
                    allow_failure: true,
                    ..Default::default()
                },
            )
            .await?;
        if out.code != 0 {
            return Ok(parse::diff(
                &self
                    .read(&["stash", "show", "-p", "--no-color", &r])
                    .await?,
            ));
        }
        Ok(parse::diff(&out.text()))
    }

    // MARK: Rewriting

    pub async fn reset(&self, sha: &str, mode: ResetMode) -> Result<Undoable> {
        let old = self
            .head()
            .await
            .ok_or_else(|| GitError::new("Nothing to reset yet."))?;
        let snapshot = if mode == ResetMode::Hard {
            Some(self.working_copy_snapshot().await?)
        } else {
            None
        };
        let flag = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        };
        self.run(&["reset", flag, sha]).await?;
        let backup = self.save_backup_ref(&old).await?;
        Ok(match snapshot {
            Some(snapshot) => undoable(
                "Reset branch",
                UndoAction::RestoreWorkingCopy {
                    reset_to: Some(old),
                    snapshot,
                },
            ),
            None => undoable(
                "Reset branch",
                UndoAction::ResetSoft {
                    to: old,
                    backup: Some(backup),
                },
            ),
        })
    }

    pub async fn revert(&self, sha: &str) -> Result<Undoable> {
        let undo = self.head_undo("Revert commit").await?;
        let mut args = vec!["revert", "--no-edit"];
        if self.parents(sha).await?.len() > 1 {
            args.extend(["-m", "1"]);
        }
        args.push(sha);
        self.run(&args).await?;
        Ok(undo)
    }

    pub async fn cherry_pick(&self, shas: &[String]) -> Result<Undoable> {
        let undo = self.head_undo("Cherry-pick").await?;
        let mut args = vec!["cherry-pick"];
        args.extend(shas.iter().map(String::as_str));
        self.run(&args).await?;
        Ok(undo)
    }

    pub async fn reword(&self, sha: &str, message: &str) -> Result<Undoable> {
        if self.head().await.as_deref() == Some(sha) {
            let undo = self.head_undo("Rename commit").await?;
            self.run_with(
                &[
                    "commit",
                    "--amend",
                    "--only",
                    "--no-verify",
                    "--cleanup=strip",
                    "-F",
                    "-",
                ],
                RunOptions {
                    stdin: Some(message),
                    ..Default::default()
                },
            )
            .await?;
            return Ok(undo);
        }
        let undo = self.head_undo("Rename commit").await?;
        rewrite::rewrite(self, sha, |c| {
            if c == sha {
                Step::Reword(message.to_string())
            } else {
                Step::Pick
            }
        })
        .await?;
        Ok(undo)
    }

    pub async fn drop_commits(&self, shas: &HashSet<String>) -> Result<Undoable> {
        let oldest = self.oldest_of(shas).await?;
        let undo = self
            .head_undo(if shas.len() == 1 {
                "Remove commit"
            } else {
                "Remove commits"
            })
            .await?;
        rewrite::rewrite(self, &oldest, |c| {
            if shas.contains(c) {
                Step::Drop
            } else {
                Step::Pick
            }
        })
        .await?;
        Ok(undo)
    }

    /// Folds `shas` into the oldest of them, with `message` as the new message.
    pub async fn squash(&self, shas: &HashSet<String>, message: &str) -> Result<Undoable> {
        let oldest = self.oldest_of(shas).await?;
        let undo = self.head_undo("Squash commits").await?;
        rewrite::squash(self, &oldest, shas, message).await?;
        Ok(undo)
    }

    async fn oldest_of(&self, shas: &HashSet<String>) -> Result<String> {
        // rev-list lists newest first; the last one in the set is the oldest.
        self.read(&["rev-list", "--topo-order", "HEAD"])
            .await?
            .lines()
            .rfind(|l| shas.contains(*l))
            .map(String::from)
            .ok_or_else(|| GitError::new("Those commits are not on the current branch."))
    }

    // MARK: In-progress operations

    pub async fn abort(&self, op: &InProgress) -> Result<()> {
        self.run(&[op.command(), "--abort"]).await.map(|_| ())
    }

    pub async fn continue_op(&self, op: &InProgress) -> Result<()> {
        // Never open an editor for the message: keep git's prepared one.
        self.run_with(
            &[op.command(), "--continue"],
            RunOptions {
                env: vec![("GIT_EDITOR", "true".into())],
                ..Default::default()
            },
        )
        .await
        .map(|_| ())
    }

    pub async fn skip(&self, op: &InProgress) -> Result<()> {
        self.run(&[op.command(), "--skip"]).await.map(|_| ())
    }

    // MARK: Remotes

    pub async fn fetch(&self, progress: &(dyn Fn(String) + Send + Sync)) -> Result<()> {
        self.run_with(
            &["fetch", "--all", "--prune", "--progress"],
            RunOptions {
                progress: Some(progress),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    pub async fn pull(
        &self,
        mode: PullMode,
        progress: &(dyn Fn(String) + Send + Sync),
    ) -> Result<Undoable> {
        let undo = self.head_undo("Pull").await?;
        let flag = match mode {
            PullMode::FastForwardOnly => "--ff-only",
            PullMode::Rebase => "--rebase",
            PullMode::Merge => "--no-rebase",
        };
        self.run_with(
            &["pull", flag, "--progress"],
            RunOptions {
                progress: Some(progress),
                ..Default::default()
            },
        )
        .await?;
        Ok(undo)
    }

    pub async fn push(&self, force: bool, progress: &(dyn Fn(String) + Send + Sync)) -> Result<()> {
        let mut args: Vec<String> = cmd(&["push", "--progress"]);
        if force {
            args.push("--force-with-lease".into());
        }
        if self
            .try_line(&["rev-parse", "--abbrev-ref", "@{upstream}"])
            .await
            .is_none()
        {
            let branch = self
                .current_branch()
                .await
                .ok_or_else(|| GitError::new("Switch to a branch before pushing."))?;
            let remotes = self.remotes().await?;
            let remote = remotes
                .iter()
                .find(|r| r.name == "origin")
                .or(remotes.first())
                .ok_or_else(|| GitError::new("This repository has no remote to push to."))?;
            args.extend(["-u".into(), remote.name.clone(), branch]);
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run_with(
            &args,
            RunOptions {
                progress: Some(progress),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    // MARK: Undo support

    /// Undo by moving the current branch back to where HEAD is now, keeping local edits.
    pub async fn head_undo(&self, title: &str) -> Result<Undoable> {
        match self.head().await {
            Some(old) => {
                let backup = self.save_backup_ref(&old).await?;
                Ok(undoable(title, UndoAction::ResetKeep { to: old, backup }))
            }
            None => Ok(undoable(title, UndoAction::Git(vec![]))),
        }
    }

    /// Keeps `sha` reachable so `git gc` can't collect it before the user undoes.
    async fn save_backup_ref(&self, sha: &str) -> Result<String> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let reference = format!("refs/seam/undo/{millis}-{}", &sha[..8.min(sha.len())]);
        self.run(&["update-ref", &reference, sha]).await?;
        Ok(reference)
    }

    async fn index_undo(&self, title: &str) -> Option<Undoable> {
        let tree = self.try_line(&["write-tree"]).await?;
        Some(undoable(title, UndoAction::ReadTree(tree)))
    }

    async fn snapshot_files(&self, paths: &[String]) -> Result<Vec<FileSnapshot>> {
        let mut out = Vec::new();
        for path in paths {
            let full = self.root.join(path);
            let blob = if full.is_file() {
                Some(
                    self.line(&["hash-object", "-w", "--no-filters", "--", path])
                        .await?,
                )
            } else {
                None
            };
            out.push(FileSnapshot {
                path: path.clone(),
                blob,
            });
        }
        Ok(out)
    }

    async fn restore_files(&self, files: &[FileSnapshot]) -> Result<()> {
        for file in files {
            let full = self.root.join(&file.path);
            match &file.blob {
                Some(blob) => {
                    let data = self
                        .run_with(
                            &["cat-file", "blob", blob],
                            RunOptions {
                                read: true,
                                ..Default::default()
                            },
                        )
                        .await?
                        .stdout;
                    if let Some(dir) = full.parent() {
                        std::fs::create_dir_all(dir)?;
                    }
                    std::fs::write(&full, data)?;
                }
                None => {
                    let _ = std::fs::remove_file(&full);
                }
            }
        }
        Ok(())
    }

    /// The index plus the contents of every changed or untracked file.
    async fn working_copy_snapshot(&self) -> Result<WorkingCopySnapshot> {
        let status = self.status().await?;
        let mut paths: Vec<String> = status
            .staged
            .iter()
            .chain(&status.unstaged)
            .chain(&status.untracked)
            .flat_map(|c| std::iter::once(c.path.clone()).chain(c.original_path.clone()))
            .collect();
        paths.sort();
        paths.dedup();
        let index = self.try_line(&["write-tree"]).await;
        Ok(WorkingCopySnapshot {
            index,
            files: self.snapshot_files(&paths).await?,
        })
    }

    /// Reverses an action described by `undo`.
    pub async fn undo(&self, action: &UndoAction) -> Result<()> {
        match action {
            UndoAction::ReadTree(tree) => {
                self.run(&["read-tree", tree]).await?;
            }
            UndoAction::ResetSoft { to, backup } => {
                self.run(&["reset", "--soft", to]).await?;
                if let Some(b) = backup {
                    let _ = self.run(&["update-ref", "-d", b]).await;
                }
            }
            UndoAction::ResetKeep { to, backup } => {
                self.run(&["reset", "--keep", to]).await?;
                let _ = self.run(&["update-ref", "-d", backup]).await;
            }
            UndoAction::UnbornBranch { branch_ref, index } => {
                self.run(&["update-ref", "-d", branch_ref]).await?;
                if let Some(index) = index {
                    self.run(&["read-tree", index]).await?;
                }
            }
            UndoAction::RestoreFiles(files) => self.restore_files(files).await?,
            UndoAction::RestoreWorkingCopy { reset_to, snapshot } => {
                // Tracked files go back with reset --hard; only new untracked files need deleting.
                let before: HashSet<&str> =
                    snapshot.files.iter().map(|f| f.path.as_str()).collect();
                let now = self.status().await?;
                if let Some(to) = reset_to {
                    self.run(&["reset", "-q", "--hard", to]).await?;
                } else if self.head().await.is_some() {
                    self.run(&["reset", "-q", "--hard"]).await?;
                }
                for change in &now.untracked {
                    if !before.contains(change.path.as_str()) {
                        let _ = std::fs::remove_file(self.root.join(&change.path));
                    }
                }
                self.restore_files(&snapshot.files).await?;
                if let Some(index) = &snapshot.index {
                    self.run(&["read-tree", index]).await?;
                }
            }
            UndoAction::Git(commands) => {
                for c in commands {
                    let args: Vec<&str> = c.iter().map(String::as_str).collect();
                    self.run(&args).await?;
                }
            }
            UndoAction::Sequence(steps) => {
                for step in steps {
                    Box::pin(self.undo(step)).await?;
                }
            }
        }
        Ok(())
    }
}
