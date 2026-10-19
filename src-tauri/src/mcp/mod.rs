pub mod agents;
pub mod tools;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::ai::Provider;
use tools::{Context, TOOLS};

const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

const INSTRUCTIONS: &str = "Seam is the user's desktop git client. Use it to show the user a change and to leave review comments they read beside the diff.

- Read a change with get_diff before leave_comments, so each comment names a file in the diff and a line in its new version.
- leave_comments replaces the last review Seam kept for the repository and opens it in Seam; put every comment in one call.
- open_diff shows a change without commenting on it.
- get_review reads the review Seam last kept, so you can act on its findings.
- With no range, commit, staged, uncommitted or pull_request, a tool works on the current branch against main, or on the uncommitted changes when on main.";

pub struct Session {
    context: Context,
    client: Option<Provider>,
}

impl Session {
    pub fn new(context: Context) -> Self {
        Session {
            context,
            client: None,
        }
    }

    /// One message in, at most one reply out: notifications and the client's own replies get none.
    pub async fn handle(&mut self, message: Value) -> Option<Value> {
        let id = message.get("id").cloned().filter(|id| !id.is_null());
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return id.map(|id| error(id, INVALID_REQUEST, "A request needs a method."));
        };
        let id = id?;
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_list() })),
            "tools/call" => self.call(params).await,
            _ => Err((METHOD_NOT_FOUND, format!("No method called {method}."))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error(id, code, &message),
        })
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let client = params["clientInfo"]["name"].as_str().unwrap_or_default();
        self.client = client_provider(client);
        let requested = params["protocolVersion"].as_str().unwrap_or_default();
        let version = PROTOCOL_VERSIONS
            .into_iter()
            .find(|version| *version == requested)
            .unwrap_or(PROTOCOL_VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "seam", "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        })
    }

    async fn call(&self, params: Value) -> Result<Value, (i64, String)> {
        let name = params["name"].as_str().unwrap_or_default();
        if !TOOLS.iter().any(|tool| tool.name == name) {
            return Err((INVALID_PARAMS, format!("No tool called {name}.")));
        }
        let input = params.get("arguments").cloned().unwrap_or(Value::Null);
        Ok(
            match tools::call(&self.context, self.client, name, input).await {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                Err(failure) => json!({
                    "content": [{ "type": "text", "text": failure.0 }],
                    "isError": true,
                }),
            },
        )
    }
}

fn client_provider(name: &str) -> Option<Provider> {
    let name = name.to_lowercase();
    if name.contains("claude") {
        Some(Provider::Claude)
    } else if name.contains("codex") {
        Some(Provider::Codex)
    } else if name.contains("opencode") {
        Some(Provider::OpenCode)
    } else {
        None
    }
}

fn tool_list() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tools::input_schema(tool.name),
                "annotations": {
                    "readOnlyHint": tool.read_only,
                    "destructiveHint": false,
                    "openWorldHint": false,
                },
            })
        })
        .collect()
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Newline-delimited JSON-RPC on stdin and stdout, as MCP's stdio transport has it. Nothing
/// else may write to stdout while this runs.
pub async fn serve(context: Context) -> std::io::Result<()> {
    let mut session = Session::new(context);
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => session.handle(message).await,
            Err(failure) => Some(error(Value::Null, PARSE_ERROR, &failure.to_string())),
        };
        if let Some(reply) = reply {
            let mut text = reply.to_string();
            text.push('\n');
            stdout.write_all(text.as_bytes()).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}
