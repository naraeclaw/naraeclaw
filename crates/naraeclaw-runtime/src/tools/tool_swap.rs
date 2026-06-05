//! ToolSwapTool — runtime tool replacement/disabling without restart.
//!
//! Lives in naraeclaw-runtime (not naraeclaw-tools) because it directly
//! manipulates the SharedOverrideRegistry that also lives here.

use async_trait::async_trait;
use naraeclaw_api::tool::{Tool, ToolResult};
use serde_json::{Value, json};

use super::override_registry::{OverrideKind, SharedOverrideRegistry, ToolOverride};

pub struct ToolSwapTool {
    registry: SharedOverrideRegistry,
    /// Autonomy gate: mutating actions (disable/http_delegate/restore) are
    /// rejected under `ReadOnly`. Defaults to `Supervised` for back-compat.
    autonomy: crate::security::AutonomyLevel,
}

impl ToolSwapTool {
    pub fn new(registry: SharedOverrideRegistry) -> Self {
        Self {
            registry,
            autonomy: crate::security::AutonomyLevel::Supervised,
        }
    }

    pub fn with_autonomy(mut self, autonomy: crate::security::AutonomyLevel) -> Self {
        self.autonomy = autonomy;
        self
    }
}

#[async_trait]
impl Tool for ToolSwapTool {
    fn name(&self) -> &str {
        "tool_swap"
    }

    fn description(&self) -> &str {
        "런타임에 도구를 교체하거나 비활성화한다. 재시작 없이 즉시 적용되며 \
         재시작 후에도 유지된다. \
         - disable: 도구를 비활성화하여 LLM이 대안을 찾도록 유도 \
         - http_delegate: 도구 호출을 외부 HTTP 엔드포인트로 위임 \
         - restore: 오버라이드를 제거하고 기본 구현으로 복원 \
         - list: 현재 활성 오버라이드 목록 조회"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["disable", "http_delegate", "restore", "list"],
                    "description": "수행할 작업"
                },
                "tool_name": {
                    "type": "string",
                    "description": "대상 도구 이름 (list 제외 필수)"
                },
                "reason": {
                    "type": "string",
                    "description": "오버라이드 이유"
                },
                "url": {
                    "type": "string",
                    "description": "HTTP 위임 URL (http_delegate 전용)"
                },
                "headers": {
                    "type": "object",
                    "description": "HTTP 요청 헤더 (http_delegate 선택)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let action = match args["action"].as_str() {
            Some(a) => a,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("'action' 파라미터가 필요합니다.".into()),
                });
            }
        };
        // Security gate: mutating actions change tool wiring at runtime and are
        // forbidden under ReadOnly. `list` (inspection) is always allowed.
        let mutating = matches!(action, "disable" | "http_delegate" | "restore");
        if mutating && self.autonomy == crate::security::AutonomyLevel::ReadOnly {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "ReadOnly 자율성에서는 tool_swap의 도구 교체/비활성화가 허용되지 않습니다."
                        .into(),
                ),
            });
        }
        if mutating {
            // Audit: runtime override changes are security-relevant.
            tracing::info!(
                action,
                tool = args["tool_name"].as_str().unwrap_or("-"),
                "tool_swap: runtime override change requested"
            );
        }
        match action {
            "disable" => self.do_disable(&args),
            "http_delegate" => self.do_http_delegate(&args),
            "restore" => self.do_restore(&args),
            "list" => self.do_list(),
            other => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("알 수 없는 action: '{other}'")),
            }),
        }
    }
}

