mod args;
mod difftool;
mod doctor;
pub mod install;
mod term;

use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::ai::{AiConfig, Provider};
use crate::git::process::{git, inherit_env, RunOptions};
use crate::git::Repo;
use crate::handoff::{self, Action, Request};
use crate::prefs::PrefsStore;
use crate::review::{self, Scope, Severity, Target};
use args::{Command, Subject};
use term::Paint;

pub use install::CLI_NAME;

type Outcome = Result<i32, String>;

/// The app this binary belongs to: its name, where it keeps its preferences, and its bundle.
pub(crate) struct App {
    pub name: String,
    identifier: String,
    pub bundle: Option<PathBuf>,
}

impl App {
    fn current() -> Self {
        let context = crate::context();
        let config = context.config();
        App {
            name: config.product_name.clone().unwrap_or_default(),
            identifier: config.identifier.clone(),
            bundle: std::env::current_exe()
                .and_then(|exe| exe.canonicalize())
                .ok()
                .and_then(|exe| {
                    exe.ancestors()
                        .find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
                        .map(Path::to_path_buf)
                }),
        }
    }

    /// Where Tauri's `app_data_dir` is on macOS.
    fn data_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
        home.join("Library/Application Support")
            .join(&self.identifier)
    }

    pub fn ai_config(&self) -> AiConfig {
        PrefsStore::load(&self.data_dir()).ai()
    }

    pub fn launcher(&self) -> Option<PathBuf> {
        Some(
            self.bundle
                .as_ref()?
                .join("Contents/Resources/resources")
                .join(CLI_NAME),
        )
    }

    /// Through `open`, so a running app receives the folder the way it does from Finder.
    /// A development build has no bundle to open, so it starts another copy of itself.
    fn open(&self, folder: Option<&Path>) -> Result<(), String> {
        let result = match &self.bundle {
            Some(bundle) => {
                let mut command = std::process::Command::new("/usr/bin/open");
                command.arg("-a").arg(bundle).args(folder);
                command.status().and_then(|status| {
                    status
                        .success()
                        .then_some(())
                        .ok_or_else(|| std::io::Error::other("open failed"))
                })
            }
            None => std::env::current_exe().and_then(|exe| {
                std::process::Command::new(exe)
                    .args(folder)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map(|_| ())
            }),
        };
        result.map_err(|error| format!("couldn't open {}: {error}", self.name))
    }
}

pub fn main() -> ! {
    let arguments: Vec<String> = std::env::args().skip(2).collect();
    let command = match args::parse(&arguments) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{CLI_NAME}: {message}\nRun `{CLI_NAME} --help` for usage.");
            std::process::exit(2);
        }
    };
    inherit_env();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let code = match runtime.block_on(run(command)) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{CLI_NAME}: {message}");
            1
        }
    };
    std::process::exit(code);
}

