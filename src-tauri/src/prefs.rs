//! What Seam remembers between launches: the AI configuration, how it tells you an AI run
//! finished, and recent repositories.
//! Kept by the host rather than the web views, because the tray needs it with no window open.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::ai::AiConfig;

const MAX_RECENTS: usize = 12;

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Prefs {
    ai: AiConfig,
    notify: NotifyPrefs,
    recents: Vec<String>,
}

/// What happens when a review, recap or plan finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotifyPrefs {
    /// A system notification, shown while the repository's window isn't in front.
    pub banner: bool,
    /// A macOS system sound by name (`Glass`, `Ping`…), or empty for none.
    pub sound: String,
}

impl Default for NotifyPrefs {
    fn default() -> Self {
        NotifyPrefs {
            banner: true,
            sound: "Glass".into(),
        }
    }
}

pub struct PrefsStore {
    path: PathBuf,
    prefs: Mutex<Prefs>,
}

impl PrefsStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("prefs.json");
        let prefs = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        PrefsStore {
            path,
            prefs: Mutex::new(prefs),
        }
    }

    fn save(&self, prefs: &Prefs) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(prefs) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    fn update(&self, change: impl FnOnce(&mut Prefs)) {
        let mut prefs = self.prefs.lock().unwrap();
        change(&mut prefs);
        self.save(&prefs);
    }

    pub fn ai(&self) -> AiConfig {
        self.prefs.lock().unwrap().ai.clone()
    }

    pub fn set_ai(&self, ai: AiConfig) {
        self.update(|prefs| prefs.ai = ai);
    }

    pub fn notify(&self) -> NotifyPrefs {
        self.prefs.lock().unwrap().notify.clone()
    }

    pub fn set_notify(&self, notify: NotifyPrefs) {
        self.update(|prefs| prefs.notify = notify);
    }

    /// Recent repositories that still exist, newest first.
    pub fn recents(&self) -> Vec<PathBuf> {
        self.prefs
            .lock()
            .unwrap()
            .recents
            .iter()
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .collect()
    }

    pub fn add_recent(&self, path: &Path) {
        let path = path.to_string_lossy().into_owned();
        self.update(|prefs| {
            prefs.recents.retain(|existing| *existing != path);
            prefs.recents.insert(0, path);
            prefs.recents.truncate(MAX_RECENTS);
        });
    }

    pub fn remove_recent(&self, path: &str) {
        self.update(|prefs| prefs.recents.retain(|existing| existing != path));
    }

    /// Adds paths remembered elsewhere (older builds kept them in the web view) after our own.
    pub fn import_recents(&self, paths: Vec<String>) {
        self.update(|prefs| {
            for path in paths {
                if !prefs.recents.contains(&path) && prefs.recents.len() < MAX_RECENTS {
                    prefs.recents.push(path);
                }
            }
        });
    }
}
