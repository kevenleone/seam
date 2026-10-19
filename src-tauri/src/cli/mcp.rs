use std::io::IsTerminal;

use super::args::McpCommand;
use super::term::Paint;
use super::{App, Outcome, CLI_NAME};
use crate::mcp::agents::{self, AgentStatus, Connection, Launch};
use crate::mcp::tools::Context;

pub async fn run(app: &App, command: McpCommand) -> Outcome {
    let name = agents::server_name(&app.identifier);
    let launch = Launch::current().ok_or("couldn't tell where this program is installed")?;
    match command {
        McpCommand::Serve if !std::io::stdin().is_terminal() => serve(app).await,
        McpCommand::Serve | McpCommand::Status => {
            let paint = Paint::stdout();
            println!(
                "{} is the server your agents start, to open diffs in {} and leave comments.\n",
                paint.bold(&format!("{CLI_NAME} mcp")),
                app.name
            );
            let statuses = agents::statuses(&name, &launch).await;
            for status in &statuses {
                println!("{}", row(paint, status));
            }
            if statuses
                .iter()
                .any(|status| status.installed && status.connection != Connection::On)
            {
                println!(
                    "\nconnect them: {}",
                    paint.accent(&format!("{CLI_NAME} mcp install"))
                );
            }
            Ok(0)
        }
        McpCommand::Install(chosen) => connect(&name, &launch, chosen, true).await,
        McpCommand::Uninstall(chosen) => connect(&name, &launch, chosen, false).await,
    }
}

async fn serve(app: &App) -> Outcome {
    let opener = app.clone();
    let context = Context {
        cwd: std::env::current_dir().map_err(|error| error.to_string())?,
        open: Box::new(move |root| opener.open(Some(root))),
        fallback_provider: app.ai_config().provider,
    };
    crate::mcp::serve(context)
        .await
        .map_err(|error| error.to_string())?;
    Ok(0)
}

fn row(paint: Paint, status: &AgentStatus) -> String {
    let (mark, state) = match (status.installed, status.connection) {
        (false, _) => (paint.dim("-"), paint.dim("not installed")),
        (true, Connection::On) => (paint.good(true), format!("connected as {}", status.server)),
        (true, Connection::Off) => (paint.good(false), "not connected".to_string()),
        (true, Connection::Stale) => (
            paint.good(false),
            format!(
                "starts another copy of {CLI_NAME}; {} points it here",
                paint.accent(&format!("{CLI_NAME} mcp install {}", status.id))
            ),
        ),
    };
    format!("  {mark} {:9} {:12} {state}", status.id, status.label)
}

async fn connect(name: &str, launch: &Launch, chosen: Vec<String>, on: bool) -> Outcome {
    let paint = Paint::stdout();
    let picked = if chosen.is_empty() {
        let wanted =
            |status: &AgentStatus| status.installed && (on || status.connection != Connection::Off);
        let all = agents::statuses(name, launch).await;
        let picked: Vec<_> = all
            .iter()
            .filter(|status| wanted(status))
            .map(|status| agents::agent(status.id))
            .collect::<Result<_, _>>()?;
        if picked.is_empty() {
            println!(
                "{}",
                if on {
                    "none of Claude Code, Codex or OpenCode is installed here"
                } else {
                    "no agent has it connected"
                }
            );
            return Ok(if on { 1 } else { 0 });
        }
        picked
    } else {
        chosen
            .iter()
            .map(|id| agents::agent(id))
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut failed = false;
    for agent in picked {
        match agents::connect(agent, name, launch, on).await {
            Ok(status) => println!("{}", row(paint, &status)),
            Err(message) => {
                failed = true;
                println!(
                    "  {} {:9} {:12} {message}",
                    paint.good(false),
                    agent.id(),
                    agent.label()
                );
            }
        }
    }
    if on && !failed {
        println!("\nrestart a running agent session to pick it up");
    }
    Ok(if failed { 1 } else { 0 })
}