async fn run(command: Command) -> Outcome {
    let app = App::current();
    match command {
        Command::Help => {
            print!("{}", help(Paint::stdout()));
            Ok(0)
        }
        Command::Version => {
            println!("{CLI_NAME} {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Command::Open { subject, agent } => open(&app, subject, agent).await,
        Command::Review {
            subject,
            agent,
            print,
            last,
        } => match (last, print) {
            (true, _) => open_last_review(&app).await,
            (false, true) => print_review(&app, subject, agent).await,
            (false, false) => open_review(&app, subject, agent).await,
        },
        Command::Explain {
            revision,
            agent,
            print,
        } => explain(&app, revision, agent, print).await,
        Command::Doctor => Ok(doctor::run(&app).await),
        Command::GitInstall { default, uninstall } => git_install(default, uninstall).await,
        Command::Difftool {
            left,
            right,
            merged,
        } => open_difftool(&app, &left, &right, merged.as_deref()).await,
    }
}

fn help(paint: Paint) -> String {
    let rows = |rows: &[(&str, &str)], width: usize| -> String {
        rows.iter()
            .map(|(usage, what)| format!("  {usage:width$}  {}\n", paint.dim(what)))
            .collect()
    };
    format!(
        "{} — open a git diff, review it with the agent you already use\n\n\
         {}\n  seam [<range>] [--staged] [-- <path>…]\n\n\
         {}\n{}\n{}\n{}{}\n{}\n{}",
        paint.bold(CLI_NAME),
        paint.heading("USAGE"),
        paint.heading("OPEN"),
        rows(
            &[
                ("seam", "working tree vs HEAD"),
                ("seam --staged", "what you are about to commit"),
                ("seam main..HEAD", "this branch against main"),
                ("seam HEAD~3", "the last three commits"),
                ("seam pr 128", "a pull or merge request, via gh or glab"),
                ("git diff | seam -", "any patch on stdin"),
            ],
            26,
        ),
        paint.heading("REVIEW"),
        rows(
            &[
                (
                    "seam review [<range>]",
                    "open with AI comments already in place"
                ),
                ("seam review --last", "open the last review as it was"),
                (
                    "seam explain <commit>",
                    "plain-language summary of a change"
                ),
            ],
            26,
        ),
        rows(
            &[
                (
                    "  --agent claude|opencode|codex",
                    "override the default agent"
                ),
                ("  --print", "stay in the terminal, no window"),
            ],
            32,
        ),
        paint.heading("SETUP"),
        rows(
            &[
                ("seam doctor", "find agents on PATH, check git"),
                ("seam git-install", "register as git difftool"),
            ],
            26,
        ),
    )
}

fn provider(agent: Option<String>) -> Result<Option<Provider>, String> {
    let Some(agent) = agent else {
        return Ok(None);
    };
    Provider::ALL
        .into_iter()
        .find(|provider| provider.id() == agent)
        .map(Some)
        .ok_or_else(|| {
            let ids: Vec<&str> = Provider::ALL.iter().map(|provider| provider.id()).collect();
            format!("no agent `{agent}`; pick one of {}", ids.join(", "))
        })
}

/// The configured AI, with `--agent` swapped in for this run the way the app's picker does.
fn config(app: &App, agent: Option<Provider>) -> AiConfig {
    let mut config = app.ai_config();
    if let Some(agent) = agent {
        if agent != config.provider {
            config.provider = agent;
            config.model.clear();
        }
    }
    config
}

struct Change {
    repo: Repo,
    git_dir: PathBuf,
    target: Target,
}

enum Opened {
    Folder(PathBuf),
    Change(Change),
}

async fn repository() -> Result<(Repo, PathBuf), String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let root = Repo::top_level(&cwd)
        .await
        .ok_or("not inside a git repository")?;
    let repo = Repo::new(root);
    let git_dir = repo.git_dir().await.map_err(|error| error.message)?;
    Ok((repo, git_dir))
}

async fn is_revision(revision: &str) -> bool {
    let cwd = std::env::current_dir().unwrap_or_default();
    let commit = format!("{revision}^{{commit}}");
    git(
        &cwd,
        &["rev-parse", "--verify", "--quiet", &commit],
        RunOptions {
            allow_failure: true,
            read: true,
            ..Default::default()
        },
    )
    .await
    .is_ok_and(|output| output.code == 0)
}

async fn resolve(subject: Subject) -> Result<Opened, String> {
    if let Some(revision) = &subject.revision {
        let path = Path::new(revision);
        if path.is_dir() && subject.paths.is_empty() && !is_revision(revision).await {
            return path
                .canonicalize()
                .map(Opened::Folder)
                .map_err(|error| error.to_string());
        }
    }
    let (repo, git_dir) = repository().await?;
    let Subject {
        revision,
        staged,
        stdin,
        pull_request,
        paths,
    } = subject;
    let target = if stdin {
        if std::io::stdin().is_terminal() {
            return Err("`-` reads a patch from a pipe, e.g. `git diff | seam -`".into());
        }
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("couldn't read stdin: {error}"))?;
        let file = handoff::keep_patch(&git_dir, &text).map_err(|error| error.to_string())?;
        Target::Patch {
            file,
            label: "Patch from stdin".into(),
        }
    } else if let Some(number) = pull_request {
        Target::PullRequest { number }
    } else if staged {
        Target::Staged { paths }
    } else if let Some(range) = revision {
        Target::Range { range, paths }
    } else {
        Target::WorkingTree { paths }
    };
    Ok(Opened::Change(Change {
        repo,
        git_dir,
        target,
    }))
}

