//! `mark_skill_candidate` tool (ADR-005 M3b, D2 tool-signal path).
//!
//! A near no-op tool the agent calls to explicitly flag the current task as
//! worth turning into a reusable skill. The call is recorded in the turn's
//! execution trace; the agent loop detects the call and feeds
//! [`UserSignal::Tool`] into the skill-evolution trigger, which bypasses the
//! value threshold. The tool itself only validates input and acknowledges —
//! it deliberately holds no state and performs no I/O.

use async_trait::async_trait;
use naraeclaw_api::tool::{Tool, ToolResult};
use serde_json::{Value, json};

/// Stateless explicit-signal tool. See module docs.
#[derive(Default)]
pub struct MarkSkillCandidateTool;

impl MarkSkillCandidateTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for MarkSkillCandidateTool {
    fn name(&self) -> &str {
        "mark_skill_candidate"
    }

    fn description(&self) -> &str {
        "현재 작업 흐름을 재사용 가능한 스킬로 만들 가치가 있다고 명시적으로 표시한다. \
         다단계로 성공했고 다음에도 자동으로 끌어다 쓰고 싶은 절차일 때 호출한다."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "스킬 후보의 짧은 제목. 예: '스테이징 배포'"
                },
                "reason": {
                    "type": "string",
                    "description": "왜 스킬화할 가치가 있는지 (선택)"
                }
            },
            "required": ["title"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let title = args["title"].as_str().map(str::trim).unwrap_or("");
        if title.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'title' 파라미터가 필요합니다.".into()),
            });
        }
        let reason = args["reason"].as_str().unwrap_or("").trim();
        tracing::info!(title, reason, "mark_skill_candidate: 스킬 후보로 표시됨");
        Ok(ToolResult {
            success: true,
            output: format!(
                "'{title}' 작업을 스킬 후보로 표시했습니다. 이번 턴이 자동 스킬 생성 대상으로 우선 처리됩니다."
            ),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_and_required_param() {
        let tool = MarkSkillCandidateTool::new();
        assert_eq!(tool.name(), "mark_skill_candidate");
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"][0], "title");
    }

    #[tokio::test]
    async fn execute_succeeds_with_title() {
        let tool = MarkSkillCandidateTool::new();
        let res = tool
            .execute(json!({"title": "스테이징 배포", "reason": "자주 씀"}))
            .await
            .unwrap();
        assert!(res.success);
        assert!(res.output.contains("스테이징 배포"));
    }

    #[tokio::test]
    async fn execute_fails_without_title() {
        let tool = MarkSkillCandidateTool::new();
        let res = tool.execute(json!({"reason": "x"})).await.unwrap();
        assert!(!res.success);
        assert!(res.error.is_some());
    }
}
