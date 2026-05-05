//! Tauri IPC commands for computer use (screen, mouse, keyboard).
//!
//! ## Security model
//!
//! All destructive actions (screenshot, mouse, keyboard, app launch) require a
//! valid *one-shot approval nonce* issued by `request_computer_use_approval`.
//!
//! The frontend **must** display a visible confirmation dialog to the user and
//! call `request_computer_use_approval` only after the user explicitly accepts.
//! The nonce is then single-use and expires after 30 seconds.
//!
//! Compared to the old `approved: bool` parameter (which the renderer could
//! trivially set to `true`), this design requires the renderer to make a
//! separate round-trip to Rust state and consume the nonce within the TTL.

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Monotonic counter for nonce uniqueness within a session.
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_nonce() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let count = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{ts:016x}{count:08x}")
}

/// Issue a one-shot approval nonce for a computer-use action.
///
/// The frontend **must** show the user a confirmation dialog BEFORE calling
/// this command. Only call after the user explicitly clicks "허용". The
/// returned nonce must be passed to the action command within 30 seconds and
/// is single-use.
#[tauri::command]
pub async fn request_computer_use_approval(
    state: tauri::State<'_, crate::state::SharedState>,
    action_description: String,
) -> Result<String, String> {
    let nonce = generate_nonce();
    tracing::info!("computer-use approval issued for action: {action_description} nonce={nonce}");
    let s = state.read().await;
    s.approvals.insert(nonce.clone()).await;
    Ok(nonce)
}

/// Consume and validate an approval nonce. Used internally and by `resources`.
pub async fn consume_approval_pub(
    state: &tauri::State<'_, crate::state::SharedState>,
    nonce: &str,
) -> Result<(), String> {
    consume_approval(state, nonce).await
}

async fn consume_approval(
    state: &tauri::State<'_, crate::state::SharedState>,
    nonce: &str,
) -> Result<(), String> {
    let s = state.read().await;
    if s.approvals.consume(nonce).await {
        Ok(())
    } else {
        Err("유효하지 않거나 만료된 승인 토큰입니다".into())
    }
}

/// Screenshot result with base64-encoded PNG.
#[derive(Debug, Serialize)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub data_base64: String,
}

/// Take a screenshot of the entire screen.
/// Requires a valid one-shot approval nonce from `request_computer_use_approval`.
#[tauri::command]
pub async fn take_screenshot(
    state: tauri::State<'_, crate::state::SharedState>,
    approval_nonce: String,
) -> Result<Screenshot, String> {
    consume_approval(&state, &approval_nonce).await?;

    let tmp = std::env::temp_dir().join("naraeclaw_screenshot.png");

    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("screencapture")
            .args(["-x", "-t", "png"])
            .arg(&tmp)
            .output()
            .await
            .map_err(|e| format!("스크린샷 실패: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        let result = tokio::process::Command::new("import")
            .args(["-window", "root"])
            .arg(&tmp)
            .output()
            .await;
        if result.is_err() {
            tokio::process::Command::new("gnome-screenshot")
                .args(["-f"])
                .arg(&tmp)
                .output()
                .await
                .map_err(|e| format!("스크린샷 실패: {e}"))?;
        }
    }

    let bytes = std::fs::read(&tmp).map_err(|e| format!("스크린샷 읽기 실패: {e}"))?;
    let _ = std::fs::remove_file(&tmp);

    use base64::Engine;
    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    // Try to read dimensions from PNG header (width at bytes 16-19, height at 20-23).
    let (width, height) = if bytes.len() > 24 {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        (w, h)
    } else {
        (0, 0)
    };

    Ok(Screenshot {
        width,
        height,
        data_base64,
    })
}

/// Move the mouse to (x, y) and optionally click.
/// Requires a valid one-shot approval nonce.
#[tauri::command]
pub async fn mouse_action(
    state: tauri::State<'_, crate::state::SharedState>,
    x: i32,
    y: i32,
    click: bool,
    approval_nonce: String,
) -> Result<String, String> {
    consume_approval(&state, &approval_nonce).await?;

    #[cfg(target_os = "macos")]
    {
        let action = if click {
            format!("c:{x},{y}")
        } else {
            format!("m:{x},{y}")
        };
        tokio::process::Command::new("cliclick")
            .arg(&action)
            .output()
            .await
            .map_err(|e| {
                format!(
                    "마우스 제어 실패: {e}. cliclick이 설치되어 있나요? (brew install cliclick)"
                )
            })?;
    }

    #[cfg(target_os = "linux")]
    {
        tokio::process::Command::new("xdotool")
            .args(["mousemove", &x.to_string(), &y.to_string()])
            .output()
            .await
            .map_err(|e| format!("마우스 이동 실패: {e}"))?;
        if click {
            tokio::process::Command::new("xdotool")
                .arg("click")
                .arg("1")
                .output()
                .await
                .map_err(|e| format!("클릭 실패: {e}"))?;
        }
    }

    Ok(if click {
        format!("클릭: ({x}, {y})")
    } else {
        format!("이동: ({x}, {y})")
    })
}

/// Type text using keyboard simulation.
/// Requires a valid one-shot approval nonce.
#[tauri::command]
pub async fn keyboard_type(
    state: tauri::State<'_, crate::state::SharedState>,
    text: String,
    approval_nonce: String,
) -> Result<String, String> {
    consume_approval(&state, &approval_nonce).await?;

    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("cliclick")
            .arg(format!("t:{text}"))
            .output()
            .await
            .map_err(|e| format!("키보드 입력 실패: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        tokio::process::Command::new("xdotool")
            .args(["type", "--", &text])
            .output()
            .await
            .map_err(|e| format!("키보드 입력 실패: {e}"))?;
    }

    Ok("입력 완료".into())
}

/// Press a key combination (e.g. "cmd+c", "ctrl+v").
/// Requires a valid one-shot approval nonce.
#[tauri::command]
pub async fn keyboard_shortcut(
    state: tauri::State<'_, crate::state::SharedState>,
    keys: String,
    approval_nonce: String,
) -> Result<String, String> {
    consume_approval(&state, &approval_nonce).await?;

    #[cfg(target_os = "macos")]
    {
        // cliclick uses kp: for key press, e.g. "kp:cmd+c"
        // Convert common names.
        let cliclick_keys = keys
            .replace("ctrl", "ctrl")
            .replace("cmd", "cmd")
            .replace("alt", "alt")
            .replace("shift", "shift");
        tokio::process::Command::new("cliclick")
            .arg(format!("kp:{cliclick_keys}"))
            .output()
            .await
            .map_err(|e| format!("단축키 실패: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        let xdotool_keys = keys.replace("cmd", "super").replace("+", "+");
        tokio::process::Command::new("xdotool")
            .args(["key", &xdotool_keys])
            .output()
            .await
            .map_err(|e| format!("단축키 실패: {e}"))?;
    }

    Ok(format!("단축키 실행: {keys}"))
}

/// Get list of running applications/windows.
#[tauri::command]
pub async fn list_windows() -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let output = tokio::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of every process whose visible is true",
            ])
            .output()
            .await
            .map_err(|e| format!("앱 목록 조회 실패: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.split(", ").map(|s| s.trim().to_string()).collect())
    }

    #[cfg(target_os = "linux")]
    {
        let output = tokio::process::Command::new("wmctrl")
            .arg("-l")
            .output()
            .await
            .map_err(|e| format!("창 목록 조회 실패: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|l| l.to_string()).collect())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err("지원하지 않는 OS".into())
    }
}
