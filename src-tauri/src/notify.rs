//! Telling the user an AI run finished: a notification while they are elsewhere, and a sound.

use std::path::Path;

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::prefs::PrefsStore;

/// The sounds in /System/Library/Sounds, the only names `play` accepts.
pub const SOUNDS: [&str; 14] = [
    "Basso",
    "Blow",
    "Bottle",
    "Frog",
    "Funk",
    "Glass",
    "Hero",
    "Morse",
    "Ping",
    "Pop",
    "Purr",
    "Sosumi",
    "Submarine",
    "Tink",
];

pub fn play(sound: &str) {
    if !SOUNDS.contains(&sound) {
        return;
    }
    let path = format!("/System/Library/Sounds/{sound}.aiff");
    tauri::async_runtime::spawn(async move {
        let _ = tokio::process::Command::new("/usr/bin/afplay")
            .arg(path)
            .status()
            .await;
    });
}

fn in_front(app: &AppHandle, root: &Path) -> bool {
    let state = app.state::<crate::AppState>();
    let roots = state.window_roots.lock().unwrap();
    app.webview_windows().values().any(|window| {
        window.is_focused().unwrap_or(false)
            && roots.get(window.label()).map(|path| path.as_path()) == Some(root)
    })
}

/// Called once a run ends, finished or failed; a cancelled run says nothing.
pub fn finished(app: &AppHandle, root: &Path, title: &str, body: &str) {
    let prefs = app.state::<PrefsStore>().notify();
    if prefs.banner && !in_front(app, root) {
        let _ = app.notification().builder().title(title).body(body).show();
    }
    play(&prefs.sound);
}
