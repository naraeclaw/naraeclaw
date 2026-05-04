//! Tauri IPC commands for knowledge management via gateway memory API.

/// Save a memory entry as wiki page via gateway.
#[tauri::command]
pub async fn memory_to_wiki(
    state: tauri::State<'_, crate::state::SharedState>,
    title: String,
    content: String,
) -> Result<String, String> {
    let key = format!(
        "wiki/{}",
        title
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
    );
    let md = format!("# {title}\n\n{content}\n");

    let (url, token) = {
        let s = state.read().await;
        (s.gateway_url.clone(), s.token.clone())
    };

    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    client
        .post_json(
            "/api/memory",
            &serde_json::json!({ "key": key, "content": md, "category": "wiki" }),
        )
        .await
        .map_err(|e| format!("위키 저장 실패: {e}"))?;

    Ok(format!("위키에 저장됨: {key}"))
}

/// List memory entries from gateway.
#[tauri::command]
pub async fn list_memories(
    state: tauri::State<'_, crate::state::SharedState>,
    query: Option<String>,
) -> Result<serde_json::Value, String> {
    let (url, token) = {
        let s = state.read().await;
        (s.gateway_url.clone(), s.token.clone())
    };
    let client = crate::gateway_client::GatewayClient::new(&url, token.as_deref());
    let path = if let Some(q) = query {
        format!("/api/memory?query={}", urlencoding::encode(&q))
    } else {
        "/api/memory".into()
    };
    client.get_json(&path).await.map_err(|e| e.to_string())
}
