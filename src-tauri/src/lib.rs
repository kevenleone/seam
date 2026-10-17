pub mod ai;
mod cli;
pub mod git;
mod notify;
mod prefs;
pub mod review;
mod tray;
pub mod visual;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ai::{AiConfig, Provider, ProviderStatus, TestResult};
use git::models::*;
use git::ops::{self, History, Op, Outcome};
use git::{GitError, Repo};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult, Debouncer};
use prefs::{NotifyPrefs, PrefsStore};
use review::ReviewState;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, RunEvent, State, TitleBarStyle, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, Window,
};
use tauri_plugin_dialog::DialogExt;
use visual::{ScopeSummary, Visual, VisualKind, VisualState, VisualSummary};

type CmdResult<T> = Result<T, GitError>;

/// A recap or plan state, with the branch it was shown on: a recap is shown only there.
type ShownVisual = (Option<String>, VisualState);

#[derive(Default)]
struct AppState {
    histories: Mutex<HashMap<PathBuf, History>>,
    /// One lock per repository so actions (and undos) run one at a time.
    locks: Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>,
    watchers: Mutex<HashMap<String, Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>>>,
    reviews: Mutex<HashMap<PathBuf, ReviewState>>,
    review_tasks: Mutex<HashMap<PathBuf, tauri::async_runtime::JoinHandle<()>>>,
    visuals: Mutex<HashMap<(PathBuf, VisualKind), ShownVisual>>,
    visual_tasks: Mutex<HashMap<(PathBuf, VisualKind), tauri::async_runtime::JoinHandle<()>>>,
    /// Which repository each window shows, by window label.
    window_roots: Mutex<HashMap<String, PathBuf>>,
    /// The repository window focused last; the tray offers to review it.
    last_repo: Mutex<Option<PathBuf>>,
    /// Repositories behind the tray's numbered menu ids.
    tray_targets: Mutex<Vec<PathBuf>>,
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

const SETTINGS: &str = "settings";

/// Settings in a window of their own, for when no repository window is there to hold the sheet.
fn show_settings(app: &AppHandle, pane: Option<&str>) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        if let Some(pane) = pane {
            let _ = app.emit_to(SETTINGS, "show-pane", pane);
        }
        window.show()?;
        window.set_focus()?;
        return Ok(window);
    }
    let url = format!(
        "index.html?view=settings&pane={}",
        pane.unwrap_or("general")
    );
    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(url.into()))
        .title("Seam Settings")
        .inner_size(1000.0, 700.0)
        .min_inner_size(820.0, 560.0)
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(traffic_lights())
        .background_color(BACKGROUND)
        .visible(false)
        .center()
        .build()
        .inspect(|w| reveal_later(w.clone()))
}

#[tauri::command]
fn open_settings_window(app: AppHandle, pane: Option<String>) {
    let _ = show_settings(&app, pane.as_deref());
}

/// Opens (or focuses) the window for the repository containing `path`, optionally on a tab.
async fn open_repo_window(app: &AppHandle, path: &Path, tab: Option<&str>) -> CmdResult<()> {
    let root = Repo::top_level(path).await.ok_or_else(|| {
        GitError::new(format!("{} isn't inside a git repository.", path.display()))
    })?;
    let label = repo_label(&root);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(tab) = tab {
            let _ = app.emit_to(&label, "show-tab", tab);
        }
    } else {
        let mut url = format!("index.html?repo={}", encode(&root.to_string_lossy()));
        if let Some(tab) = tab {
            url.push_str(&format!("&tab={tab}"));
        }
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
    app.state::<AppState>()
        .window_roots
        .lock()
        .unwrap()
        .insert(label, root.clone());
    *app.state::<AppState>().last_repo.lock().unwrap() = Some(root.clone());
    app.state::<PrefsStore>().add_recent(&root);
    let _ = app.emit("recent-opened", root.to_string_lossy());
    tray::refresh(app);
    Ok(())
}

/// Opens a repository window from a callback that can't wait, reporting failures in a dialog.
fn spawn_open(app: &AppHandle, root: PathBuf, tab: Option<&'static str>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open_repo_window(&app, &root, tab).await {
            app.dialog()
                .message(e.message)
                .title("Couldn't open repository")
                .show(|_| {});
        }
    });
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
                if let Err(e) = open_repo_window(&handle, &path, None).await {
                    handle
                        .dialog()
                        .message(e.message)
                        .title("Couldn't open repository")
                        .show(|_| {});
                }
            });
        });
}

// MARK: Commands: windows

