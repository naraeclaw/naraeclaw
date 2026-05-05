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
    ("claude", "Claude Code", "npm install -g @anthropic-ai/claude-code"),
    ("codex",  "Codex CLI",   "npm install -g @openai/codex"),
    ("gemini", "Gemini CLI",  "npm install -g @google/gemini-cli"),
    ("kiro",   "Kiro CLI",    "npm install -g @aws/kiro-cli"),
];

// Tauri 앱은 GUI 런처로 시작되어 사용자 쉘 PATH를 상속하지 않는다.
// 로그인 쉘로 명령을 실행해야 nvm/fnm/homebrew 등 경로가 포함된다.
fn login_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

async fn shell_which(bin: &str) -> bool {
    let shell = login_shell();
    tokio::process::Command::new(&shell)
        .args(["-lc", &format!("which {bin}")])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// List external CLI tools with install status.
#[tauri::command]
pub async fn list_cli_tools() -> Vec<CliToolInfo> {
    let mut result = Vec::new();
    for (bin, name, hint) in TOOLS {
        let installed = shell_which(bin).await;
        result.push(CliToolInfo {
            id: bin.to_string(),
            name: name.to_string(),
            installed,
            install_hint: hint.to_string(),
        });
    }
    result
}

/// Install an external CLI tool via npm (로그인 쉘 경유로 npm 경로 보장).
#[tauri::command]
pub async fn install_cli_tool(tool: String) -> Result<String, String> {
    let pkg = match tool.as_str() {
        "claude" => "@anthropic-ai/claude-code",
        "codex"  => "@openai/codex",
        "gemini" => "@google/gemini-cli",
        "kiro"   => "@aws/kiro-cli",
        _        => return Err(format!("지원하지 않는 도구: {tool}")),
    };

    let shell = login_shell();
    let output = tokio::process::Command::new(&shell)
        .args(["-lc", &format!("npm install -g {pkg}")])
        .output()
        .await
        .map_err(|e| format!("npm 실행 실패: {e}"))?;

    if output.status.success() {
        Ok(format!("{pkg} 설치 완료"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("설치 실패: {stderr}"))
    }
}

/// Run an external CLI tool with a prompt and return the output.
#[tauri::command]
pub async fn run_cli_tool(tool: String, prompt: String) -> Result<String, String> {
    let bin = match tool.as_str() {
        "claude" | "codex" | "gemini" | "kiro" => tool.as_str(),
        _ => return Err(format!("지원하지 않는 도구: {tool}")),
    };

    let shell = login_shell();
    // prompt의 작은따옴표를 이스케이프하여 쉘 인젝션 방지
    let safe_prompt = prompt.replace('\'', "'\\''");
    let output = tokio::process::Command::new(&shell)
        .args(["-lc", &format!("{bin} -m '{safe_prompt}'")])
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
