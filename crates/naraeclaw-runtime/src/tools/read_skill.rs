use async_trait::async_trait;
use naraeclaw_api::tool::{Tool, ToolResult};
use serde_json::json;
use std::path::PathBuf;

/// Compact-mode helper for loading a skill's source file on demand.
pub struct ReadSkillTool {
    workspace_dir: PathBuf,
    open_skills_enabled: bool,
    open_skills_dir: Option<String>,
    allow_scripts: bool,
    use_byori_knowledge: bool,
}

impl ReadSkillTool {
    pub fn new(
        workspace_dir: PathBuf,
        open_skills_enabled: bool,
        open_skills_dir: Option<String>,
    ) -> Self {
        Self {
            workspace_dir,
            open_skills_enabled,
            open_skills_dir,
            allow_scripts: false,
            use_byori_knowledge: false,
        }
    }

    pub fn from_config(workspace_dir: PathBuf, config: &naraeclaw_config::schema::Config) -> Self {
        Self {
            workspace_dir,
            open_skills_enabled: config.skills.open_skills_enabled,
            open_skills_dir: config.skills.open_skills_dir.clone(),
            allow_scripts: config.skills.allow_scripts,
            use_byori_knowledge: config.uses_byori_knowledge(),
        }
    }
}

#[async_trait]
impl Tool for ReadSkillTool {
    fn name(&self) -> &str {
        "read_skill"
    }

    fn description(&self) -> &str {
        "Read the full source file for an available skill by name. Use this in compact skills mode when you need the complete skill instructions without remembering file paths."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The skill name exactly as listed in <available_skills>."
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let requested = args
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' parameter"))?;

        let skills = crate::skills::load_skills_with_runtime_settings(
            &self.workspace_dir,
            self.open_skills_enabled,
            self.open_skills_dir.as_deref(),
            self.allow_scripts,
            self.use_byori_knowledge,
        );

        let Some(skill) = skills
            .iter()
            .find(|skill| skill.name.eq_ignore_ascii_case(requested))
        else {
            let mut names: Vec<&str> = skills.iter().map(|skill| skill.name.as_str()).collect();
            names.sort_unstable();
            let available = if names.is_empty() {
                "none".to_string()
            } else {
                names.join(", ")
            };

            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Unknown skill '{requested}'. Available skills: {available}"
                )),
            });
        };

        if let Some(source) = crate::skills::bundled_skill_source(&skill.name)
            && skill.location.is_none()
        {
            return Ok(ToolResult {
                success: true,
                output: source.to_string(),
                error: None,
            });
        }

        let Some(location) = skill.location.as_ref() else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Skill '{}' has no readable source location.",
                    skill.name
                )),
            });
        };

        match tokio::fs::read_to_string(location).await {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
            }),
            Err(err) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Failed to read skill '{}' from {}: {err}",
                    skill.name,
                    location.display()
                )),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_tool(tmp: &TempDir) -> ReadSkillTool {
        ReadSkillTool::new(tmp.path().join("workspace"), false, None)
    }

    fn make_byori_tool(tmp: &TempDir, enabled: bool) -> ReadSkillTool {
        let workspace = tmp.path().join("workspace");
        let mut config = naraeclaw_config::schema::Config::default();
        config.workspace_dir = workspace.clone();
        config.knowledge.enabled = enabled;
        ReadSkillTool::from_config(workspace, &config)
    }

    #[tokio::test]
    async fn reads_markdown_skill_by_name() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("workspace/skills/weather");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Weather\n\nUse this skill for forecast lookups.\n",
        )
        .unwrap();

        let result = make_tool(&tmp)
            .execute(json!({ "name": "weather" }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("# Weather"));
        assert!(result.output.contains("forecast lookups"));
    }

    #[tokio::test]
    async fn reads_toml_skill_manifest_by_name() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("workspace/skills/deploy");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.toml"),
            r#"[skill]
name = "deploy"
description = "Ship safely"
"#,
        )
        .unwrap();

        let result = make_tool(&tmp)
            .execute(json!({ "name": "deploy" }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("[skill]"));
        assert!(result.output.contains("Ship safely"));
    }

    #[tokio::test]
    async fn unknown_skill_lists_available_names() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("workspace/skills/weather");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Weather\n").unwrap();

        let result = make_tool(&tmp)
            .execute(json!({ "name": "calendar" }))
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some("Unknown skill 'calendar'. Available skills: weather")
        );
    }

    #[tokio::test]
    async fn reads_bundled_byori_skill_when_workspace_has_no_copy() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().join("workspace");

        let result = make_byori_tool(&tmp, true)
            .execute(json!({ "name": "byoridb-memory" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.output, crate::skills::BYORIDB_MEMORY_SKILL_SOURCE);
        assert!(
            !workspace.exists(),
            "reading a bundled skill must not write it"
        );
    }

    #[tokio::test]
    async fn disabled_byori_knowledge_does_not_expose_bundled_skill() {
        let tmp = TempDir::new().unwrap();

        let result = make_byori_tool(&tmp, false)
            .execute(json!({ "name": "byoridb-memory" }))
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some("Unknown skill 'byoridb-memory'. Available skills: none")
        );
    }

    #[tokio::test]
    async fn workspace_byori_skill_overrides_bundled_source() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("workspace/skills/byoridb-memory");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let custom = "# Workspace Byori\n\nUse this local workflow.\n";
        std::fs::write(skill_dir.join("SKILL.md"), custom).unwrap();

        let result = make_byori_tool(&tmp, true)
            .execute(json!({ "name": "byoridb-memory" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.output, custom);
    }

    #[test]
    fn default_non_interactive_policy_allows_read_skill() {
        let config = naraeclaw_config::schema::AutonomyConfig::default();
        let manager = crate::approval::ApprovalManager::for_non_interactive(&config);

        assert!(!manager.needs_approval("read_skill"));
    }
}
