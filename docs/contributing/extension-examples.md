# Extension Examples

NaraeClaw's architecture is trait-driven and modular.
To add a new provider, channel, or tool, implement the corresponding trait and register it in the factory module.

This page contains minimal, working examples for each core extension point.
For step-by-step integration checklists, see [change-playbooks.md](./change-playbooks.md).

> **Source of truth**: public trait definitions live in `crates/naraeclaw-api/src/`.
> If an example here conflicts with the trait file, the trait file wins.

---

## Tool (`crates/naraeclaw-api/src/tool.rs`)

Tools are the agent's hands — they let it interact with the world.

**Required methods**: `name()`, `description()`, `parameters_schema()`, `execute()`.
The `spec()` method has a default implementation that composes the others.

Place the implementation in `crates/naraeclaw-tools/src/` and register runtime wiring in
`crates/naraeclaw-runtime/src/tools/mod.rs` via `default_tools()`.

```rust
// In your crate: use naraeclaw_api::tool::{Tool, ToolResult};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

/// A tool that fetches a URL and returns the status code.
pub struct HttpGetTool;

#[async_trait]
impl Tool for HttpGetTool {
    fn name(&self) -> &str {
        "http_get"
    }

    fn description(&self) -> &str {
        "Fetch a URL and return the HTTP status code and content length"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;

        match reqwest::get(url).await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let len = resp.content_length().unwrap_or(0);
                Ok(ToolResult {
                    success: status < 400,
                    output: format!("HTTP {status} — {len} bytes"),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Request failed: {e}")),
            }),
        }
    }
}
```

---

## Channel (`crates/naraeclaw-api/src/channel.rs`)

Channels let NaraeClaw communicate through any messaging platform.

**Required methods**: `name()`, `send(&SendMessage)`, `listen()`.
Default implementations exist for `health_check()`, `start_typing()`, `stop_typing()`,
draft methods (`send_draft`, `update_draft`, `finalize_draft`, `cancel_draft`),
and reaction methods (`add_reaction`, `remove_reaction`).

Place the implementation and factory wiring in `crates/naraeclaw-channels/`, and add public
config to `ChannelsConfig` under `crates/naraeclaw-config/src/schema/`.

```rust
// In your crate: use naraeclaw_api::channel::{Channel, ChannelMessage, SendMessage};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Telegram channel via Bot API.
pub struct TelegramChannel {
    bot_token: String,
    allowed_users: Vec<String>,
    client: reqwest::Client,
}

impl TelegramChannel {
    pub fn new(bot_token: &str, allowed_users: Vec<String>) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            allowed_users,
            client: reqwest::Client::new(),
        }
    }

    fn api_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{method}", self.bot_token)
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn send(&self, message: &SendMessage) -> Result<()> {
        self.client
            .post(self.api_url("sendMessage"))
            .json(&serde_json::json!({
                "chat_id": message.recipient,
                "text": message.content,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;
        Ok(())
    }

    async fn listen(&self, tx: mpsc::Sender<ChannelMessage>) -> Result<()> {
        let mut offset: i64 = 0;

        loop {
            let resp = self
                .client
                .get(self.api_url("getUpdates"))
                .query(&[("offset", offset.to_string()), ("timeout", "30".into())])
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;

            if let Some(updates) = resp["result"].as_array() {
                for update in updates {
                    if let Some(msg) = update.get("message") {
                        let sender = msg["from"]["username"]
                            .as_str()
                            .unwrap_or("unknown")
                            .to_string();

                        if !self.allowed_users.is_empty()
                            && !self.allowed_users.contains(&sender)
                        {
                            continue;
                        }

                        let chat_id = msg["chat"]["id"].to_string();

                        let channel_msg = ChannelMessage {
                            id: msg["message_id"].to_string(),
                            sender,
                            reply_target: chat_id,
                            content: msg["text"].as_str().unwrap_or("").to_string(),
                            channel: "telegram".into(),
                            timestamp: msg["date"].as_u64().unwrap_or(0),
                            thread_ts: None,
                            interruption_scope_id: None,
                            attachments: vec![],
                            is_mention: true,
                        };

                        if tx.send(channel_msg).await.is_err() {
                            return Ok(());
                        }
                    }
                    offset = update["update_id"].as_i64().unwrap_or(offset) + 1;
                }
            }
        }
    }

    async fn health_check(&self) -> bool {
        self.client
            .get(self.api_url("getMe"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}
```

---

## Provider (`crates/naraeclaw-api/src/provider.rs`)

Providers are LLM backend adapters. Each provider connects NaraeClaw to a different model API.

**Required method**: `chat_with_system(system_prompt: Option<&str>, message: &str, model: &str, temperature: f64) -> Result<String>`.
Everything else has default implementations:
`simple_chat()` and `chat_with_history()` delegate to `chat_with_system()`;
`capabilities()` returns no native tool calling by default;
streaming methods return empty/error streams by default.

Place the implementation and factory wiring in `crates/naraeclaw-providers/src/`.

```rust
// In your crate: use naraeclaw_api::provider::Provider;

use anyhow::Result;
use async_trait::async_trait;

/// Ollama local provider.
pub struct OllamaProvider {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: Option<&str>) -> Self {
        Self {
            base_url: base_url.unwrap_or("http://localhost:11434").to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    async fn chat_with_system(
        &self,
        system_prompt: Option<&str>,
        message: &str,
        model: &str,
        temperature: f64,
    ) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        let mut body = serde_json::json!({
            "model": model,
            "prompt": message,
            "temperature": temperature,
            "stream": false,
        });

        if let Some(system) = system_prompt {
            body["system"] = serde_json::Value::String(system.to_string());
        }

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        resp["response"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No response field in Ollama reply"))
    }
}
```

---

## Legacy Memory Compatibility

The `Memory` trait in `crates/naraeclaw-api/src/memory_traits.rs` and implementations in
`crates/naraeclaw-memory/` are retained for migration and compatibility-only runtime paths.
They are active only when `[knowledge].enabled = false`; default ByoriDB mode replaces the
runtime handle with a no-op so a second durable source cannot form.

Do not add a new durable knowledge provider through this legacy factory. Extend the managed
ByoriDB MCP adapter and its `byoridb-memory` skill when a durable knowledge capability is
needed. A legacy backend change still requires an explicit compatibility use case, factory
wiring in `crates/naraeclaw-memory/`, focused tests, and migration documentation.

---

## Registration Pattern

All extension traits follow the same wiring pattern:

1. Create your implementation file in the relevant workspace crate.
2. Register it in the module's factory function (e.g., `default_tools()`, provider match arm).
3. Add any needed config keys to `crates/naraeclaw-config/src/schema/`.
4. Write focused tests for factory wiring and error paths.

See [change-playbooks.md](./change-playbooks.md) for full checklists per extension type.