async fn scope(change: &Change) -> Result<Scope, String> {
    review::scope_of(&change.repo, &change.target)
        .await
        .map_err(|error| error.message)
}

fn stat(paint: Paint, scope: &Scope) -> String {
    let files = scope.diffs.len();
    let additions: u32 = scope.diffs.iter().map(|diff| diff.additions).sum();
    let deletions: u32 = scope.diffs.iter().map(|diff| diff.deletions).sum();
    format!(
        "{files} file{} {} {}",
        if files == 1 { "" } else { "s" },
        paint.added(&format!("+{additions}")),
        paint.deleted(&format!("-{deletions}")),
    )
}

fn hand_off(app: &App, change: &Change, request: Request) -> Result<(), String> {
    handoff::send(&change.git_dir, &request).map_err(|error| error.to_string())?;
    app.open(Some(&change.repo.root))
}

async fn open(app: &App, subject: Subject, agent: Option<String>) -> Outcome {
    let agent = provider(agent)?;
    let nothing_named = subject == Subject::default();
    if nothing_named && repository().await.is_err() {
        app.open(None)?;
        return Ok(0);
    }
    let change = match resolve(subject).await? {
        Opened::Folder(folder) => {
            app.open(Some(&folder))?;
            println!("opened {} in {}", folder.display(), app.name);
            return Ok(0);
        }
        Opened::Change(change) => change,
    };
    let paint = Paint::stdout();
    let scope = scope(&change).await?;
    let config = config(app, agent);
    let (action, queued) = if config.review_on_open {
        (
            Action::Review,
            format!(" · review queued with {}", config.provider.id()),
        )
    } else {
        (Action::Open, String::new())
    };
    hand_off(
        app,
        &change,
        Request::new(action, change.target.clone(), agent),
    )?;
    println!("{} · opened in {}{queued}", stat(paint, &scope), app.name);
    Ok(0)
}

async fn open_review(app: &App, subject: Subject, agent: Option<String>) -> Outcome {
    let agent = provider(agent)?;
    let Opened::Change(change) = resolve(subject).await? else {
        return Err("review takes a range, not a folder; run it inside the repository".into());
    };
    let scope = scope(&change).await?;
    let config = config(app, agent);
    hand_off(
        app,
        &change,
        Request::new(Action::Review, change.target.clone(), agent),
    )?;
    println!(
        "{} · opened in {} · review queued with {}",
        stat(Paint::stdout(), &scope),
        app.name,
        config.provider.id()
    );
    Ok(0)
}

async fn open_last_review(app: &App) -> Outcome {
    let (repo, git_dir) = repository().await?;
    let saved = review::load(&git_dir)
        .ok_or("no review saved here yet; run `seam review` or `seam review --print`")?;
    let change = Change {
        repo,
        git_dir,
        target: saved.target.clone(),
    };
    hand_off(
        app,
        &change,
        Request::new(Action::ShowReview, saved.target, None),
    )?;
    let count = saved.findings.len();
    println!(
        "{} · {count} finding{} · opened in {}",
        saved.scope,
        if count == 1 { "" } else { "s" },
        app.name
    );
    Ok(0)
}

