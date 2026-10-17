//! Every git action the UI can take, as data, so it can be performed, undone and redone.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::models::*;
use super::patch::Selection;
use super::process::Result;
use super::repo::{Repo, Undoable};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Op {
    Stage {
        paths: Vec<String>,
    },
    Unstage {
        paths: Vec<String>,
    },
    StageAll,
    UnstageAll,
    Discard {
        changes: Vec<FileChange>,
    },
    /// `selection` maps hunk id → line ids; None means every line of the file.
    ApplyLines {
        file: FileDiff,
        selection: Option<HashMap<usize, Vec<usize>>>,
        action: LineAction,
    },
    Resolve {
        path: String,
        side: Option<ConflictSide>,
    },
    Commit {
        message: String,
        amend: bool,
    },
    Uncommit,
    Reword {
        sha: String,
        message: String,
    },
    Drop {
        shas: Vec<String>,
    },
    Squash {
        shas: Vec<String>,
        message: String,
    },
    Revert {
        sha: String,
    },
    CherryPick {
        shas: Vec<String>,
    },
    Reset {
        sha: String,
        mode: ResetMode,
    },
    Abort,
    Continue,
    Skip,
    Checkout {
        branch: String,
    },
    CreateBranch {
        name: String,
        start: Option<String>,
        checkout: bool,
    },
    DeleteBranch {
        name: String,
        force: bool,
    },
    RenameBranch {
        old: String,
        new: String,
    },
    CreateTag {
        name: String,
        sha: String,
        message: Option<String>,
    },
    DeleteTag {
        name: String,
    },
    StashPush {
        message: Option<String>,
        include_untracked: bool,
        paths: Vec<String>,
    },
    StashApply {
        stash: Stash,
        pop: bool,
    },
    StashDrop {
        stash: Stash,
    },
    Fetch,
    Pull {
        mode: PullMode,
    },
    Push {
        force: bool,
    },
}

/// What happened, for the toast.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub message: String,
    pub undoable: bool,
}

