//! Background health polling for the NaraeClaw gateway.

use crate::gateway_client::GatewayClient;
use crate::state::SharedState;
use crate::tray::icon;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Runtime};

const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Consecutive gateway failures before attempting sidecar restart.
static CONSECUTIVE_FAILURES: AtomicU32 = AtomicU32::new(0);
const RESTART_THRESHOLD: u32 = 3;

/// Spawn a background task that polls gateway health and updates state + tray.
/// Also handles self-healing: restarts sidecar if gateway goes down.
pub fn spawn_health_poller<R: Runtime>(app: AppHandle<R>, state: SharedState) {
    tauri::async_runtime::spawn(async move {
        loop {
            let (url, token) = {
                let s = state.read().await;
                (s.gateway_url.clone(), s.token.clone())
            };

            let client = GatewayClient::new(&url, token.as_deref());
            let healthy = client.get_health().await.unwrap_or(false);

            if healthy {
                CONSECUTIVE_FAILURES.store(0, Ordering::Relaxed);

                // Check Ollama health when provider is ollama.
                let is_ollama = {
                    let s = state.read().await;
                    // Simple heuristic: check if gateway reports ollama as provider
                    true // Always check — cheap HTTP call
                };
                if is_ollama {
                    let ollama_ok = reqwest::Client::new()
                        .get("http://127.0.0.1:11434/api/tags")
                        .timeout(std::time::Duration::from_secs(3))
                        .send()
                        .await
                        .map(|r| r.status().is_success())
                        .unwrap_or(false);
                    if !ollama_ok {
                        tracing::warn!("Ollama not responding, attempting to start");
                        let _ = tokio::process::Command::new("ollama")
                            .arg("serve")
                            .spawn();
                    }
                }
            } else {
                let failures = CONSECUTIVE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
                if failures == RESTART_THRESHOLD {
                    tracing::warn!(
                        "Gateway unhealthy for {failures} checks, attempting sidecar restart"
                    );
                    // Try to restart the sidecar automatically.
                    if let Err(e) = crate::sidecar::spawn_agent().await {
                        tracing::error!("Sidecar restart failed: {e}");
                    }
                }
            }

            let (connected, agent_status) = {
                let mut s = state.write().await;
                s.connected = healthy;
                (s.connected, s.agent_status)
            };

            if let Some(tray) = app.tray_by_id("main") {
                let _ = tray.set_icon(Some(icon::icon_for_state(connected, agent_status)));
                let _ = tray.set_tooltip(Some(icon::tooltip_for_state(connected, agent_status)));
            }

            let _ = app.emit("naraeclaw://status-changed", healthy);

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}
