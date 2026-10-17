pub mod git;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use git::models::*;
use git::ops::{self, History, Op, Outcome};
use git::{GitError, Repo};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult, Debouncer};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, RunEvent, State, TitleBarStyle, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, Window,
};
use tauri_plugin_dialog::DialogExt;

type CmdResult<T> = Result<T, GitError>;

#[derive(Default)]
struct AppState {
    histories: Mutex<HashMap<PathBuf, History>>,
    /// One lock per repository so actions (and undos) run one at a time.
    locks: Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>,
    watchers: Mutex<HashMap<String, Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>>>,
}

impl AppState {
    fn lock(&self, root: &Path) -> Arc<tokio::sync::Mutex<()>> {
        self.locks
            .lock()
            .unwrap()
            .entry(root.to_path_buf())
            .or_default()
            .clone()
    }
}

// MARK: Windows

const WELCOME: &str = "welcome";

/// Centres the close/minimise/zoom buttons in the 46px header the web UI draws.
/// Measured: y = 17 sat ~7px high and y = 32 ~7px low, so the centre is ~24.
fn traffic_lights() -> LogicalPosition<f64> {
    LogicalPosition::new(16.0, 24.0)
}

/// The app background (`--app` in styles.css), painted before the page loads.
const BACKGROUND: tauri::window::Color = tauri::window::Color(18, 19, 23, 255);

