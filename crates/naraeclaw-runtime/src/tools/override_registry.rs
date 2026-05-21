//! Runtime tool-override registry.
//!
//! Allows the agent to replace or disable any built-in tool at runtime
//! without restarting the service.  Overrides are persisted to
//! `workspace_dir/tool_overrides.json` and reloaded on the next startup.
//!
//! Lookup priority (highest → lowest):
//!   1. ToolOverrideRegistry  — agent-controlled at runtime
//!   2. Static tools registry — compiled-in built-ins
//!   3. ActivatedToolSet      — dynamically loaded MCP tools

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use naraeclaw_api::tool::{Tool, ToolResult};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// ── Override kinds ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OverrideKind {
    /// Return a fixed error so the LLM knows to find another approach.
    Disabled {
        reason: String,
    },
    /// Forward the tool call as a JSON POST to an HTTP endpoint.
    /// Request body: `{"tool": "<name>", "args": {...}}`
    /// Expected response body: `{"success": bool, "output": "...", "error": "..."}`
    HttpDelegate {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_timeout_secs")]
        timeout_secs: u64,
    },
}

fn default_timeout_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOverride {
    pub tool_name: String,
    pub kind: OverrideKind,
    pub reason: Option<String>,
}

// ── Registry ──────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct ToolOverrideRegistry {
    overrides: HashMap<String, ToolOverride>,
    persist_path: Option<PathBuf>,
}

impl ToolOverrideRegistry {
    pub fn new(workspace_dir: &std::path::Path) -> Self {
        Self {
            overrides: HashMap::new(),
            persist_path: Some(workspace_dir.join("tool_overrides.json")),
        }
    }

    /// Load persisted overrides from disk. Missing file is not an error.
    pub fn load(&mut self) {
        let Some(ref path) = self.persist_path else {
            return;
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return,
        };
        match serde_json::from_str::<HashMap<String, ToolOverride>>(&text) {
            Ok(map) => {
                let count = map.len();
                self.overrides = map;
                if count > 0 {
                    info!(count, "Loaded tool overrides from disk");
                }
            }
            Err(e) => warn!("Failed to parse tool_overrides.json: {e}"),
        }
    }

    fn persist(&self) {
        let Some(ref path) = self.persist_path else {
            return;
        };
        match serde_json::to_string_pretty(&self.overrides) {
            Ok(s) => {
                if let Err(e) = std::fs::write(path, s) {
                    warn!("Failed to persist tool overrides: {e}");
                }
            }
            Err(e) => warn!("Failed to serialize tool overrides: {e}"),
        }
    }

    pub fn set(&mut self, override_entry: ToolOverride) {
        info!(
            tool = override_entry.tool_name.as_str(),
            kind = ?override_entry.kind,
            "Tool override registered"
        );
        self.overrides
            .insert(override_entry.tool_name.clone(), override_entry);
        self.persist();
    }

    pub fn remove(&mut self, tool_name: &str) -> bool {
        let removed = self.overrides.remove(tool_name).is_some();
        if removed {
            info!(tool = tool_name, "Tool override removed");
            self.persist();
        }
        removed
    }

    pub fn get(&self, tool_name: &str) -> Option<&ToolOverride> {
        self.overrides.get(tool_name)
    }

    pub fn list(&self) -> Vec<&ToolOverride> {
        self.overrides.values().collect()
    }

    /// Resolve a tool call through the override registry.
    /// Returns `Some(result)` if an override handled it, `None` to fall through.
    pub async fn resolve(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Option<ToolResult> {
        let entry = self.overrides.get(tool_name)?;
        let result = match &entry.kind {
            OverrideKind::Disabled { reason } => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "도구 '{tool_name}'이 비활성화됨: {reason}. 다른 방법을 사용하세요."
                )),
            },
            OverrideKind::HttpDelegate {
                url,
                headers,
                timeout_secs,
            } => call_http_delegate(tool_name, args, url, headers, *timeout_secs).await,
        };
        Some(result)
    }
}

pub async fn call_http_delegate(
    tool_name: &str,
    args: serde_json::Value,
    url: &str,
    headers: &HashMap<String, String>,
    timeout_secs: u64,
) -> ToolResult {
    let body = serde_json::json!({ "tool": tool_name, "args": args });

    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_default()
        .post(url)
        .json(&body);

    for (k, v) in headers {
        builder = builder.header(k.as_str(), v.as_str());
    }

    match builder.send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(v) => ToolResult {
                    success: v["success"].as_bool().unwrap_or(true),
                    output: v["output"].as_str().unwrap_or("").to_string(),
                    error: v["error"].as_str().map(str::to_string),
                },
                Err(e) => ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("HTTP delegate parse error: {e}")),
                },
            }
        }
        Ok(resp) => ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("HTTP delegate returned {}", resp.status())),
        },
        Err(e) => ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("HTTP delegate request failed: {e}")),
        },
    }
}