pub async fn perform(
    repo: &Repo,
    op: &Op,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(Option<Undoable>, String)> {
    let set = |v: &[String]| v.iter().cloned().collect::<HashSet<String>>();
    let u = |r: Undoable| Some(r);
    Ok(match op {
        Op::Stage { paths } => (repo.stage(paths).await?, "Staged".into()),
        Op::Unstage { paths } => (repo.unstage(paths).await?, "Unstaged".into()),
        Op::StageAll => (repo.stage_all().await?, "Staged all".into()),
        Op::UnstageAll => (repo.unstage_all().await?, "Unstaged all".into()),
        Op::Discard { changes } => (repo.discard(changes).await?, "Discarded changes".into()),
        Op::ApplyLines {
            file,
            selection,
            action,
        } => {
            let selection: Option<Selection> = selection.as_ref().map(|s| {
                s.iter()
                    .map(|(h, lines)| (*h, lines.iter().copied().collect()))
                    .collect()
            });
            let msg = match action {
                LineAction::Stage => "Staged lines",
                LineAction::Unstage => "Unstaged lines",
                LineAction::Discard => "Discarded lines",
            };
            (
                repo.apply_lines(file, selection.as_ref(), *action).await?,
                msg.into(),
            )
        }
        Op::Resolve { path, side } => (
            repo.resolve_conflict(path, *side).await?,
            "Marked resolved".into(),
        ),
        Op::Commit { message, amend } => (
            u(repo.commit(message, *amend).await?),
            if *amend {
                "Amended commit"
            } else {
                "Committed"
            }
            .into(),
        ),
        Op::Uncommit => (
            u(repo.uncommit().await?),
            "Undid commit — changes are staged".into(),
        ),
        Op::Reword { sha, message } => {
            (u(repo.reword(sha, message).await?), "Renamed commit".into())
        }
        Op::Drop { shas } => (
            u(repo.drop_commits(&set(shas)).await?),
            if shas.len() == 1 {
                "Removed commit"
            } else {
                "Removed commits"
            }
            .into(),
        ),
        Op::Squash { shas, message } => (
            u(repo.squash(&set(shas), message).await?),
            format!("Squashed {} commits", shas.len()),
        ),
        Op::Revert { sha } => (u(repo.revert(sha).await?), "Reverted commit".into()),
        Op::CherryPick { shas } => (
            u(repo.cherry_pick(shas).await?),
            format!(
                "Cherry-picked {} commit{}",
                shas.len(),
                if shas.len() == 1 { "" } else { "s" }
            ),
        ),
        Op::Reset { sha, mode } => (
            u(repo.reset(sha, *mode).await?),
            format!("Reset to {}", &sha[..7.min(sha.len())]),
        ),
        Op::Abort | Op::Continue | Op::Skip => {
            let current = repo
                .in_progress()
                .await?
                .ok_or_else(|| super::GitError::new("Nothing is in progress."))?;
            match op {
                Op::Abort => repo.abort(&current).await?,
                Op::Continue => repo.continue_op(&current).await?,
                _ => repo.skip(&current).await?,
            }
            (
                None,
                match op {
                    Op::Abort => "Aborted",
                    Op::Continue => "Continued",
                    _ => "Skipped commit",
                }
                .into(),
            )
        }
        Op::Checkout { branch } => (
            u(repo.checkout(branch).await?),
            format!("Switched to {branch}"),
        ),
        Op::CreateBranch {
            name,
            start,
            checkout,
        } => (
            u(repo
                .create_branch(name, start.as_deref(), *checkout)
                .await?),
            format!("Created {name}"),
        ),
        Op::DeleteBranch { name, force } => (
            u(repo.delete_branch(name, *force).await?),
            format!("Deleted {name}"),
        ),
        Op::RenameBranch { old, new } => (
            u(repo.rename_branch(old, new).await?),
            format!("Renamed to {new}"),
        ),
        Op::CreateTag { name, sha, message } => (
            u(repo.create_tag(name, sha, message.as_deref()).await?),
            format!("Tagged {name}"),
        ),
        Op::DeleteTag { name } => (
            u(repo.delete_tag(name).await?),
            format!("Deleted tag {name}"),
        ),
        Op::StashPush {
            message,
            include_untracked,
            paths,
        } => (
            u(repo
                .stash_push(message.as_deref(), *include_untracked, paths)
                .await?),
            "Stashed changes".into(),
        ),
        Op::StashApply { stash, pop } => (
            u(repo.stash_apply(stash, *pop).await?),
            if *pop {
                "Popped stash"
            } else {
                "Applied stash"
            }
            .into(),
        ),
        Op::StashDrop { stash } => (u(repo.stash_drop(stash).await?), "Dropped stash".into()),
        Op::Fetch => {
            repo.fetch(progress).await?;
            (None, "Fetched".into())
        }
        Op::Pull { mode } => (u(repo.pull(*mode, progress).await?), "Pulled".into()),
        Op::Push { force } => {
            repo.push(*force, progress).await?;
            (None, "Pushed — this can't be undone from Seam".into())
        }
    })
}

/// Per-repository undo and redo stacks.
#[derive(Default)]
pub struct History {
    undo: Vec<(Op, Undoable)>,
    redo: Vec<Op>,
}

impl History {
    pub fn record(&mut self, op: Op, undoable: Option<Undoable>) {
        if let Some(u) = undoable {
            self.undo.push((op, u));
        }
        self.redo.clear();
    }

    pub fn pop_undo(&mut self) -> Option<(Op, Undoable)> {
        self.undo.pop()
    }

    pub fn push_redo(&mut self, op: Op) {
        self.redo.push(op);
    }

    pub fn pop_redo(&mut self) -> Option<Op> {
        self.redo.pop()
    }

    /// After a redo, keep the remaining redo stack.
    pub fn record_redo(&mut self, op: Op, undoable: Option<Undoable>) {
        if let Some(u) = undoable {
            self.undo.push((op, u));
        }
    }

    pub fn titles(&self) -> (Option<String>, Option<String>) {
        (
            self.undo.last().map(|(_, u)| u.title.clone()),
            self.redo.last().map(|_| "Redo".to_string()),
        )
    }
}
