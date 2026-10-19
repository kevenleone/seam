use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Map, Value};
use tokio::process::Command;

use crate::ai::{find_program, Provider};
use crate::git::process::base_env;

const RUN_TIMEOUT: Duration = Duration::from_secs(20);

/// How an agent starts this server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
}

impl Launch {
    /// The launcher inside the app bundle, so the server outlives a removed `/usr/local/bin`
    /// link; a development build has no bundle and runs its own binary.
    pub fn current() -> Option<Self> {
        let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
        let bundle = exe
            .ancestors()
            .find(|dir| dir.extension().is_some_and(|extension| extension == "app"));
        Some(match bundle {
            Some(bundle) => Launch {
                command: bundle
                    .join("Contents/Resources/resources")
                    .join(crate::cli::CLI_NAME)
                    .to_string_lossy()
                    .into_owned(),
                args: vec!["mcp".into()],
            },
            None => Launch {
                command: exe.to_string_lossy().into_owned(),
                args: vec!["--cli".into(), "mcp".into()],
            },
        })
    }

    /// `seam mcp` by hand, through the PATH link, counts as connected too.
    fn matches(&self, other: &Launch) -> bool {
        self == other
            || (Path::new(&other.command).file_name()
                == Some(std::ffi::OsStr::new(crate::cli::CLI_NAME))
                && other.args == ["mcp"])
    }
}

/// `seam`, or `seam-dev` for the development build, so each reaches its own app.
pub fn server_name(identifier: &str) -> String {
    if identifier.ends_with(".dev") {
        format!("{}-dev", crate::cli::CLI_NAME)
    } else {
        crate::cli::CLI_NAME.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    Off,
    On,
    /// Registered, but starting something other than this app (it moved, or another copy).
    Stale,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub id: &'static str,
    pub label: &'static str,
    pub installed: bool,
    pub connection: Connection,
    pub server: String,
}

pub const AGENTS: [Provider; 3] = [Provider::Claude, Provider::Codex, Provider::OpenCode];

pub fn agent(id: &str) -> Result<Provider, String> {
    AGENTS
        .into_iter()
        .find(|agent| agent.id() == id)
        .ok_or_else(|| {
            let ids: Vec<&str> = AGENTS.iter().map(|agent| agent.id()).collect();
            format!("no agent `{id}`; pick one of {}", ids.join(", "))
        })
}

fn home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}

pub async fn status(agent: Provider, name: &str, launch: &Launch) -> AgentStatus {
    let installed = find_program(agent.id()).await.is_some();
    let connection = match registered(agent, name).await {
        _ if !installed => Connection::Off,
        None => Connection::Off,
        Some(found) if launch.matches(&found) => Connection::On,
        Some(_) => Connection::Stale,
    };
    AgentStatus {
        id: agent.id(),
        label: agent.label(),
        installed,
        connection,
        server: name.to_string(),
    }
}

pub async fn statuses(name: &str, launch: &Launch) -> Vec<AgentStatus> {
    let mut statuses = Vec::new();
    for agent in AGENTS {
        statuses.push(status(agent, name, launch).await);
    }
    statuses
}

/// Registers this server with the agent, or removes it, through the agent's own `mcp`
/// commands where it has them. Neither Claude Code nor Codex replaces a server by name, so a
/// stale entry goes first.
pub async fn connect(
    agent: Provider,
    name: &str,
    launch: &Launch,
    on: bool,
) -> Result<AgentStatus, String> {
    let binary = find_program(agent.id())
        .await
        .ok_or_else(|| format!("{} is not installed on this machine.", agent.label()))?;
    match agent {
        Provider::OpenCode => {
            let path = opencode_config(&home());
            set_opencode_server(&path, name, on.then_some(launch))?;
        }
        _ => {
            if registered(agent, name).await.is_some() {
                run(agent, &binary, &remove_arguments(agent, name)).await?;
            }
            if on {
                run(agent, &binary, &add_arguments(agent, name, launch)).await?;
            }
        }
    }
    Ok(status(agent, name, launch).await)
}