// ── Shared handle ─────────────────────────────────────────────────────────────

pub type SharedOverrideRegistry = Arc<Mutex<ToolOverrideRegistry>>;

pub fn new_shared_registry(workspace_dir: &std::path::Path) -> SharedOverrideRegistry {
    let mut reg = ToolOverrideRegistry::new(workspace_dir);
    reg.load();
    Arc::new(Mutex::new(reg))
}

// ── Wrapper Tool — makes an override entry callable as a Tool trait object ───

/// Wraps an HTTP-delegate override so it can be placed in the tools registry.
pub struct OverriddenTool {
    name: String,
    registry: SharedOverrideRegistry,
}

impl OverriddenTool {
    pub fn new(name: String, registry: SharedOverrideRegistry) -> Self {
        Self { name, registry }
    }
}

#[async_trait]
impl Tool for OverriddenTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Overridden tool — delegates to the registered override."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {}, "additionalProperties": true })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        // Clone the kind before releasing the lock so we don't hold MutexGuard across await.
        let kind_opt: Option<OverrideKind> = self
            .registry
            .lock()
            .unwrap()
            .get(&self.name)
            .map(|o| o.kind.clone());
        match kind_opt {
            Some(OverrideKind::Disabled { reason }) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "도구 '{}'이 비활성화됨: {}. 다른 방법을 사용하세요.",
                    self.name, reason
                )),
            }),
            Some(OverrideKind::HttpDelegate { url, headers, timeout_secs }) => {
                Ok(call_http_delegate(&self.name, args, &url, &headers, timeout_secs).await)
            }
            None => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Override for '{}' was removed", self.name)),
            }),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_reg() -> ToolOverrideRegistry {
        ToolOverrideRegistry::default()
    }

    #[test]
    fn disabled_override_added_and_retrieved() {
        let mut reg = make_reg();
        reg.set(ToolOverride {
            tool_name: "jira".into(),
            kind: OverrideKind::Disabled {
                reason: "반복 실패".into(),
            },
            reason: None,
        });
        assert!(reg.get("jira").is_some());
    }

    #[test]
    fn remove_clears_override() {
        let mut reg = make_reg();
        reg.set(ToolOverride {
            tool_name: "jira".into(),
            kind: OverrideKind::Disabled {
                reason: "test".into(),
            },
            reason: None,
        });
        assert!(reg.remove("jira"));
        assert!(reg.get("jira").is_none());
        assert!(!reg.remove("jira")); // double remove
    }

    #[tokio::test]
    async fn disabled_resolve_returns_error_result() {
        let mut reg = make_reg();
        reg.set(ToolOverride {
            tool_name: "jira".into(),
            kind: OverrideKind::Disabled {
                reason: "API 폐기됨".into(),
            },
            reason: None,
        });
        let result = reg
            .resolve("jira", serde_json::json!({}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.as_deref().unwrap().contains("비활성화됨"));
    }

    #[tokio::test]
    async fn unknown_tool_resolve_returns_none() {
        let reg = make_reg();
        assert!(reg.resolve("unknown_tool", serde_json::json!({})).await.is_none());
    }

    #[test]
    fn persist_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = ToolOverrideRegistry::new(dir.path());
        reg.set(ToolOverride {
            tool_name: "redash".into(),
            kind: OverrideKind::Disabled {
                reason: "maintenance".into(),
            },
            reason: Some("scheduled downtime".into()),
        });

        // Reload from disk
        let mut reg2 = ToolOverrideRegistry::new(dir.path());
        reg2.load();
        assert!(reg2.get("redash").is_some());
    }

    #[test]
    fn list_returns_all_overrides() {
        let mut reg = make_reg();
        reg.set(ToolOverride {
            tool_name: "jira".into(),
            kind: OverrideKind::Disabled { reason: "x".into() },
            reason: None,
        });
        reg.set(ToolOverride {
            tool_name: "redash".into(),
            kind: OverrideKind::Disabled { reason: "y".into() },
            reason: None,
        });
        assert_eq!(reg.list().len(), 2);
    }
}
