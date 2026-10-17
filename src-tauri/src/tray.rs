//! The menu bar icon: recent projects, and a review of the current branch one click away.

use std::path::{Path, PathBuf};

use tauri::image::Image;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_opener::OpenerExt;

use crate::git::Repo;
use crate::prefs::PrefsStore;
use crate::review::ReviewState;
use crate::AppState;

const TRAY_ID: &str = "seam";
const MAX_RECENTS: usize = 8;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Seam")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .build(app)?;
    refresh(app);
    Ok(())
}

/// Rebuilds the menu in the background; call it whenever what it shows may have changed.
pub fn refresh(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(menu) = build(&app).await else { return };
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(menu));
        }
    });
}

fn name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

async fn branch(root: &Path) -> String {
    let repo = Repo::new(root);
    match repo.current_branch().await {
        Some(branch) => branch,
        None => repo
            .head()
            .await
            .map(|sha| sha[..7].to_string())
            .unwrap_or_else(|| "no commits".into()),
    }
}

/// One repository as the menu shows it.
struct Entry {
    root: PathBuf,
    label: String,
    review: ReviewState,
}

async fn entry(app: &AppHandle, root: PathBuf) -> Entry {
    let label = format!("{} · {}", name(&root), branch(&root).await);
    let review = crate::review_state_for(app, &root).await;
    Entry {
        root,
        label,
        review,
    }
}

/// Gathers everything that needs git first, so the menu itself is built without awaiting.
async fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let (agent, recents, current) = {
        let prefs = app.state::<PrefsStore>();
        let recents: Vec<PathBuf> = prefs.recents().into_iter().take(MAX_RECENTS).collect();
        let last = app.state::<AppState>().last_repo.lock().unwrap().clone();
        let current = last
            .filter(|root| root.is_dir())
            .or_else(|| recents.first().cloned());
        (prefs.ai().provider.label(), recents, current)
    };
    let current = match current {
        Some(root) => Some(entry(app, root).await),
        None => None,
    };
    let mut entries = Vec::new();
    for root in recents {
        entries.push(entry(app, root).await);
    }
    menu(app, agent, current, entries)
}

fn menu(
    app: &AppHandle,
    agent: &str,
    current: Option<Entry>,
    recents: Vec<Entry>,
) -> tauri::Result<Menu<Wry>> {
    let mut targets: Vec<PathBuf> = Vec::new();
    let mut target = |root: &Path| {
        targets.push(root.to_path_buf());
        targets.len() - 1
    };
    let item = |id: String, text: String, enabled: bool| {
        MenuItem::with_id(app, id, text, enabled, None::<&str>)
    };
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();

    if let Some(current) = &current {
        let index = target(&current.root);
        match &current.review {
            ReviewState::Running { provider, .. } => {
                items.push(Box::new(item(
                    "none".into(),
                    format!("Reviewing {} with {}…", current.label, provider.label()),
                    false,
                )?));
                items.push(Box::new(item(
                    format!("show:{index}"),
                    "Show Progress".into(),
                    true,
                )?));
            }
            ReviewState::Done { review } => {
                items.push(Box::new(item(
                    format!("review:{index}"),
                    format!("Review {} with {agent}", current.label),
                    true,
                )?));
                let count = review.findings.len();
                let plural = if count == 1 { "" } else { "s" };
                items.push(Box::new(item(
                    format!("show:{index}"),
                    format!("Show Last Review · {count} finding{plural}"),
                    true,
                )?));
            }
            _ => {
                items.push(Box::new(item(
                    format!("review:{index}"),
                    format!("Review {} with {agent}", current.label),
                    true,
                )?));
            }
        }
        items.push(Box::new(PredefinedMenuItem::separator(app)?));
    }

    if !recents.is_empty() {
        items.push(Box::new(item(
            "none".into(),
            "Recent Projects".into(),
            false,
        )?));
        for recent in &recents {
            let index = target(&recent.root);
            let running = matches!(recent.review, ReviewState::Running { .. });
            let submenu = Submenu::with_items(
                app,
                recent.label.replacen(" · ", "    ", 1),
                true,
                &[
                    &item(format!("open:{index}"), "Open".into(), true)?,
                    &item(
                        format!("review:{index}"),
                        format!("Review Branch with {agent}"),
                        !running,
                    )?,
                    &item(format!("finder:{index}"), "Show in Finder".into(), true)?,
                ],
            )?;
            items.push(Box::new(submenu));
        }
        items.push(Box::new(PredefinedMenuItem::separator(app)?));
    }

    items.push(Box::new(item(
        "open-repository".into(),
        "Open Repository…".into(),
        true,
    )?));
    items.push(Box::new(item(
        "welcome".into(),
        "Show Welcome Window".into(),
        true,
    )?));
    items.push(Box::new(MenuItem::with_id(
        app,
        "settings",
        "Settings…",
        true,
        Some("CmdOrCtrl+,"),
    )?));
    items.push(Box::new(PredefinedMenuItem::separator(app)?));
    items.push(Box::new(MenuItem::with_id(
        app,
        "quit",
        "Quit Seam",
        true,
        Some("CmdOrCtrl+Q"),
    )?));

    *app.state::<AppState>().tray_targets.lock().unwrap() = targets;
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|item| item.as_ref()).collect();
    Menu::with_items(app, &refs)
}

fn on_menu(app: &AppHandle, id: &str) {
    let target = |index: &str| -> Option<PathBuf> {
        let index: usize = index.parse().ok()?;
        app.state::<AppState>()
            .tray_targets
            .lock()
            .unwrap()
            .get(index)
            .cloned()
    };
    match id.split_once(':') {
        Some(("open", index)) => {
            if let Some(root) = target(index) {
                crate::spawn_open(app, root, None);
            }
        }
        Some(("show", index)) => {
            if let Some(root) = target(index) {
                crate::spawn_open(app, root, Some("review"));
            }
        }
        Some(("review", index)) => {
            if let Some(root) = target(index) {
                let config = app.state::<PrefsStore>().ai();
                crate::spawn_open(app, root.clone(), Some("review"));
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::start_review_task(&app, root, config).await;
                });
            }
        }
        Some(("finder", index)) => {
            if let Some(root) = target(index) {
                let _ = app.opener().open_path(root.to_string_lossy(), None::<&str>);
            }
        }
        _ => match id {
            "open-repository" => crate::pick_and_open(app),
            "welcome" => {
                let _ = crate::show_welcome(app);
            }
            "settings" => {
                let _ = crate::show_settings(app, None);
            }
            "quit" => app.exit(0),
            _ => {}
        },
    }
}