/// Windows start hidden and the page shows itself after its first paint (see main.tsx),
/// so nobody sees a blank webview. Show it anyway if the page never gets that far.
fn reveal_later(window: WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        if !window.is_visible().unwrap_or(true) {
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}

fn repo_label(root: &Path) -> String {
    let mut h = DefaultHasher::new();
    root.hash(&mut h);
    format!("repo-{:x}", h.finish())
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn show_welcome(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(WELCOME) {
        w.show()?;
        w.set_focus()?;
        return Ok(w);
    }
    WebviewWindowBuilder::new(
        app,
        WELCOME,
        WebviewUrl::App("index.html?view=welcome".into()),
    )
    .title("Welcome to Seam")
    .inner_size(780.0, 480.0)
    .resizable(false)
    .title_bar_style(TitleBarStyle::Overlay)
    .hidden_title(true)
    .traffic_light_position(traffic_lights())
    .background_color(BACKGROUND)
    .visible(false)
    .center()
    .build()
    .inspect(|w| reveal_later(w.clone()))
}

/// Opens (or focuses) the window for the repository containing `path`.
async fn open_repo_window(app: &AppHandle, path: &Path) -> CmdResult<()> {
    let root = Repo::top_level(path).await.ok_or_else(|| {
        GitError::new(format!("{} isn't inside a git repository.", path.display()))
    })?;
    let label = repo_label(&root);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.set_focus();
    } else {
        let url = format!("index.html?repo={}", encode(&root.to_string_lossy()));
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
            .title(
                root.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
            .inner_size(1360.0, 860.0)
            .min_inner_size(960.0, 600.0)
            .title_bar_style(TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(traffic_lights())
            .background_color(BACKGROUND)
            .visible(false)
            .build()
            .map(reveal_later)
            .map_err(|e| GitError::new(e.to_string()))?;
    }
    if let Some(w) = app.get_webview_window(WELCOME) {
        let _ = w.close();
    }
    let _ = app.emit("recent-opened", root.to_string_lossy());
    Ok(())
}

fn focused_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.webview_windows()
        .into_values()
        .find(|w| w.is_focused().unwrap_or(false))
}

fn pick_and_open(app: &AppHandle) {
    let handle = app.clone();
    app.dialog()
        .file()
        .set_title("Open a git repository")
        .pick_folder(move |folder| {
            let Some(path) = folder.and_then(|f| f.into_path().ok()) else {
                return;
            };
            tauri::async_runtime::spawn(async move {
                if let Err(e) = open_repo_window(&handle, &path).await {
                    handle
                        .dialog()
                        .message(e.message)
                        .title("Couldn't open repository")
                        .show(|_| {});
                }
            });
        });
}

// MARK: Commands — windows

#[tauri::command]
async fn open_repository(app: AppHandle, path: String) -> CmdResult<()> {
    open_repo_window(&app, Path::new(&path)).await
}

#[tauri::command]
fn open_folder_dialog(app: AppHandle) {
    pick_and_open(&app);
}

#[tauri::command]
fn open_welcome(app: AppHandle) {
    let _ = show_welcome(&app);
}

#[tauri::command]
async fn top_level(path: String) -> Option<String> {
    Repo::top_level(Path::new(&path))
        .await
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn init_repository(path: String) -> CmdResult<()> {
    Repo::init(Path::new(&path)).await
}

#[tauri::command]
async fn clone_repository(
    app: AppHandle,
    window: Window,
    url: String,
    destination: String,
) -> CmdResult<()> {
    let label = window.label().to_string();
    let emitter = app.clone();
    let progress = move |line: String| {
        let _ = emitter.emit_to(&label, "git-progress", line);
    };
    let dest = PathBuf::from(&destination);
    Repo::clone_into(&url, &dest, &progress).await?;
    open_repo_window(&app, &dest).await
}

#[tauri::command]
fn set_git_path(path: Option<String>) {
    git::process::set_git_executable(path.filter(|p| !p.trim().is_empty()).map(PathBuf::from));
}

#[tauri::command]
async fn git_version() -> CmdResult<String> {
    let out = git::process::git(&std::env::temp_dir(), &["--version"], Default::default()).await?;
    Ok(out.text().trim().to_string())
}

// MARK: Commands — reading

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    status: RepoStatus,
    in_progress: Option<InProgress>,
    last_commit: Option<Commit>,
}

#[tauri::command]
async fn snapshot(root: String) -> CmdResult<Snapshot> {
    let repo = Repo::new(root);
    Ok(Snapshot {
        status: repo.status().await?,
        in_progress: repo.in_progress().await?,
        last_commit: repo.last_commit().await?,
    })
}

#[tauri::command]
async fn file_diff(
    root: String,
    path: String,
    source: DiffSource,
    ignore_whitespace: bool,
) -> CmdResult<Option<FileDiff>> {
    Repo::new(root).diff(&path, source, ignore_whitespace).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryMeta {
    branches: Vec<Branch>,
    tags: Vec<Tag>,
    stashes: Vec<Stash>,
    remotes: Vec<Remote>,
    unpushed: Vec<String>,
}

#[tauri::command]
async fn history_meta(root: String) -> CmdResult<HistoryMeta> {
    let repo = Repo::new(root);
    let (branches, tags, stashes, remotes, unpushed) = tokio::join!(
        repo.branches(),
        repo.tags(),
        repo.stashes(),
        repo.remotes(),
        repo.unpushed()
    );
    Ok(HistoryMeta {
        branches: branches?,
        tags: tags?,
        stashes: stashes?,
        remotes: remotes?,
        unpushed: unpushed.into_iter().collect(),
    })
}

#[tauri::command]
async fn log(root: String, reference: String, skip: usize, limit: usize) -> CmdResult<Vec<Commit>> {
    Repo::new(root).log(&reference, skip, limit).await
}

#[tauri::command]
async fn merge_base(root: String, a: String, b: String) -> Option<String> {
    Repo::new(root).merge_base(&a, &b).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Detail {
    commit: Option<Commit>,
    files: Vec<CommitFileStat>,
    diffs: Vec<FileDiff>,
}

#[tauri::command]
async fn commit_detail(root: String, sha: String, ignore_whitespace: bool) -> CmdResult<Detail> {
    let repo = Repo::new(root);
    let commit = repo.log(&sha, 0, 1).await?.into_iter().next();
    let (files, diffs) = tokio::join!(
        repo.commit_files(&sha),
        repo.commit_diff(&sha, ignore_whitespace)
    );
    Ok(Detail {
        commit,
        files: files?,
        diffs: diffs?,
    })
}

fn stats(diffs: &[FileDiff]) -> Vec<CommitFileStat> {
    diffs
        .iter()
        .map(|d| CommitFileStat {
            path: d.path().to_string(),
            additions: Some(d.additions),
            deletions: Some(d.deletions),
        })
        .collect()
}

#[tauri::command]
async fn stash_detail(root: String, stash: Stash) -> CmdResult<Detail> {
    let diffs = Repo::new(root).stash_diff(&stash).await?;
    Ok(Detail {
        commit: None,
        files: stats(&diffs),
        diffs,
    })
}

#[tauri::command]
async fn range_detail(root: String, oldest: String, newest: String) -> CmdResult<Detail> {
    let diffs = Repo::new(root).range_diff(&oldest, &newest).await?;
    Ok(Detail {
        commit: None,
        files: stats(&diffs),
        diffs,
    })
}

// MARK: Commands — actions and undo

fn progress_for(app: &AppHandle, window: &Window) -> impl Fn(String) + Send + Sync + 'static {
    let (app, label) = (app.clone(), window.label().to_string());
    move |line: String| {
        let _ = app.emit_to(&label, "git-progress", line);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UndoState {
    undo: Option<String>,
    redo: Option<String>,
}

fn undo_state(state: &AppState, root: &Path) -> UndoState {
    let (undo, redo) = state
        .histories
        .lock()
        .unwrap()
        .get(root)
        .map(|h| h.titles())
        .unwrap_or_default();
    UndoState { undo, redo }
}

#[tauri::command]
async fn perform(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    root: String,
    op: Op,
) -> CmdResult<Outcome> {
    let root = PathBuf::from(root);
    let lock = state.lock(&root);
    let _guard = lock.lock().await;
    let progress = progress_for(&app, &window);
    let (undoable, message) = ops::perform(&Repo::new(&root), &op, &progress).await?;
    let can_undo = undoable.is_some();
    state
        .histories
        .lock()
        .unwrap()
        .entry(root)
        .or_default()
        .record(op, undoable);
    Ok(Outcome {
        message,
        undoable: can_undo,
    })
}

#[tauri::command]
async fn undo(state: State<'_, AppState>, root: String) -> CmdResult<Option<Outcome>> {
    let root = PathBuf::from(root);
    let lock = state.lock(&root);
    let _guard = lock.lock().await;
    let Some((op, undoable)) = state
        .histories
        .lock()
        .unwrap()
        .entry(root.clone())
        .or_default()
        .pop_undo()
    else {
        return Ok(None);
    };
    Repo::new(&root).undo(&undoable.action).await?;
    state
        .histories
        .lock()
        .unwrap()
        .entry(root)
        .or_default()
        .push_redo(op);
    Ok(Some(Outcome {
        message: format!("Undid {}", undoable.title.to_lowercase()),
        undoable: false,
    }))
}

#[tauri::command]
async fn redo(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    root: String,
) -> CmdResult<Option<Outcome>> {
    let root = PathBuf::from(root);
    let lock = state.lock(&root);
    let _guard = lock.lock().await;
    let Some(op) = state
        .histories
        .lock()
        .unwrap()
        .entry(root.clone())
        .or_default()
        .pop_redo()
    else {
        return Ok(None);
    };
    let progress = progress_for(&app, &window);
    let (undoable, message) = ops::perform(&Repo::new(&root), &op, &progress).await?;
    let can_undo = undoable.is_some();
    state
        .histories
        .lock()
        .unwrap()
        .entry(root)
        .or_default()
        .record_redo(op, undoable);
    Ok(Some(Outcome {
        message,
        undoable: can_undo,
    }))
}

#[tauri::command]
fn undo_titles(state: State<'_, AppState>, root: String) -> UndoState {
    undo_state(&state, Path::new(&root))
}

// MARK: Watching

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RepoChanged {
    work_tree: bool,
    refs: bool,
}

#[tauri::command]
async fn watch(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    root: String,
) -> CmdResult<()> {
    let root = PathBuf::from(&root);
    let git_dir = Repo::new(&root).git_dir().await?;
    let label = window.label().to_string();
    let emitter = app.clone();
    let target = label.clone();
    let git_dir_for_events = git_dir.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        move |res: DebounceEventResult| {
            let Ok(events) = res else { return };
            let mut change = RepoChanged {
                work_tree: false,
                refs: false,
            };
            for event in events {
                if let Ok(rel) = event.path.strip_prefix(&git_dir_for_events) {
                    let rel = rel.to_string_lossy();
                    // Object writes and lock files are noise; the ref or index update that follows is what matters.
                    if rel.starts_with("objects")
                        || rel.starts_with("logs")
                        || rel.ends_with(".lock")
                    {
                        continue;
                    }
                    change.refs = true;
                } else {
                    change.work_tree = true;
                }
            }
            if change.work_tree || change.refs {
                let _ = emitter.emit_to(&target, "repo-changed", change);
            }
        },
    )
    .map_err(|e| GitError::new(e.to_string()))?;
    debouncer
        .watcher()
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| GitError::new(e.to_string()))?;
    if !git_dir.starts_with(&root) {
        let _ = debouncer
            .watcher()
            .watch(&git_dir, RecursiveMode::Recursive);
    }
    state.watchers.lock().unwrap().insert(label, debouncer);
    Ok(())
}

// MARK: Menu

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let item =
        |id: &str, title: &str, accel: Option<&str>| MenuItem::with_id(app, id, title, true, accel);
    let app_menu = Submenu::with_items(
        app,
        "Seam",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Seam"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &item("settings", "Settings…", Some("CmdOrCtrl+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &item("open", "Open Repository…", Some("CmdOrCtrl+O"))?,
            &item("clone", "Clone Repository…", None)?,
            &item("welcome", "Welcome to Seam", Some("CmdOrCtrl+Shift+0"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &item("view-changes", "Changes", Some("CmdOrCtrl+1"))?,
            &item("view-history", "History", Some("CmdOrCtrl+2"))?,
            &PredefinedMenuItem::separator(app)?,
            &item("toggle-left", "Toggle Sidebar", Some("CmdOrCtrl+B"))?,
            &item("toggle-right", "Toggle Commit Panel", Some("CmdOrCtrl+L"))?,
            &PredefinedMenuItem::separator(app)?,
            &item("palette", "Command Palette…", Some("CmdOrCtrl+K"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;
    let repo = Submenu::with_items(
        app,
        "Repository",
        true,
        &[
            &item("stage-all", "Stage All", Some("CmdOrCtrl+Shift+A"))?,
            &item("unstage-all", "Unstage All", None)?,
            &item("stash", "Stash Changes…", Some("CmdOrCtrl+Alt+S"))?,
            &PredefinedMenuItem::separator(app)?,
            &item("fetch", "Fetch", Some("CmdOrCtrl+Shift+F"))?,
            &item("pull", "Pull", Some("CmdOrCtrl+Shift+P"))?,
            &item("push", "Push", Some("CmdOrCtrl+P"))?,
            &PredefinedMenuItem::separator(app)?,
            &item("new-branch", "New Branch…", Some("CmdOrCtrl+Shift+N"))?,
            &item("uncommit", "Undo Last Commit", None)?,
            &PredefinedMenuItem::separator(app)?,
            &item("reveal", "Show in Finder", None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &repo, &window])
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => pick_and_open(app),
        "welcome" => {
            let _ = show_welcome(app);
        }
        "clone" => {
            if let Ok(w) = show_welcome(app) {
                let _ = w.emit_to(WELCOME, "menu", "clone");
            }
        }
        _ => {
            // Repository commands go to the focused window, which knows its repository.
            if let Some(w) = focused_window(app) {
                let _ = app.emit_to(w.label(), "menu", id);
            }
        }
    }
}

// MARK: App

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .menu(build_menu)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            open_repository,
            open_folder_dialog,
            open_welcome,
            top_level,
            init_repository,
            clone_repository,
            set_git_path,
            git_version,
            snapshot,
            file_diff,
            history_meta,
            log,
            merge_base,
            commit_detail,
            stash_detail,
            range_detail,
            perform,
            undo,
            redo,
            undo_titles,
            watch,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                state.watchers.lock().unwrap().remove(window.label());
            }
        })
        .setup(|app| {
            // `seam /path/to/repo` from a terminal.
            let paths: Vec<PathBuf> = std::env::args()
                .skip(1)
                .filter(|a| !a.starts_with('-'))
                .map(PathBuf::from)
                .filter(|p| p.is_dir())
                .collect();
            let handle = app.handle().clone();
            if paths.is_empty() {
                show_welcome(&handle)?;
            } else {
                tauri::async_runtime::spawn(async move {
                    for p in paths {
                        if open_repo_window(&handle, &p).await.is_err() {
                            let _ = show_welcome(&handle);
                        }
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Seam");

    app.run(|app, event| match event {
        // Folders dropped on the Dock icon or opened with `open -a Seam <dir>`.
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            for url in urls {
                if let Ok(path) = url.to_file_path() {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = open_repo_window(&handle, &path).await {
                            handle
                                .dialog()
                                .message(e.message)
                                .title("Couldn't open repository")
                                .show(|_| {});
                        }
                    });
                }
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } => {
            let _ = show_welcome(app);
        }
        _ => {}
    });
}