fn add_arguments(agent: Provider, name: &str, launch: &Launch) -> Vec<String> {
    let mut arguments: Vec<String> = match agent {
        Provider::Claude => vec!["mcp", "add", "--scope", "user", name, "--"],
        _ => vec!["mcp", "add", name, "--"],
    }
    .into_iter()
    .map(String::from)
    .collect();
    arguments.push(launch.command.clone());
    arguments.extend(launch.args.iter().cloned());
    arguments
}

fn remove_arguments(agent: Provider, name: &str) -> Vec<String> {
    match agent {
        Provider::Claude => vec!["mcp", "remove", "--scope", "user", name],
        _ => vec!["mcp", "remove", name],
    }
    .into_iter()
    .map(String::from)
    .collect()
}

async fn run(agent: Provider, binary: &Path, arguments: &[String]) -> Result<String, String> {
    let output = tokio::time::timeout(
        RUN_TIMEOUT,
        Command::new(binary)
            .args(arguments)
            .envs(base_env().await)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| format!("{} took too long to answer.", agent.label()))?
    .map_err(|error| format!("couldn't run {}: {error}", agent.label()))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(match (stderr.is_empty(), stdout.is_empty()) {
        (false, _) => stderr,
        (true, false) => stdout,
        (true, true) => format!("`{} {}` failed.", agent.id(), arguments.join(" ")),
    })
}

async fn registered(agent: Provider, name: &str) -> Option<Launch> {
    match agent {
        Provider::Claude => {
            let text = std::fs::read_to_string(home().join(".claude.json")).ok()?;
            let config: Value = serde_json::from_str(&text).ok()?;
            launch_of(&config["mcpServers"][name])
        }
        Provider::Codex => {
            let binary = find_program(agent.id()).await?;
            let arguments = ["mcp", "get", name, "--json"].map(String::from);
            let text = run(agent, &binary, &arguments).await.ok()?;
            let config: Value = serde_json::from_str(&text).ok()?;
            launch_of(&config["transport"])
        }
        Provider::OpenCode => {
            let text = std::fs::read_to_string(opencode_config(&home())).ok()?;
            let (config, _) = parse_jsonc(&text).ok()?;
            let command = config["mcp"][name]["command"].as_array()?;
            let mut parts = command.iter().filter_map(Value::as_str).map(String::from);
            Some(Launch {
                command: parts.next()?,
                args: parts.collect(),
            })
        }
        _ => None,
    }
}

