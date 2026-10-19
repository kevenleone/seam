//! What the `seam` command asks of the app. It leaves a request in the repository's git
//! directory and has macOS open the folder; the app takes the request when the folder arrives.
//! Only a local process can write one, unlike a URL scheme any web page could fire.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ai::Provider;
use crate::review::{now_secs, Target};

const MAX_AGE_SECS: i64 = 60;
const MAX_PATCHES: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// Show the change in the Review tab without asking a model.
    Open,
    Review,
    /// A Visual recap of the target.
    Explain,
    /// The saved review, as it is.
    ShowReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub action: Action,
    #[serde(default)]
    pub target: Target,
    pub provider: Option<Provider>,
    pub created_at: i64,
}

impl Request {
    pub fn new(action: Action, target: Target, provider: Option<Provider>) -> Self {
        Request {
            action,
            target,
            provider,
            created_at: now_secs(),
        }
    }
}

fn requests_dir(git_dir: &Path) -> PathBuf {
    git_dir.join("seam").join("requests")
}

pub fn send(git_dir: &Path, request: &Request) -> std::io::Result<()> {
    let dir = requests_dir(git_dir);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string(request)?;
    std::fs::write(dir.join(format!("{}.json", uuid::Uuid::new_v4())), json)
}

/// Every waiting request, oldest first. All are removed; stale ones are dropped.
pub fn take(git_dir: &Path) -> Vec<Request> {
    let Ok(entries) = std::fs::read_dir(requests_dir(git_dir)) else {
        return vec![];
    };
    let now = now_secs();
    let mut requests: Vec<Request> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let text = std::fs::read_to_string(&path).ok();
            let _ = std::fs::remove_file(&path);
            serde_json::from_str(&text?).ok()
        })
        .filter(|request: &Request| now - request.created_at <= MAX_AGE_SECS)
        .collect();
    requests.sort_by_key(|request| request.created_at);
    requests
}

/// Keeps a patch read from stdin or made for `git difftool`, so the app can read it later.
pub fn keep_patch(git_dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    let dir = git_dir.join("seam").join("patches");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.patch", uuid::Uuid::new_v4()));
    std::fs::write(&path, text)?;

    let mut kept: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir)?
        .flatten()
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    kept.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, old) in kept.iter().skip(MAX_PATCHES) {
        if *old != path {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_taken_once_and_only_while_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        let fresh = Request::new(
            Action::Review,
            Target::Range {
                range: "main..HEAD".into(),
                paths: vec!["src".into()],
            },
            Some(Provider::Codex),
        );
        let stale = Request {
            created_at: now_secs() - MAX_AGE_SECS - 5,
            ..Request::new(Action::Open, Target::Auto, None)
        };
        send(git_dir, &fresh).unwrap();
        send(git_dir, &stale).unwrap();

        assert_eq!(take(git_dir), vec![fresh]);
        assert!(take(git_dir).is_empty());
        assert_eq!(std::fs::read_dir(requests_dir(git_dir)).unwrap().count(), 0);
    }

    #[test]
    fn keeps_only_the_newest_patches() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = (0..MAX_PATCHES + 3)
            .map(|index| keep_patch(dir.path(), &format!("patch {index}")).unwrap())
            .collect();
        let last = paths.last().unwrap();
        assert_eq!(
            std::fs::read_to_string(last).unwrap(),
            format!("patch {}", MAX_PATCHES + 2)
        );
        let count = std::fs::read_dir(dir.path().join("seam/patches"))
            .unwrap()
            .count();
        assert_eq!(count, MAX_PATCHES);
    }
}