async fn print_review(app: &App, subject: Subject, agent: Option<String>) -> Outcome {
    let agent = provider(agent)?;
    let Opened::Change(change) = resolve(subject).await? else {
        return Err("review takes a range, not a folder; run it inside the repository".into());
    };
    let scope = scope(&change).await?;
    let config = config(app, agent);
    let files = scope.diffs.len();
    let label = format!(
        "{} · reading {files} file{}",
        config.provider.id(),
        if files == 1 { "" } else { "s" }
    );
    let review = term::progress(&label, review::run(&config, &change.repo.root, scope))
        .await
        .map_err(|error| error.message)?;
    review::save(&change.git_dir, &review);

    let paint = Paint::stdout();
    let mut findings = review.findings.clone();
    findings.sort_by_key(|finding| finding.severity as u8);
    let place = |finding: &review::Finding| match finding.line {
        Some(line) => format!("{}:{line}", finding.file),
        None => finding.file.clone(),
    };
    let width = findings
        .iter()
        .map(|finding| place(finding).chars().count())
        .max()
        .unwrap_or(0);
    for finding in &findings {
        let severity = match finding.severity {
            Severity::Risk => paint.deleted("risk "),
            Severity::Check => paint.added("check"),
            Severity::Note => "note ".to_string(),
        };
        println!("  {severity}  {:width$}  {}", place(finding), finding.title);
    }
    if findings.is_empty() {
        println!("  no findings · {}", review.summary);
    }
    println!(
        "open in app: {}",
        paint.accent(&format!("{CLI_NAME} review --last"))
    );
    let risky = findings
        .iter()
        .any(|finding| finding.severity == Severity::Risk);
    Ok(if risky { 1 } else { 0 })
}

async fn explain(app: &App, revision: String, agent: Option<String>, print: bool) -> Outcome {
    let agent = provider(agent)?;
    let (repo, git_dir) = repository().await?;
    let change = Change {
        repo,
        git_dir,
        target: Target::Commit { rev: revision },
    };
    let scope = scope(&change).await?;
    let config = config(app, agent);
    let commit = scope.commits.first().cloned().unwrap_or_default();
    if !print {
        hand_off(
            app,
            &change,
            Request::new(Action::Explain, change.target.clone(), agent),
        )?;
        println!(
            "{commit} · opened in {} · recap queued with {}",
            app.name,
            config.provider.id()
        );
        return Ok(0);
    }
    let label = format!("{} · explaining {}", config.provider.id(), scope.label);
    let text = term::progress(
        &label,
        crate::explain::run(&config, &change.repo.root, &scope),
    )
    .await
    .map_err(|error| error.message)?;
    println!(
        "{}\n\n{}",
        Paint::stdout().bold(&commit),
        term::wrap(&text, "")
    );
    Ok(0)
}

async fn git_install(default: bool, uninstall: bool) -> Outcome {
    let paint = Paint::stdout();
    if uninstall {
        difftool::unregister(&["--global"])
            .await
            .map_err(|error| error.message)?;
        println!(
            "removed the {} difftool from your git config",
            difftool::TOOL
        );
        return Ok(0);
    }
    let link = install::link();
    let program = if link.is_symlink() {
        link.to_string_lossy().into_owned()
    } else {
        CLI_NAME.to_string()
    };
    difftool::register(&["--global"], &program, default)
        .await
        .map_err(|error| error.message)?;
    println!(
        "registered {} as a git difftool{}",
        difftool::TOOL,
        if default {
            " (and the default one)"
        } else {
            ""
        }
    );
    let example = if default {
        "git difftool -d main".to_string()
    } else {
        format!("git difftool -d -t {} main", difftool::TOOL)
    };
    println!("try: {}", paint.accent(&example));
    Ok(0)
}

async fn open_difftool(app: &App, left: &str, right: &str, merged: Option<&str>) -> Outcome {
    let (repo, git_dir) = repository().await?;
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let text = difftool::patch(&cwd, left, right, merged)
        .await
        .map_err(|error| error.message)?;
    if text.trim().is_empty() {
        println!("no differences");
        return Ok(0);
    }
    let file = handoff::keep_patch(&git_dir, &text).map_err(|error| error.to_string())?;
    let change = Change {
        repo,
        git_dir,
        target: Target::Patch {
            file,
            label: merged.unwrap_or("git difftool").to_string(),
        },
    };
    let scope = scope(&change).await?;
    hand_off(
        app,
        &change,
        Request::new(Action::Open, change.target.clone(), None),
    )?;
    println!("{} · opened in {}", stat(Paint::stdout(), &scope), app.name);
    Ok(0)
}
