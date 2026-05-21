//! ReflectionHook — automatic lesson capture and injection.
//!
//! Two responsibilities:
//! 1. `on_after_tool_call`: when a tool fails, append a structured lesson entry
//!    to `~/.naraeclaw/lessons.jsonl` so mistakes are not repeated.
//! 2. `before_prompt_build`: inject the N most recent lessons into the system
//!    prompt so the agent is aware of past failures before it acts.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use naraeclaw_api::tool::ToolResult;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use crate::hooks::traits::{HookHandler, HookResult};

const DEFAULT_MAX_INJECT: usize = 8;
const DEFAULT_MAX_STORED: usize = 200;

// ── LessonEntry ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonEntry {
    /// "tool_error" | "explicit"
    pub kind: String,
    pub tool: Option<String>,
    pub error: Option<String>,
    pub lesson: String,
    pub ts: chrono::DateTime<Utc>,
}

// ── ReflectionHook ────────────────────────────────────────────────────────────

pub struct ReflectionHook {
    lessons_path: PathBuf,
    max_inject: usize,
}

impl ReflectionHook {
    pub fn new(workspace_dir: &std::path::Path) -> Self {
        Self {
            lessons_path: workspace_dir.join("lessons.jsonl"),
            max_inject: DEFAULT_MAX_INJECT,
        }
    }

    async fn append(&self, entry: &LessonEntry) {
        let line = match serde_json::to_string(entry) {
            Ok(s) => s,
            Err(e) => {
                warn!("reflection: failed to serialize lesson: {e}");
                return;
            }
        };
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.lessons_path)
            .await
        {
            Ok(mut f) => {
                let _ = f.write_all(format!("{line}\n").as_bytes()).await;
            }
            Err(e) => warn!("reflection: failed to write lesson: {e}"),
        }
    }

    /// Read up to `max_inject` most-recent lessons from the JSONL file.
    async fn read_recent(&self) -> Vec<LessonEntry> {
        let text = match tokio::fs::read_to_string(&self.lessons_path).await {
            Ok(t) => t,
            Err(_) => return vec![],
        };
        let mut entries: Vec<LessonEntry> = text
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        // Keep only the tail (most recent)
        if entries.len() > DEFAULT_MAX_STORED {
            entries.drain(..entries.len() - DEFAULT_MAX_STORED);
        }
        let start = entries.len().saturating_sub(self.max_inject);
        entries[start..].to_vec()
    }
}

#[async_trait]
impl HookHandler for ReflectionHook {
    fn name(&self) -> &str {
        "reflection"
    }

    // Lower priority so it fires after other void hooks.
    fn priority(&self) -> i32 {
        -10
    }

    async fn on_after_tool_call(&self, tool: &str, result: &ToolResult, _duration: Duration) {
        if result.success {
            return;
        }
        let error = result.error.clone().unwrap_or_else(|| "unknown error".into());
        let lesson = format!(
            "도구 '{tool}' 실패: {error}. \
             원인을 파악하고 같은 방식으로 재시도하지 않는다.",
        );
        debug!(tool, error = %error, "reflection: recording lesson");
        self.append(&LessonEntry {
            kind: "tool_error".into(),
            tool: Some(tool.to_string()),
            error: Some(error),
            lesson,
            ts: Utc::now(),
        })
        .await;
    }

    async fn before_prompt_build(&self, prompt: String) -> HookResult<String> {
        let lessons = self.read_recent().await;
        if lessons.is_empty() {
            return HookResult::Continue(prompt);
        }

        let mut lines = vec![
            "## 이전 실수 — 반복하지 말 것".to_string(),
            String::new(),
        ];
        for (i, l) in lessons.iter().enumerate() {
            let tool_tag = l
                .tool
                .as_deref()
                .map(|t| format!("[{t}] "))
                .unwrap_or_default();
            lines.push(format!("{}. {}{}", i + 1, tool_tag, l.lesson));
        }
        lines.push(String::new());

        let prefix = lines.join("\n");
        HookResult::Continue(prefix + &prompt)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_result(msg: &str) -> ToolResult {
        ToolResult {
            success: false,
            output: String::new(),
            error: Some(msg.into()),
        }
    }

    fn ok_result() -> ToolResult {
        ToolResult {
            success: true,
            output: "ok".into(),
            error: None,
        }
    }

    #[tokio::test]
    async fn appends_lesson_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let hook = ReflectionHook::new(dir.path());
        hook.on_after_tool_call("jira", &failed_result("Denied by user"), Duration::ZERO)
            .await;
        let lessons = hook.read_recent().await;
        assert_eq!(lessons.len(), 1);
        assert_eq!(lessons[0].tool.as_deref(), Some("jira"));
        assert_eq!(lessons[0].kind, "tool_error");
    }

    #[tokio::test]
    async fn skips_lesson_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let hook = ReflectionHook::new(dir.path());
        hook.on_after_tool_call("weather", &ok_result(), Duration::ZERO)
            .await;
        assert!(hook.read_recent().await.is_empty());
    }

    #[tokio::test]
    async fn injects_lessons_into_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let hook = ReflectionHook::new(dir.path());
        hook.on_after_tool_call("redash", &failed_result("timeout"), Duration::ZERO)
            .await;
        let result = hook
            .before_prompt_build("original system prompt".into())
            .await;
        match result {
            HookResult::Continue(s) => {
                assert!(s.contains("이전 실수"));
                assert!(s.contains("redash"));
                assert!(s.contains("original system prompt"));
            }
            HookResult::Cancel(_) => panic!("should not cancel"),
        }
    }

    #[tokio::test]
    async fn empty_lessons_does_not_modify_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let hook = ReflectionHook::new(dir.path());
        let result = hook.before_prompt_build("clean prompt".into()).await;
        match result {
            HookResult::Continue(s) => assert_eq!(s, "clean prompt"),
            HookResult::Cancel(_) => panic!("should not cancel"),
        }
    }

    #[tokio::test]
    async fn respects_max_inject_limit() {
        let dir = tempfile::tempdir().unwrap();
        let hook = ReflectionHook {
            lessons_path: dir.path().join("lessons.jsonl"),
            max_inject: 2,
        };
        for i in 0..5 {
            hook.on_after_tool_call(
                &format!("tool{i}"),
                &failed_result("err"),
                Duration::ZERO,
            )
            .await;
        }
        let lessons = hook.read_recent().await;
        assert_eq!(lessons.len(), 2);
    }

    #[tokio::test]
    async fn explicit_lesson_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let hook = ReflectionHook::new(dir.path());
        hook.append(&LessonEntry {
            kind: "explicit".into(),
            tool: None,
            error: None,
            lesson: "항상 설정 먼저 확인".into(),
            ts: Utc::now(),
        })
        .await;
        let lessons = hook.read_recent().await;
        assert_eq!(lessons[0].kind, "explicit");
        assert_eq!(lessons[0].lesson, "항상 설정 먼저 확인");
    }
}
