//! Tauri IPC commands for channel configuration.

use serde::{Deserialize, Serialize};

const DEFAULT_WEBHOOK_PORT: u16 = 42618;

#[derive(Debug, Serialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub connected: bool,
    pub needs: Vec<String>,
    pub description: String,
}

/// List available channels with their status.
#[tauri::command]
pub async fn get_channels(
    state: tauri::State<'_, crate::state::SharedState>,
) -> Result<Vec<ChannelInfo>, String> {
    let s = state.read().await;
    let url = &s.gateway_url;
    let token = s.token.as_deref();

    let client = crate::gateway_client::GatewayClient::new(url, token);
    let gateway_channels = client
        .list_channels_raw()
        .await
        .unwrap_or_else(|_| serde_json::json!([]));

    let mut channels = vec![
        ChannelInfo {
            id: "webhook".into(),
            name: "Webhook".into(),
            enabled: false,
            connected: false,
            needs: vec!["secret".into()],
            description: "HTTP Webhook으로 에이전트에 메시지를 보냅니다. 비밀 키(선택)로 요청을 검증합니다.".into(),
        },
        ChannelInfo {
            id: "mqtt".into(),
            name: "MQTT".into(),
            enabled: false,
            connected: false,
            needs: vec!["broker_url".into(), "topics".into()],
            description: "MQTT 브로커를 통해 IoT/서버 환경에서 에이전트와 통신합니다.".into(),
        },
    ];

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
    // Webhook fields
    pub port: Option<u16>,
    pub secret: Option<String>,
    // MQTT fields
    pub broker_url: Option<String>,
    pub client_id: Option<String>,
    pub topics: Option<Vec<String>>,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Save channel settings to config.toml and restart gateway.
#[tauri::command]
pub async fn save_channel(settings: ChannelSettings) -> Result<String, String> {
    let path = super::config::config_path_pub();
    let content = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let mut doc: toml::Table = content.parse().unwrap_or_default();

    let channels = doc
        .entry("channels_config")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or("channels_config is not a table")?;

    match settings.channel.as_str() {
        "webhook" => {
            let section = channels
                .entry("webhook")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or("webhook section is not a table")?;
            section.insert("enabled".into(), toml::Value::Boolean(true));
            // port is required in WebhookConfig — use default if not provided.
            let port = settings.port.unwrap_or(DEFAULT_WEBHOOK_PORT);
            section.insert("port".into(), toml::Value::Integer(i64::from(port)));
            if let Some(ref s) = settings.secret {
                section.insert("secret".into(), toml::Value::String(s.clone()));
            }
        }
        "mqtt" => {
            let broker_url = settings
                .broker_url
                .as_deref()
                .filter(|u| !u.is_empty())
                .ok_or("broker_url은 필수입니다")?;

            // Validate broker_url scheme and derive use_tls.
            let use_tls = if broker_url.starts_with("mqtts://") {
                true
            } else if broker_url.starts_with("mqtt://") {
                false
            } else {
                return Err("broker_url은 'mqtt://' 또는 'mqtts://'로 시작해야 합니다".into());
            };

            let topics = settings
                .topics
                .as_deref()
                .filter(|t| !t.is_empty())
                .ok_or("최소 하나의 topic이 필요합니다")?;

            // Auto-generate client_id if not provided.
            let client_id = settings
                .client_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(generate_client_id);

            let section = channels
                .entry("mqtt")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or("mqtt section is not a table")?;
            section.insert("enabled".into(), toml::Value::Boolean(true));
            section.insert("broker_url".into(), toml::Value::String(broker_url.to_string()));
            section.insert("client_id".into(), toml::Value::String(client_id));
            section.insert(
                "topics".into(),
                toml::Value::Array(
                    topics
                        .iter()
                        .map(|t| toml::Value::String(t.clone()))
                        .collect(),
                ),
            );
            section.insert("use_tls".into(), toml::Value::Boolean(use_tls));
            if let Some(ref u) = settings.username {
                section.insert("username".into(), toml::Value::String(u.clone()));
            }
            if let Some(ref p) = settings.password {
                section.insert("password".into(), toml::Value::String(p.clone()));
            }
        }
        other => return Err(format!("지원하지 않는 채널: {other}")),
    }

    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, out).map_err(|e| e.to_string())?;

    crate::sidecar::shutdown_agent().await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    crate::sidecar::spawn_agent()
        .await
        .map_err(|e| format!("Gateway 재시작 실패: {e}"))?;

    Ok(format!("{} 저장 완료 — Gateway 재시작 중", settings.channel))
}

fn generate_client_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("naraeclaw-{t:x}")
}
