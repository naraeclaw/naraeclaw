//! Tauri IPC commands for scheduled tasks (natural language cron).

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScheduledTask {
    pub id: String,
    pub description: String,
    pub schedule_display: String,
    pub enabled: bool,
}

/// List scheduled tasks from gateway.
#[tauri::command]
pub async fn list_tasks(
    state: tauri::State<'_, crate::state::SharedState>,
) -> Result<Vec<ScheduledTask>, String> {
    let (url, token) = {
        let s = state.read().await;
        (s.gateway_url.clone(), s.token.clone())
    };

    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    let resp = client
        .get_json("/api/cron/jobs")
        .await
        .map_err(|e| e.to_string())?;

    let tasks = resp
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|j| ScheduledTask {
                    id: j["id"].as_str().unwrap_or("").into(),
                    description: j["description"].as_str().unwrap_or("").into(),
                    schedule_display: j["schedule_display"]
                        .as_str()
                        .or(j["schedule"].as_str())
                        .unwrap_or("")
                        .into(),
                    enabled: j["enabled"].as_bool().unwrap_or(true),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(tasks)
}

/// Create a task from natural language description.
/// The gateway/agent will parse the natural language into a cron schedule.
#[tauri::command]
pub async fn create_task_natural(
    state: tauri::State<'_, crate::state::SharedState>,
    description: String,
) -> Result<String, String> {
    let (url, token) = {
        let s = state.read().await;
        (s.gateway_url.clone(), s.token.clone())
    };

    // Send as a webhook message — the agent will parse and create the cron job.
    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    let message = format!("예약 작업을 만들어줘: {description}");
    client
        .send_webhook_message(&message)
        .await
        .map_err(|e| format!("작업 생성 실패: {e}"))?;

    Ok("예약 작업 요청 전송됨".into())
}

/// Delete a scheduled task.
#[tauri::command]
pub async fn delete_task(
    state: tauri::State<'_, crate::state::SharedState>,
    task_id: String,
) -> Result<String, String> {
    let (url, token) = {
        let s = state.read().await;
        (s.gateway_url.clone(), s.token.clone())
    };

    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    client
        .delete_json(&format!("/api/cron/jobs/{task_id}"))
        .await
        .map_err(|e| format!("작업 삭제 실패: {e}"))?;

    Ok("작업 삭제됨".into())
}