#[tauri::command]
async fn open_repository(app: AppHandle, path: String) -> CmdResult<()> {
    open_repo_window(&app, Path::new(&path), None).await
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
    open_repo_window(&app, &dest, None).await
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

// MARK: Commands: reading

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

// MARK: Commands: actions and undo

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

// MARK: Commands: review

fn emit_review(app: &AppHandle, root: &Path) {
    let _ = app.emit("review-updated", root.to_string_lossy());
    tray::refresh(app);
}

/// The repository's review: the one running or finished this session, else the saved one.
async fn review_state_for(app: &AppHandle, root: &Path) -> ReviewState {
    if let Some(state) = app.state::<AppState>().reviews.lock().unwrap().get(root) {
        return state.clone();
    }
    let state = match Repo::new(root)
        .git_dir()
        .await
        .ok()
        .and_then(|dir| review::load(&dir))
    {
        Some(review) => ReviewState::Done { review },
        None => ReviewState::Idle,
    };
    app.state::<AppState>()
        .reviews
        .lock()
        .unwrap()
        .entry(root.to_path_buf())
        .or_insert_with(|| state.clone());
    state
}

fn previous_review(state: ReviewState) -> Option<review::Review> {
    match state {
        ReviewState::Done { review } => Some(review),
        ReviewState::Failed { previous, .. } => previous,
        _ => None,
    }
}

async fn start_review_task(app: &AppHandle, root: PathBuf, config: AiConfig) -> CmdResult<()> {
    let current = review_state_for(app, &root).await;
    if matches!(current, ReviewState::Running { .. }) {
        return Ok(());
    }
    let repo = Repo::new(&root);
    let set = |state: ReviewState| {
        app.state::<AppState>()
            .reviews
            .lock()
            .unwrap()
            .insert(root.clone(), state);
        emit_review(app, &root);
    };
    let scope = match review::scope(&repo).await {
        Ok(scope) => scope,
        Err(e) => {
            set(ReviewState::Failed {
                provider: config.provider,
                message: e.message.clone(),
                previous: previous_review(current),
            });
            return Err(e);
        }
    };
    set(ReviewState::Running {
        provider: config.provider,
        scope: scope.label.clone(),
        files: scope.diffs.len(),
        started_at: review::now_secs(),
    });

    let handle = app.clone();
    let task_root = root.clone();
    let previous = previous_review(current);
    let task = tauri::async_runtime::spawn(async move {
        let result = review::run(&config, &task_root, scope).await;
        let state = match result {
            Ok(review) => {
                if let Ok(dir) = Repo::new(&task_root).git_dir().await {
                    review::save(&dir, &review);
                }
                ReviewState::Done { review }
            }
            Err(e) => ReviewState::Failed {
                provider: config.provider,
                message: e.message,
                previous,
            },
        };
        notify_review(&handle, &task_root, &state);
        let app_state = handle.state::<AppState>();
        app_state
            .reviews
            .lock()
            .unwrap()
            .insert(task_root.clone(), state);
        app_state.review_tasks.lock().unwrap().remove(&task_root);
        emit_review(&handle, &task_root);
    });
    app.state::<AppState>()
        .review_tasks
        .lock()
        .unwrap()
        .insert(root, task);
    Ok(())
}

// MARK: Commands: visual recaps and plans

fn emit_visual(app: &AppHandle, root: &Path) {
    let _ = app.emit("visual-updated", root.to_string_lossy());
}

fn set_visual(
    app: &AppHandle,
    root: &Path,
    kind: VisualKind,
    branch: Option<String>,
    state: VisualState,
) {
    app.state::<AppState>()
        .visuals
        .lock()
        .unwrap()
        .insert((root.to_path_buf(), kind), (branch, state));
    emit_visual(app, root);
}

/// The one running, else the one shown on this branch this session, else the newest saved
/// recap of this branch (or plan of any). Switching branches is what brings the change.
async fn visual_state_for(app: &AppHandle, root: &Path, kind: VisualKind) -> VisualState {
    let key = (root.to_path_buf(), kind);
    let repo = Repo::new(root);
    let branch = repo.current_branch().await;
    if let Some((shown_on, state)) = app.state::<AppState>().visuals.lock().unwrap().get(&key) {
        let current = kind == VisualKind::Plan || *shown_on == branch;
        if current || matches!(state, VisualState::Running { .. }) {
            return state.clone();
        }
    }
    let saved = match repo.git_dir().await {
        Ok(dir) => visual::latest(&dir, kind, branch.as_deref()),
        Err(_) => None,
    };
    let state = saved.map_or(VisualState::Idle, |visual| VisualState::Done { visual });
    app.state::<AppState>()
        .visuals
        .lock()
        .unwrap()
        .insert(key, (branch, state.clone()));
    state
}

fn previous_visual(state: VisualState) -> Option<Visual> {
    match state {
        VisualState::Done { visual } => Some(visual),
        VisualState::Failed { previous, .. } => previous,
        _ => None,
    }
}

#[tauri::command]
async fn visual_state(app: AppHandle, root: String, kind: VisualKind) -> VisualState {
    visual_state_for(&app, Path::new(&root), kind).await
}

/// What a recap would cover right now; an error when there is nothing to recap.
#[tauri::command]
async fn visual_scope(root: String) -> CmdResult<ScopeSummary> {
    let scope = review::scope(&Repo::new(Path::new(&root))).await?;
    Ok(ScopeSummary::of(&scope))
}

/// Every saved recap or plan, newest first, from every branch.
#[tauri::command]
async fn visual_history(root: String, kind: VisualKind) -> Vec<VisualSummary> {
    match Repo::new(Path::new(&root)).git_dir().await {
        Ok(dir) => visual::history(&dir, kind),
        Err(_) => vec![],
    }
}

/// Shows a saved one, from any branch, until the branch changes.
#[tauri::command]
async fn open_visual(app: AppHandle, root: String, kind: VisualKind, id: String) -> CmdResult<()> {
    let root = PathBuf::from(root);
    let repo = Repo::new(&root);
    let dir = repo.git_dir().await?;
    let visual = visual::load(&dir, kind, &id).ok_or_else(|| GitError::new("That one is gone."))?;
    let branch = repo.current_branch().await;
    set_visual(&app, &root, kind, branch, VisualState::Done { visual });
    Ok(())
}

/// Starts a recap of the branch, or a plan for `prompt`, with the configured AI or `provider`.
#[tauri::command]
async fn start_visual(
    app: AppHandle,
    root: String,
    kind: VisualKind,
    prompt: Option<String>,
    provider: Option<Provider>,
) -> CmdResult<()> {
    let root = PathBuf::from(root);
    let mut config = app.state::<PrefsStore>().ai();
    if let Some(provider) = provider {
        if provider != config.provider {
            config.provider = provider;
            config.model.clear();
        }
    }
    let current = visual_state_for(&app, &root, kind).await;
    if matches!(current, VisualState::Running { .. }) {
        return Ok(());
    }
    let prompt = prompt
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    let repo = Repo::new(&root);
    let branch = repo.current_branch().await;
    let scope = match kind {
        VisualKind::Recap => review::scope(&repo).await,
        VisualKind::Plan if prompt.is_none() => {
            Err(GitError::new("Describe the change to plan first."))
        }
        VisualKind::Plan => visual::plan_scope(&repo).await,
    };
    let scope = match scope {
        Ok(scope) => scope,
        Err(e) => {
            set_visual(
                &app,
                &root,
                kind,
                branch,
                VisualState::Failed {
                    kind,
                    provider: config.provider,
                    message: e.message.clone(),
                    previous: previous_visual(current),
                },
            );
            return Err(e);
        }
    };
    set_visual(
        &app,
        &root,
        kind,
        branch.clone(),
        VisualState::Running {
            kind,
            provider: config.provider,
            scope: scope.label.clone(),
            prompt: prompt.clone(),
            started_at: review::now_secs(),
            commits: scope.commits.clone(),
            diffs: scope.diffs.clone(),
        },
    );

    let handle = app.clone();
    let task_root = root.clone();
    let previous = previous_visual(current);
    let task = tauri::async_runtime::spawn(async move {
        let state = match visual::run(&config, &task_root, kind, scope, prompt).await {
            Ok(mut visual) => {
                visual.branch = branch.clone();
                if let Ok(dir) = Repo::new(&task_root).git_dir().await {
                    visual::save(&dir, &visual);
                }
                VisualState::Done { visual }
            }
            Err(e) => VisualState::Failed {
                kind,
                provider: config.provider,
                message: e.message,
                previous,
            },
        };
        handle
            .state::<AppState>()
            .visual_tasks
            .lock()
            .unwrap()
            .remove(&(task_root.clone(), kind));
        notify_visual(&handle, &task_root, &state);
        set_visual(&handle, &task_root, kind, branch, state);
    });
    app.state::<AppState>()
        .visual_tasks
        .lock()
        .unwrap()
        .insert((root, kind), task);
    Ok(())
}

#[tauri::command]
async fn cancel_visual(app: AppHandle, root: String, kind: VisualKind) {
    let key = (PathBuf::from(root), kind);
    // Dropping the task kills the agent process (kill_on_drop).
    if let Some(task) = app
        .state::<AppState>()
        .visual_tasks
        .lock()
        .unwrap()
        .remove(&key)
    {
        task.abort();
    }
    let state = app.state::<AppState>();
    let mut visuals = state.visuals.lock().unwrap();
    if matches!(visuals.get(&key), Some((_, VisualState::Running { .. }))) {
        visuals.remove(&key);
    }
    drop(visuals);
    emit_visual(&app, &key.0);
}

/// Asks where to save an export and writes it there; the renderer never names a path.
#[tauri::command]
async fn export_visual(
    window: WebviewWindow,
    name: String,
    extension: String,
    contents: String,
) -> CmdResult<bool> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_file_name(format!("{name}.{extension}"))
        .add_filter(extension.to_uppercase(), &[extension.as_str()])
        .save_file(move |path| {
            let _ = sender.send(path);
        });
    let Some(path) = receiver.await.ok().flatten() else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|e| GitError::new(e.to_string()))?;
    std::fs::write(&path, contents)?;
    Ok(true)
}

