//! Tauri IPC commands for remote NaraeClaw server management.
//!
//! ## Token security
//!
//! Bearer tokens are kept in session memory (`SharedState.token`) only.
//! The persisted store records the server URL and metadata but never the token.
//! On reconnect the user must re-enter the token, which prevents an attacker
//! with read access to `~/Library/Application Support/ai.naraeclaw.desktop/`
//! from obtaining valid gateway credentials.

use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

/// Server record persisted to the store (token intentionally excluded).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredServer {
    pub id: String,
    pub name: String,
    pub url: String,
    pub connected: bool,
}

/// Server record returned to the frontend (token field present but never
/// populated from the store — only echoed back if just supplied by the user).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteServer {
    pub id: String,
    pub name: String,
    pub url: String,
    /// Always `None` when read back from the store.
    pub token: Option<String>,
    pub connected: bool,
}

impl From<StoredServer> for RemoteServer {
    fn from(s: StoredServer) -> Self {
        RemoteServer {
            id: s.id,
            name: s.name,
            url: s.url,
            token: None, // never expose stored data as a token
            connected: s.connected,
        }
    }
}

/// List saved remote servers from store. Tokens are never returned.
#[tauri::command]
pub async fn list_servers(app: tauri::AppHandle) -> Vec<RemoteServer> {
    let store = match app.store("naraeclaw.json") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    store
        .get("remote_servers")
        .and_then(|v| serde_json::from_value::<Vec<StoredServer>>(v.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .map(RemoteServer::from)
        .collect()
}

/// Add a remote server and test connectivity.
/// The token is used for connectivity testing and returned to the caller for
/// session use, but is NOT persisted to the store.
#[tauri::command]
pub async fn add_server(
    app: tauri::AppHandle,
    name: String,
    url: String,
    token: Option<String>,
) -> Result<RemoteServer, String> {
    // Test connectivity.
    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    let connected = client.get_health().await.unwrap_or(false);

    let stored = StoredServer {
        id: uuid_simple(),
        name: name.clone(),
        url: url.clone(),
        connected,
    };

    // Persist URL + metadata only — no token.
    let store = app.store("naraeclaw.json").map_err(|e| e.to_string())?;
    let mut servers: Vec<StoredServer> = store
        .get("remote_servers")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    servers.push(stored.clone());
    store.set(
        "remote_servers",
        serde_json::to_value(&servers).unwrap_or_default(),
    );
    let _ = store.save();

    // Return server with token so the frontend can use it this session.
    let result = RemoteServer {
        id: stored.id,
        name,
        url: url.clone(),
        token,
        connected,
    };

    if !connected {
        return Err(format!(
            "서버 추가됨, 하지만 연결 실패. URL을 확인하세요: {url}"
        ));
    }
    Ok(result)
}

/// Remove a remote server.
#[tauri::command]
pub async fn remove_server(app: tauri::AppHandle, server_id: String) -> Result<String, String> {
    let store = app.store("naraeclaw.json").map_err(|e| e.to_string())?;
    let mut servers: Vec<StoredServer> = store
        .get("remote_servers")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    servers.retain(|s| s.id != server_id);
    store.set(
        "remote_servers",
        serde_json::to_value(&servers).unwrap_or_default(),
    );
    let _ = store.save();
    Ok("서버 제거됨".into())
}

/// Switch the active gateway to a remote server (or back to local).
#[tauri::command]
pub async fn switch_server(
    state: tauri::State<'_, crate::state::SharedState>,
    url: String,
    token: Option<String>,
) -> Result<String, String> {
    let mut s = state.write().await;
    s.gateway_url = url.clone();
    s.token = token; // kept in memory only, never re-persisted here
    s.connected = false;
    Ok(format!("서버 전환: {url}"))
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{t:x}")
}