fn launch_of(server: &Value) -> Option<Launch> {
    Some(Launch {
        command: server["command"].as_str()?.to_string(),
        args: server["args"]
            .as_array()
            .map(|args| {
                args.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// The file OpenCode reads its global settings from: whichever exists, else a new `opencode.json`.
fn opencode_config(home: &Path) -> PathBuf {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("opencode");
    ["opencode.jsonc", "opencode.json", "config.json"]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| dir.join("opencode.json"))
}

/// JSON with comments and trailing commas, as OpenCode accepts it. Also says whether there were
/// comments, since writing the file back would drop them.
fn parse_jsonc(text: &str) -> Result<(Value, bool), String> {
    let mut plain = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    let mut in_string = false;
    let mut comments = false;
    while let Some(character) = characters.next() {
        if in_string {
            plain.push(character);
            match character {
                '\\' => plain.extend(characters.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (character, characters.peek()) {
            ('"', _) => {
                in_string = true;
                plain.push(character);
            }
            ('/', Some('/')) => {
                comments = true;
                for skipped in characters.by_ref() {
                    if skipped == '\n' {
                        plain.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                comments = true;
                characters.next();
                let mut previous = ' ';
                for skipped in characters.by_ref() {
                    if previous == '*' && skipped == '/' {
                        break;
                    }
                    previous = skipped;
                }
            }
            (',', _) => {
                let rest: String = characters.clone().collect();
                let next = rest.trim_start().chars().next();
                if !matches!(next, Some('}' | ']')) {
                    plain.push(',');
                }
            }
            _ => plain.push(character),
        }
    }
    let value = if plain.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&plain).map_err(|error| error.to_string())?
    };
    Ok((value, comments))
}

fn set_opencode_server(path: &Path, name: &str, launch: Option<&Launch>) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let (mut config, comments) =
        parse_jsonc(&text).map_err(|error| format!("couldn't read {}: {error}", path.display()))?;
    let entry = launch.map(|launch| {
        let mut command = vec![launch.command.clone()];
        command.extend(launch.args.iter().cloned());
        json!({ "type": "local", "command": command, "enabled": true })
    });
    if comments {
        let what = match &entry {
            Some(entry) => format!(
                "add this under \"mcp\" yourself:\n\n  \"{name}\": {}",
                serde_json::to_string(entry).unwrap_or_default()
            ),
            None => format!("remove \"{name}\" from \"mcp\" yourself."),
        };
        return Err(format!(
            "{} has comments that rewriting it would lose; {what}",
            path.display()
        ));
    }
    let Some(object) = config.as_object_mut() else {
        return Err(format!("{} isn't a JSON object.", path.display()));
    };
    let servers = object
        .entry("mcp")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(servers) = servers.as_object_mut() else {
        return Err(format!("\"mcp\" in {} isn't an object.", path.display()));
    };
    match entry {
        Some(entry) => {
            servers.insert(name.to_string(), entry);
        }
        None => {
            servers.remove(name);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let mut written = serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?;
    written.push('\n');
    let partial = path.with_extension("seam-partial");
    std::fs::write(&partial, written)
        .and_then(|_| std::fs::rename(&partial, path))
        .map_err(|error| format!("couldn't write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch() -> Launch {
        Launch {
            command: "/Applications/Seam.app/Contents/Resources/resources/seam".into(),
            args: vec!["mcp".into()],
        }
    }

    #[test]
    fn reads_json_with_comments_and_trailing_commas() {
        let (value, comments) = parse_jsonc(
            "{\n  // mine\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"a\": [1, 2,],\n  /* b */ \"b\": \"x // y\",\n}",
        )
        .unwrap();
        assert!(comments);
        assert_eq!(
            value,
            json!({ "$schema": "https://opencode.ai/config.json", "a": [1, 2], "b": "x // y" })
        );
        let (_, comments) = parse_jsonc("{\"url\": \"https://x\"}").unwrap();
        assert!(!comments);
        assert_eq!(parse_jsonc("").unwrap().0, json!({}));
    }

    #[test]
    fn adds_and_removes_itself_in_opencode_keeping_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode/opencode.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "{\"$schema\": \"https://opencode.ai/config.json\", \"theme\": \"x\"}",
        )
        .unwrap();

        set_opencode_server(&path, "seam", Some(&launch())).unwrap();
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["theme"], "x");
        assert_eq!(
            written["mcp"]["seam"]["command"],
            json!([launch().command, "mcp"])
        );
        let keys: Vec<&String> = written.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["$schema", "theme", "mcp"]);

        set_opencode_server(&path, "seam", None).unwrap();
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["mcp"], json!({}));
    }

    #[test]
    fn leaves_a_commented_opencode_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.jsonc");
        let text = "{\n  // keep me\n  \"theme\": \"x\"\n}\n";
        std::fs::write(&path, text).unwrap();
        let error = set_opencode_server(&path, "seam", Some(&launch())).unwrap_err();
        assert!(error.contains("\"seam\": {"), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn counts_a_hand_made_seam_mcp_as_connected() {
        let by_hand = Launch {
            command: "/usr/local/bin/seam".into(),
            args: vec!["mcp".into()],
        };
        assert!(launch().matches(&by_hand));
        assert!(launch().matches(&launch()));
        let moved = Launch {
            command: "/Users/me/Downloads/Seam.app/Contents/Resources/resources/seam".into(),
            args: vec!["serve".into()],
        };
        assert!(!launch().matches(&moved));
        assert_eq!(server_name("app.seam.desktop"), "seam");
        assert_eq!(server_name("app.seam.desktop.dev"), "seam-dev");
    }

    #[test]
    fn registers_through_each_agents_own_command() {
        assert_eq!(
            add_arguments(Provider::Claude, "seam", &launch()).join(" "),
            format!("mcp add --scope user seam -- {} mcp", launch().command)
        );
        assert_eq!(
            add_arguments(Provider::Codex, "seam", &launch()).join(" "),
            format!("mcp add seam -- {} mcp", launch().command)
        );
        assert_eq!(
            remove_arguments(Provider::Claude, "seam").join(" "),
            "mcp remove --scope user seam"
        );
    }
}