/// The system print dialog, whose PDF menu saves the visual as a PDF.
#[tauri::command]
fn print_window(window: WebviewWindow) -> CmdResult<()> {
    window.print().map_err(|e| GitError::new(e.to_string()))
}

// MARK: Commands: notifications

fn repo_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned())
}

fn first_line(message: &str) -> String {
    message
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect()
}

fn notify_review(app: &AppHandle, root: &Path, state: &ReviewState) {
    let (title, body) = match state {
        ReviewState::Done { review } => (
            format!("Review of {} is ready", review.scope),
            match review.findings.len() {
                0 => format!("No findings · {}", repo_name(root)),
                1 => format!("1 finding · {}", repo_name(root)),
                count => format!("{count} findings · {}", repo_name(root)),
            },
        ),
        ReviewState::Failed { message, .. } => ("Review failed".into(), first_line(message)),
        _ => return,
    };
    notify::finished(app, root, &title, &body);
}

fn notify_visual(app: &AppHandle, root: &Path, state: &VisualState) {
    let (title, body) = match state {
        VisualState::Done { visual } => (
            match visual.kind {
                VisualKind::Recap => format!("Recap of {} is ready", visual.scope),
                VisualKind::Plan => "Plan is ready".into(),
            },
            format!("{} · {}", visual.title, repo_name(root)),
        ),
        VisualState::Failed { kind, message, .. } => (
            match kind {
                VisualKind::Recap => "Recap failed".into(),
                VisualKind::Plan => "Plan failed".into(),
            },
            first_line(message),
        ),
        _ => return,
    };
    notify::finished(app, root, &title, &body);
}

