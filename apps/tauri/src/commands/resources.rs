//! Tauri IPC commands for computer resource access (browser, filesystem, system info).

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub hostname: String,
    pub cpu_count: usize,
    pub memory_total_mb: u64,
    pub disk_free_mb: u64,
}

/// Get system information.
#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let hostname = hostname().unwrap_or_else(|| "unknown".into());
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    // Memory: parse from sysctl (macOS) or /proc/meminfo (Linux).
    let memory_total_mb = get_total_memory_mb().await;
    let disk_free_mb = get_disk_free_mb().await;

    Ok(SystemInfo {
        os,
        arch,
        hostname,
        cpu_count,
        memory_total_mb,
        disk_free_mb,
    })
}

fn hostname() -> Option<String> {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

async fn get_total_memory_mb() -> u64 {
    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .await
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
            .map(|b| b / 1024 / 1024)
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "macos"))]
    {
        tokio::fs::read_to_string("/proc/meminfo")
            .await
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<u64>().ok())
                    })
            })
            .map(|kb| kb / 1024)
            .unwrap_or(0)
    }
}

async fn get_disk_free_mb() -> u64 {
    let output = tokio::process::Command::new("df")
        .args(["-m", "/"])
        .output()
        .await
        .ok();
    output
        .and_then(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.lines().nth(1).and_then(|line| {
                line.split_whitespace()
                    .nth(3)
                    .and_then(|v| v.parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
}

/// Open a URL in the default browser.
#[tauri::command]
pub async fn open_browser(url: String) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("open")
            .arg(&url)
            .output()
            .await
            .map_err(|e| format!("브라우저 열기 실패: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        tokio::process::Command::new("xdg-open")
            .arg(&url)
            .output()
            .await
            .map_err(|e| format!("브라우저 열기 실패: {e}"))?;
    }
    Ok(format!("열림: {url}"))
}

/// Open an application by name.
/// Requires a valid one-shot approval nonce from `request_computer_use_approval`.
#[tauri::command]
pub async fn open_app(
    state: tauri::State<'_, crate::state::SharedState>,
    app_name: String,
    approval_nonce: String,
) -> Result<String, String> {
    crate::commands::computer_use::consume_approval_pub(&state, &approval_nonce).await?;

    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("open")
            .args(["-a", &app_name])
            .output()
            .await
            .map_err(|e| format!("{app_name} 실행 실패: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        tokio::process::Command::new(&app_name)
            .spawn()
            .map_err(|e| format!("{app_name} 실행 실패: {e}"))?;
    }
    Ok(format!("{app_name} 실행됨"))
}

/// List files in a directory.
#[tauri::command]
pub async fn list_files(path: String) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(&path).map_err(|e| format!("디렉토리 읽기 실패: {e}"))?;
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        files.push(if is_dir { format!("{name}/") } else { name });
    }
    files.sort();
    Ok(files)
}

/// Get running processes summary.
#[tauri::command]
pub async fn list_processes() -> Result<Vec<String>, String> {
    let output = tokio::process::Command::new("ps")
        .args(["aux", "--sort=-%mem"])
        .output()
        .await
        .map_err(|e| format!("프로세스 목록 실패: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().take(20).map(|l| l.to_string()).collect())
}
