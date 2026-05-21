//! ReflectTool — explicit post-task reflection and lesson recording.
//!
//! The agent calls this after completing (or failing) a task to record
//! what was learned. Lessons are appended to the same `lessons.jsonl`
//! file that ReflectionHook reads for automatic injection.

use async_trait::async_trait;
use chrono::Utc;
use naraeclaw_api::tool::{Tool, ToolResult};
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

pub struct ReflectTool {
    lessons_path: PathBuf,
}

impl ReflectTool {
    pub fn new(workspace_dir: &std::path::Path) -> Self {
        Self {
            lessons_path: workspace_dir.join("lessons.jsonl"),
        }
    }
}

#[async_trait]
impl Tool for ReflectTool {
    fn name(&self) -> &str {
        "reflect"
    }

    fn description(&self) -> &str {
        "명시적으로 교훈을 기록한다. 작업을 완료하거나 실수한 직후, 또는 중요한 발견이 있을 때 호출한다. \
         기록된 교훈은 이후 모든 세션의 시스템 프롬프트 앞에 자동으로 주입되어 \
         같은 실수를 반복하지 않도록 한다."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "lesson": {
                    "type": "string",
                    "description": "기억할 교훈. 구체적으로 작성한다. \
                                   예: 'jira 도구는 supervised 모드에서 auto_approve 없이 거부됨 — config.toml의 auto_approve에 추가 필요'"
                },
                "tool": {
                    "type": "string",
                    "description": "관련 도구 이름 (선택)"
                },
                "context": {
                    "type": "string",
                    "description": "어떤 상황에서 이 교훈을 얻었는지 (선택)"
                }
            },
            "required": ["lesson"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let lesson = match args["lesson"].as_str().filter(|s| !s.trim().is_empty()) {
            Some(l) => l.trim().to_string(),
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("'lesson' 파라미터가 필요합니다.".into()),
                });
            }
        };

        let tool = args["tool"].as_str().map(str::to_string);
        let context = args["context"].as_str().map(str::to_string);

        let full_lesson = if let Some(ref ctx) = context {
            format!("{lesson} (맥락: {ctx})")
        } else {
            lesson.clone()
        };

        let entry = serde_json::json!({
            "kind": "explicit",
            "tool": tool,
            "error": null,
            "lesson": full_lesson,
            "ts": Utc::now().to_rfc3339(),
        });

        let line = serde_json::to_string(&entry).unwrap_or_default();
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.lessons_path)
            .await
        {
            Ok(mut f) => {
                f.write_all(format!("{line}\n").as_bytes()).await?;
            }
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("교훈 저장 실패: {e}")),
                });
            }
        }

        Ok(ToolResult {
            success: true,
            output: format!("✅ 교훈 기록됨: {lesson}"),
            error: None,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(dir: &std::path::Path) -> ReflectTool {
        ReflectTool::new(dir)
    }

    #[test]
    fn name_is_reflect() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(tool(dir.path()).name(), "reflect");
    }

    #[test]
    fn schema_requires_lesson() {
        let dir = tempfile::tempdir().unwrap();
        let schema = tool(dir.path()).parameters_schema();
        let req = schema["required"].as_array().unwrap();
        assert!(req.contains(&Value::String("lesson".into())));
    }

    #[tokio::test]
    async fn records_lesson_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let t = tool(dir.path());
        let result = t
            .execute(json!({"lesson": "항상 설정 먼저 확인"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("항상 설정 먼저 확인"));

        let content = tokio::fs::read_to_string(dir.path().join("lessons.jsonl"))
            .await
            .unwrap();
        assert!(content.contains("항상 설정 먼저 확인"));
        assert!(content.contains("explicit"));
    }

    #[tokio::test]
    async fn includes_context_in_lesson() {
        let dir = tempfile::tempdir().unwrap();
        let t = tool(dir.path());
        t.execute(json!({
            "lesson": "timeout 발생",
            "tool": "redash",
            "context": "대용량 쿼리 실행 중"
        }))
        .await
        .unwrap();

        let content = tokio::fs::read_to_string(dir.path().join("lessons.jsonl"))
            .await
            .unwrap();
        assert!(content.contains("대용량 쿼리 실행 중"));
    }

    #[tokio::test]
    async fn missing_lesson_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = tool(dir.path()).execute(json!({})).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn empty_lesson_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = tool(dir.path())
            .execute(json!({"lesson": "   "}))
            .await
            .unwrap();
        assert!(!result.success);
    }
}