#[tauri::command]
fn notify_prefs(prefs: State<'_, PrefsStore>) -> NotifyPrefs {
    prefs.notify()
}

#[tauri::command]
fn set_notify_prefs(prefs: State<'_, PrefsStore>, notify: NotifyPrefs) {
    prefs.set_notify(notify);
}

#[tauri::command]
fn play_sound(sound: String) {
    notify::play(&sound);
}

#[tauri::command]
fn sounds() -> Vec<&'static str> {
    notify::SOUNDS.to_vec()
}

// MARK: Commands: AI

#[tauri::command]
async fn ai_providers() -> Vec<ProviderStatus> {
    ai::statuses().await
}

#[tauri::command]
fn ai_config(prefs: State<'_, PrefsStore>) -> AiConfig {
    prefs.ai()
}

#[tauri::command]
fn set_ai_config(app: AppHandle, prefs: State<'_, PrefsStore>, config: AiConfig) {
    prefs.set_ai(config);
    let _ = app.emit("ai-config-changed", ());
    tray::refresh(&app);
}

#[tauri::command]
fn set_ai_key(provider: Provider, key: String) -> CmdResult<()> {
    ai::set_key(provider, &key)
}

#[tauri::command]
fn clear_ai_key(provider: Provider) -> CmdResult<()> {
    ai::clear_key(provider)
}

#[tauri::command]
async fn test_ai(prefs: State<'_, PrefsStore>) -> CmdResult<TestResult> {
    let config = prefs.ai();
    ai::test(&config).await
}

#[tauri::command]
async fn review_state(app: AppHandle, root: String) -> ReviewState {
    review_state_for(&app, Path::new(&root)).await
}

