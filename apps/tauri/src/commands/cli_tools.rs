//! Tauri IPC commands for external AI CLI tool management.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CliToolInfo {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub install_hint: String,
}

const TOOLS: &[(&str, &str, &str)] = &[
    (
        "claude",
        "Claude Code",
        "npm install -g @anthropic-ai/claude-code",
    ),
    ("codex", "Codex CLI", "npm install -g @openai/codex"),
    ("gemini", "Gemini CLI", "npm install -g @google/gemini-cli"),
    ("kiro", "Kiro CLI", "npm install -g @aws/kiro-cli"),
];

/// List external CLI tools with install status.
#[tauri::command]
pub async fn list_cli_tools() -> Vec<CliToolInfo> {
    let mut result = Vec::new();
    for (bin, name, hint) in TOOLS {
        let installed = tokio::process::Command::new("which")
            .arg(bin)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
        result.push(CliToolInfo {
            id: bin.to_string(),
            name: name.to_string(),
            installed,
            install_hint: hint.to_string(),
        });
    }
    result
}

/// Run an external CLI tool with a prompt and return the output.
#[tauri::command]
pub async fn run_cli_tool(tool: String, prompt: String) -> Result<String, String> {
    let bin = match tool.as_str() {
        "claude" | "codex" | "gemini" | "kiro" => tool.as_str(),
        _ => return Err(format!("지원하지 않는 도구: {tool}")),
    };

    let output = tokio::process::Command::new(bin)
        .args(["-m", &prompt])
        .output()
        .await
        .map_err(|e| format!("{bin} 실행 실패: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{bin} 오류: {stderr}"))
    }
}
