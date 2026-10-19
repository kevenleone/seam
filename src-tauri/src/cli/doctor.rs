use super::term::Paint;
use super::{difftool, install, App};
use crate::ai::{self, Kind};
use crate::forge::{self, Forge};
use crate::git::process::{git, git_executable, RunOptions};
use crate::git::Repo;

struct Row {
    ok: bool,
    name: String,
    detail: String,
}

/// Every check, printed; fails only when git itself is missing.
pub async fn run(app: &App) -> i32 {
    let paint = Paint::stdout();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    let config = app.ai_config();
    let mut rows = Vec::new();

    let git_version = git(&cwd, &["--version"], RunOptions::default())
        .await
        .map(|output| output.text().trim().to_string());
    let has_git = git_version.is_ok();
    rows.push(Row {
        ok: has_git,
        name: "git".into(),
        detail: match git_version {
            Ok(version) => format!("{version} ({})", git_executable().display()),
            Err(error) => error.message,
        },
    });

    for status in ai::statuses().await {
        let chosen = status.id == config.provider;
        let mut detail = match (status.kind, &status.path) {
            (Kind::Harness, Some(path)) => path.clone(),
            (Kind::Harness, None) => "not on PATH".into(),
            (Kind::Api, _) if status.has_key => "key in Keychain".into(),
            (Kind::Api, _) if status.available => "no key needed".into(),
            (Kind::Api, _) => "no key (Settings › AI)".into(),
        };
        if chosen {
            let model = match config.model.trim() {
                "" => String::new(),
                model => format!(", {model}"),
            };
            detail = format!("{detail}  {}", paint.accent(&format!("default{model}")));
        }
        rows.push(Row {
            ok: status.available,
            name: status.id.id().into(),
            detail,
        });
    }

    let repo = Repo::top_level(&cwd).await.map(Repo::new);
    let host = match &repo {
        Some(repo) => forge::remote_host(repo).await.ok(),
        None => None,
    };
    for forge in Forge::ALL {
        let name = forge.program();
        let row = match ai::find_program(name).await {
            None => Row {
                ok: false,
                name: name.into(),
                detail: format!("not on PATH (needed for `seam pr` on {})", label(forge)),
            },
            Some(path) => {
                let relevant = host
                    .as_deref()
                    .filter(|host| forge::forge_by_name(host).is_none_or(|named| named == forge));
                let signed = match relevant {
                    Some(host) => {
                        let signed_in = forge::signed_in(forge, host, &cwd).await;
                        let state = if signed_in {
                            "signed in to"
                        } else {
                            "not signed in to"
                        };
                        format!(", {state} {host}")
                    }
                    None => String::new(),
                };
                Row {
                    ok: true,
                    name: name.into(),
                    detail: format!("{}{signed}", path.display()),
                }
            }
        };
        rows.push(row);
    }

    let link = install::link();
    let linked = app
        .launcher()
        .is_some_and(|script| install::points_at(&link, &script));
    rows.push(Row {
        ok: linked,
        name: "command".into(),
        detail: if linked {
            format!("{} → {}", link.display(), app.name)
        } else {
            format!(
                "{} doesn't point at this {} (menu › Install '{}' Command in PATH)",
                link.display(),
                app.name,
                install::CLI_NAME
            )
        },
    });

    let tool = difftool::get(&["--global"], &format!("difftool.{}.cmd", difftool::TOOL)).await;
    rows.push(Row {
        ok: tool.is_some(),
        name: "difftool".into(),
        detail: match tool {
            Some(_) => format!("registered (git difftool -d -t {})", difftool::TOOL),
            None => "not registered (seam git-install)".into(),
        },
    });

    rows.push(Row {
        ok: app.bundle.is_some(),
        name: "app".into(),
        detail: match &app.bundle {
            Some(bundle) => bundle.display().to_string(),
            None => "development build: opening starts another copy".into(),
        },
    });

    let width = rows.iter().map(|row| row.name.len()).max().unwrap_or(0);
    for row in rows {
        println!(
            "{} {:width$}  {}",
            paint.good(row.ok),
            row.name,
            if row.ok {
                row.detail
            } else {
                paint.dim(&row.detail)
            }
        );
    }
    if has_git {
        0
    } else {
        1
    }
}

fn label(forge: Forge) -> &'static str {
    match forge {
        Forge::GitHub => "GitHub",
        Forge::GitLab => "GitLab",
    }
}