/// Reviews with the configured AI; `provider` overrides which one for this run only.
#[tauri::command]
async fn start_review(app: AppHandle, root: String, provider: Option<Provider>) -> CmdResult<()> {
    let mut config = app.state::<PrefsStore>().ai();
    if let Some(provider) = provider {
        if provider != config.provider {
            config.provider = provider;
            config.model.clear();
        }
    }
    start_review_task(&app, PathBuf::from(root), config).await
}

#[tauri::command]
async fn cancel_review(app: AppHandle, root: String) {
    let root = PathBuf::from(root);
    // Dropping the task kills the agent process (kill_on_drop).
    if let Some(task) = app
        .state::<AppState>()
        .review_tasks
        .lock()
        .unwrap()
        .remove(&root)
    {
        task.abort();
    }
    let state = app.state::<AppState>();
    let mut reviews = state.reviews.lock().unwrap();
    if matches!(reviews.get(&root), Some(ReviewState::Running { .. })) {
        reviews.remove(&root);
    }
    drop(reviews);
    emit_review(&app, &root);
}

// MARK: Commands: recent repositories

#[tauri::command]
fn recent_repositories(prefs: State<'_, PrefsStore>) -> Vec<String> {
    prefs
        .recents()
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

#[tauri::command]
fn remove_recent_repository(app: AppHandle, prefs: State<'_, PrefsStore>, path: String) {
    prefs.remove_recent(&path);
    tray::refresh(&app);
}

#[tauri::command]
fn import_recent_repositories(app: AppHandle, prefs: State<'_, PrefsStore>, paths: Vec<String>) {
    prefs.import_recents(paths);
    tray::refresh(&app);
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
            &item(
                "install-cli",
                &format!("Install '{}' Command in PATH", cli::CLI_NAME),
                None,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            // Our own Quit: closing the last window keeps Seam in the menu bar, so ⌘Q has to
            // exit explicitly instead of going through the "all windows closed" path.
            &item("quit", "Quit Seam", Some("CmdOrCtrl+Q"))?,
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
            &item("view-review", "Review", Some("CmdOrCtrl+3"))?,
            &item("view-visual", "Visual", Some("CmdOrCtrl+4"))?,
            &item("expand-diff", "Expand Diff", Some("CmdOrCtrl+Shift+E"))?,
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
            &item(
                "review-branch",
                "Review Branch with AI",
                Some("CmdOrCtrl+Shift+R"),
            )?,
            &item(
                "recap-branch",
                "Recap Branch Visually",
                Some("CmdOrCtrl+Shift+V"),
            )?,
            &item("plan-change", "Plan a Change…", None)?,
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
    if tray::is_tray_event(id) {
        return;
    }
    match id {
        "open" => pick_and_open(app),
        // A repository window shows settings as a sheet over itself; anywhere else gets the window.
        "settings" if !focused_window(app).is_some_and(|w| w.label().starts_with("repo-")) => {
            let _ = show_settings(app, None);
        }
        "install-cli" => cli::install(app),
        "quit" => app.exit(0),
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
        .plugin(tauri_plugin_notification::init())
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
            open_settings_window,
            ai_providers,
            ai_config,
            set_ai_config,
            set_ai_key,
            clear_ai_key,
            test_ai,
            review_state,
            start_review,
            cancel_review,
            recent_repositories,
            remove_recent_repository,
            import_recent_repositories,
            visual_state,
            visual_history,
            visual_scope,
            open_visual,
            start_visual,
            cancel_visual,
            export_visual,
            print_window,
            notify_prefs,
            set_notify_prefs,
            play_sound,
            sounds,
            cli::cli_status,
            cli::set_cli_installed,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Destroyed => {
                let state = window.state::<AppState>();
                state.watchers.lock().unwrap().remove(window.label());
                state.window_roots.lock().unwrap().remove(window.label());
            }
            tauri::WindowEvent::Focused(true) => {
                let state = window.state::<AppState>();
                let root = state
                    .window_roots
                    .lock()
                    .unwrap()
                    .get(window.label())
                    .cloned();
                if root.is_some() {
                    *state.last_repo.lock().unwrap() = root;
                    tray::refresh(window.app_handle());
                }
            }
            _ => {}
        })
        .setup(|app| {
            let prefs_dir = app.path().app_data_dir()?;
            app.manage(PrefsStore::load(&prefs_dir));
            tray::create(app.handle())?;
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
                        if open_repo_window(&handle, &p, None).await.is_err() {
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
        // Closing the last window keeps Seam in the menu bar; Quit passes an exit code.
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        // Folders dropped on the Dock icon or opened with `open -a Seam <dir>`.
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            for url in urls {
                if let Ok(path) = url.to_file_path() {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = open_repo_window(&handle, &path, None).await {
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
