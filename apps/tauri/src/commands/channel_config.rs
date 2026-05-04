//! Tauri IPC commands for channel configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub connected: bool,
    pub needs: Vec<String>,
}

/// List available channels with their status.
#[tauri::command]
pub async fn get_channels(
    state: tauri::State<'_, crate::state::SharedState>,
) -> Result<Vec<ChannelInfo>, String> {
    let s = state.read().await;
    let url = &s.gateway_url;
    let token = s.token.as_deref();

    // Try to get channel status from gateway.
    let client = crate::gateway_client::GatewayClient::new(url, token);
    let gateway_channels = client
        .list_channels_raw()
        .await
        .unwrap_or_else(|_| serde_json::json!([]));

    let mut channels = vec![
        ChannelInfo {
            id: "telegram".into(),
            name: "Telegram".into(),
            enabled: false,
            connected: false,
            needs: vec!["bot_token".into()],
        },
        ChannelInfo {
            id: "slack".into(),
            name: "Slack".into(),
            enabled: false,
            connected: false,
            needs: vec!["bot_token".into(), "app_token".into()],
        },
        ChannelInfo {
            id: "discord".into(),
            name: "Discord".into(),
            enabled: false,
            connected: false,
            needs: vec!["bot_token".into()],
        },
    ];

    // Merge gateway status if available.
    if let Some(arr) = gateway_channels.as_array() {
        for ch in &mut channels {
            if let Some(gw) = arr.iter().find(|g| g["id"].as_str() == Some(&ch.id)) {
                ch.enabled = gw["enabled"].as_bool().unwrap_or(false);
                ch.connected = gw["connected"].as_bool().unwrap_or(false);
            }
        }
    }

    Ok(channels)
}

#[derive(Debug, Deserialize)]
pub struct ChannelSettings {
    pub channel: String,
    pub bot_token: Option<String>,
    pub app_token: Option<String>,
}

/// Save channel settings to config.toml.
#[tauri::command]
pub async fn save_channel(settings: ChannelSettings) -> Result<String, String> {
    let path = super::config::config_path_pub();
    let content = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let mut doc: toml::Table = content.parse().unwrap_or_default();

    // Ensure [channels_config] exists.
    let channels = doc
        .entry("channels_config")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or("channels_config is not a table")?;

    // Create channel section.
    let section = channels
        .entry(&settings.channel)
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or("channel section is not a table")?;

    section.insert("enabled".into(), toml::Value::Boolean(true));
    if let Some(ref token) = settings.bot_token {
        section.insert("bot_token".into(), toml::Value::String(token.clone()));
    }
    if let Some(ref token) = settings.app_token {
        section.insert("app_token".into(), toml::Value::String(token.clone()));
    }

    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, out).map_err(|e| e.to_string())?;

    // Restart gateway so it picks up the new channel config.
    crate::sidecar::shutdown_agent().await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    crate::sidecar::spawn_agent()
        .await
        .map_err(|e| format!("Gateway 재시작 실패: {e}"))?;

    Ok(format!(
        "{} 저장 완료 — Gateway 재시작 중",
        settings.channel
    ))
}