impl ToolSwapTool {
    fn require_tool_name(args: &Value) -> Result<String, ToolResult> {
        match args["tool_name"].as_str().filter(|s| !s.trim().is_empty()) {
            Some(n) => Ok(n.trim().to_string()),
            None => Err(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'tool_name' 파라미터가 필요합니다.".into()),
            }),
        }
    }

    fn do_disable(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let tool_name = match Self::require_tool_name(args) {
            Ok(n) => n,
            Err(r) => return Ok(r),
        };
        let reason = args["reason"]
            .as_str()
            .unwrap_or("에이전트 요청으로 비활성화됨")
            .to_string();
        self.registry.lock().unwrap().set(ToolOverride {
            tool_name: tool_name.clone(),
            kind: OverrideKind::Disabled {
                reason: reason.clone(),
            },
            reason: Some(reason.clone()),
        });
        Ok(ToolResult {
            success: true,
            output: format!(
                "✅ 도구 '{tool_name}' 비활성화됨.\n\
                 이유: {reason}\n\
                 이제 '{tool_name}' 호출 시 에러를 반환하므로 대안을 사용하세요.\n\
                 복구: tool_swap action=restore tool_name={tool_name}"
            ),
            error: None,
        })
    }

    fn do_http_delegate(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let tool_name = match Self::require_tool_name(args) {
            Ok(n) => n,
            Err(r) => return Ok(r),
        };
        let url = match args["url"].as_str().filter(|s| !s.is_empty()) {
            Some(u) => u.to_string(),
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("http_delegate에는 'url' 파라미터가 필요합니다.".into()),
                });
            }
        };
        let headers = args["headers"]
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        self.registry.lock().unwrap().set(ToolOverride {
            tool_name: tool_name.clone(),
            kind: OverrideKind::HttpDelegate {
                url: url.clone(),
                headers,
                timeout_secs: 30,
            },
            reason: args["reason"].as_str().map(str::to_string),
        });
        Ok(ToolResult {
            success: true,
            output: format!("✅ 도구 '{tool_name}' → HTTP 위임 등록됨.\n위임 URL: {url}"),
            error: None,
        })
    }

    fn do_restore(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let tool_name = match Self::require_tool_name(args) {
            Ok(n) => n,
            Err(r) => return Ok(r),
        };
        if self.registry.lock().unwrap().remove(&tool_name) {
            Ok(ToolResult {
                success: true,
                output: format!("✅ '{tool_name}' 오버라이드 제거됨. 기본 구현으로 복원됩니다."),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("'{tool_name}'에 대한 오버라이드가 없습니다.")),
            })
        }
    }

    fn do_list(&self) -> anyhow::Result<ToolResult> {
        let reg = self.registry.lock().unwrap();
        let overrides = reg.list();
        if overrides.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: "현재 활성 오버라이드 없음. 모든 도구가 기본 구현으로 동작 중.".into(),
                error: None,
            });
        }
        let mut lines = vec![format!("## 활성 오버라이드 ({} 개)\n", overrides.len())];
        for ov in overrides {
            let kind_str = match &ov.kind {
                OverrideKind::Disabled { reason } => format!("🚫 비활성화 — {reason}"),
                OverrideKind::HttpDelegate { url, .. } => format!("🔀 HTTP 위임 → {url}"),
            };
            let note = ov
                .reason
                .as_deref()
                .map(|r| format!(" _(이유: {r})_"))
                .unwrap_or_default();
            lines.push(format!("- **{}**: {}{}", ov.tool_name, kind_str, note));
        }
        Ok(ToolResult {
            success: true,
            output: lines.join("\n"),
            error: None,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::override_registry::ToolOverrideRegistry;
    use std::sync::{Arc, Mutex};

    fn make_tool() -> ToolSwapTool {
        ToolSwapTool::new(Arc::new(Mutex::new(ToolOverrideRegistry::default())))
    }

    #[tokio::test]
    async fn disable_and_list() {
        let t = make_tool();
        t.execute(json!({"action":"disable","tool_name":"jira","reason":"test"}))
            .await
            .unwrap();
        let r = t.execute(json!({"action":"list"})).await.unwrap();
        assert!(r.success);
        assert!(r.output.contains("jira"));
    }

    #[tokio::test]
    async fn restore_removes_override() {
        let t = make_tool();
        t.execute(json!({"action":"disable","tool_name":"jira","reason":"x"}))
            .await
            .unwrap();
        let r = t
            .execute(json!({"action":"restore","tool_name":"jira"}))
            .await
            .unwrap();
        assert!(r.success);
        let list = t.execute(json!({"action":"list"})).await.unwrap();
        assert!(list.output.contains("없음"));
    }

    #[tokio::test]
    async fn missing_action_errors() {
        let r = make_tool().execute(json!({})).await.unwrap();
        assert!(!r.success);
    }

    #[tokio::test]
    async fn http_delegate_requires_url() {
        let r = make_tool()
            .execute(json!({"action":"http_delegate","tool_name":"jira"}))
            .await
            .unwrap();
        assert!(!r.success);
    }

    #[tokio::test]
    async fn readonly_rejects_mutating_but_allows_list() {
        let t = ToolSwapTool::new(Arc::new(Mutex::new(ToolOverrideRegistry::default())))
            .with_autonomy(crate::security::AutonomyLevel::ReadOnly);
        // Mutating action (disable) is rejected under ReadOnly autonomy.
        let disabled = t
            .execute(json!({"action":"disable","tool_name":"jira"}))
            .await
            .unwrap();
        assert!(!disabled.success);
        // Inspection (list) remains allowed.
        let list = t.execute(json!({"action":"list"})).await.unwrap();
        assert!(list.success);
    }
}
