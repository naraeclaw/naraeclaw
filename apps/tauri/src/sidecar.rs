//! NaraeClaw agent sidecar management.
//!
//! Spawns and owns the `naraeclaw agent` subprocess that provides the gateway
//! HTTP/WebSocket server the Tauri WebView connects to.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::Mutex;

static SIDECAR: OnceLock<Mutex<Option<tokio::process::Child>>> = OnceLock::new();

fn sidecar_slot() -> &'static Mutex<Option<tokio::process::Child>> {
    SIDECAR.get_or_init(|| Mutex::new(None))
}

/// Target triple embedded by build.rs — used to find the Tauri-bundled sidecar binary.
const TARGET_TRIPLE: &str = env!("NARAECLAW_TARGET_TRIPLE");

/// Resolve the path to the `naraeclaw` binary.
///
/// Resolution order:
/// 1. `naraeclaw-{triple}` sibling to current exe — Tauri's sidecar naming convention
///    for the bundled `.app`
/// 2. `naraeclaw` sibling to current exe — plain name fallback
/// 3. `NARAECLAW_BIN` environment variable override
/// 4. Walk up from the current exe looking for `target/release/naraeclaw` then
///    `target/debug/naraeclaw` — covers `cargo tauri dev` with either profile
/// 5. `naraeclaw` on `PATH`
fn resolve_binary() -> PathBuf {
    // 1 & 2. Sibling to current exe (bundled Tauri app places sidecar next to main binary)
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent().unwrap_or(std::path::Path::new("."));
        // Tauri names the sidecar with the target triple suffix
        let suffixed = dir.join(format!("naraeclaw-{TARGET_TRIPLE}"));
        if suffixed.exists() {
            return suffixed;
        }
        let plain = dir.join("naraeclaw");
        if plain.exists() {
            return plain;
        }
    }

    // 3. Explicit env override
    if let Ok(path) = std::env::var("NARAECLAW_BIN") {
        return PathBuf::from(path);
    }

    // 4. Walk up from exe to find workspace root (development builds)
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(std::path::Path::to_path_buf);
        for _ in 0..10 {
            if let Some(ref d) = dir {
                // Check release before debug so `cargo build --release` is preferred
                for profile in ["release", "debug"] {
                    let candidate = d.join("target").join(profile).join("naraeclaw");
                    if candidate.exists() {
                        return candidate;
                    }
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }

    // 5. Rely on PATH
    PathBuf::from("naraeclaw")
}

/// Spawn `naraeclaw agent` in the background.
///
/// Safe to call multiple times — only spawns once. If a previous sidecar
/// exited unexpectedly, the slot is cleared and a new one is spawned.
/// The gateway port is inherited from `NARAECLAW_GATEWAY_PORT` /
/// `ZEROCLAW_GATEWAY_PORT` env vars if set, otherwise the binary uses its
/// compiled-in default (42617).
pub async fn spawn_agent() -> Result<()> {
    let mut slot = sidecar_slot().lock().await;

    // If a previous child exited, clear the slot so we can respawn.
    if let Some(child) = slot.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                tracing::warn!("naraeclaw agent sidecar exited with {status}, respawning");
                *slot = None;
            }
            Ok(None) => return Ok(()), // still running
            Err(_) => {
                *slot = None;
            }
        }
    }

    let binary = resolve_binary();
    tracing::info!("Starting naraeclaw agent sidecar: {}", binary.display());

    let child = tokio::process::Command::new(&binary)
        .args(["daemon"])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn naraeclaw at '{}'", binary.display()))?;

    *slot = Some(child);

    // Brief wait to catch immediate failures (e.g. port conflict).
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    if let Some(child) = slot.as_mut() {
        if let Ok(Some(status)) = child.try_wait() {
            let _ = slot.take();
            anyhow::bail!(
                "Gateway exited immediately ({}). Port 42617 may be in use.",
                status
            );
        }
    }

    Ok(())
}

/// Gracefully stop the agent sidecar.
///
/// On Unix, sends SIGTERM first and waits up to 5 seconds for the process
/// to exit cleanly before falling back to SIGKILL. On other platforms,
/// kills immediately. Called on `RunEvent::Exit` to ensure the gateway
/// process does not outlive the Tauri app.
pub async fn shutdown_agent() {
    let mut slot = sidecar_slot().lock().await;
    let Some(mut child) = slot.take() else {
        return;
    };

    // Try graceful shutdown on Unix.
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            // Send SIGTERM via the kill command.
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
            // Wait up to 5 seconds for clean exit.
            for _ in 0..50 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if let Ok(Some(_)) = child.try_wait() {
                    tracing::info!("naraeclaw agent sidecar stopped gracefully");
                    return;
                }
            }
            tracing::warn!("naraeclaw agent sidecar did not exit after SIGTERM, sending SIGKILL");
        }
    }

    if let Err(e) = child.kill().await {
        tracing::warn!("Failed to kill naraeclaw agent sidecar: {e}");
    } else {
        tracing::info!("naraeclaw agent sidecar stopped");
    }
}
