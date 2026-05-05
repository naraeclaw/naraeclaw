//! Tauri IPC commands for file drop and clipboard integration.

use std::path::{Path, PathBuf};

/// Validate that a renderer-supplied path is safe to read.
///
/// The WebView can call IPC commands with arbitrary path strings even after a
/// real OS drag-drop event fires, so we enforce path restrictions server-side:
/// - Path must be under the user home directory or /tmp.
/// - Path must NOT be inside sensitive subdirectories (.ssh, .naraeclaw, etc.).
fn validate_drop_path(path: &Path) -> Result<(), String> {
    // canonicalize resolves symlinks; fails if the file does not exist.
    let canonical = path
        .canonicalize()
        .map_err(|_| "파일 경로를 확인할 수 없습니다".to_string())?;

    let home_str = std::env::var("HOME").unwrap_or_default();
    if home_str.is_empty() {
        return Err("홈 디렉토리를 확인할 수 없습니다".into());
    }
    let home = PathBuf::from(&home_str);

    // Directories that must never be readable via drag-drop.
    const BLOCKED: &[&str] = &[
        ".ssh",
        ".gnupg",
        ".aws",
        ".kube",
        ".naraeclaw",
        ".config/gcloud",
        ".config/op",
        ".azure",
        "Library/Keychains",
        "Library/Application Support/com.apple.TCC",
    ];
    for blocked in BLOCKED {
        if canonical.starts_with(home.join(blocked)) {
            return Err("보안상 이 경로에서 파일을 읽을 수 없습니다".into());
        }
    }

    // File must be under home or /tmp.
    if !canonical.starts_with(&home) && !canonical.starts_with("/tmp") {
        return Err("허용되지 않은 경로입니다".into());
    }

    Ok(())
}

/// Read a dropped file and send its content to the agent via gateway webhook.
#[tauri::command]
pub async fn handle_file_drop(
    state: tauri::State<'_, crate::state::SharedState>,
    path: String,
) -> Result<String, String> {
    let file_path = PathBuf::from(&path);

    validate_drop_path(&file_path)?;

    let file_name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    let metadata = std::fs::metadata(&file_path).map_err(|e| format!("파일 읽기 실패: {e}"))?;
    if metadata.len() > 10 * 1024 * 1024 {
        return Err("파일이 너무 큽니다 (최대 10MB)".into());
    }

    let content = std::fs::read_to_string(&file_path)
        .unwrap_or_else(|_| format!("[바이너리 파일: {file_name}]"));

    let message = format!("파일 '{file_name}'의 내용을 분석해줘:\n\n```\n{content}\n```");

    let (url, token) = {
        let s = state.read().await;
        (s.gateway_url.clone(), s.token.clone())
    };

    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    client
        .send_webhook_message(&message)
        .await
        .map_err(|e| format!("전송 실패: {e}"))?;

    Ok(format!("{file_name} 전송 완료"))
}

/// Read clipboard text and send to agent.
#[tauri::command]
pub async fn send_clipboard(
    state: tauri::State<'_, crate::state::SharedState>,
    text: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("클립보드가 비어있습니다".into());
    }

    let message = format!("클립보드 내용을 분석해줘:\n\n{text}");

    let (url, token) = {
        let s = state.read().await;
        (s.gateway_url.clone(), s.token.clone())
    };

    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    client
        .send_webhook_message(&message)
        .await
        .map_err(|e| format!("전송 실패: {e}"))?;

    Ok("클립보드 내용 전송 완료".into())
}
