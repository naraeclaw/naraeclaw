//! Tauri IPC commands for config management and onboarding.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".naraeclaw").join("config.toml")
}

pub fn config_path_pub() -> PathBuf {
    config_path()
}

/// Check if config.toml exists.
#[tauri::command]
pub fn config_exists() -> bool {
    config_path().exists()
}

/// Check if Ollama is installed and running.
#[tauri::command]
pub async fn check_ollama() -> Result<OllamaStatus, String> {
    let installed = tokio::process::Command::new("ollama")
        .arg("--version")
        .output()
        .await
        .is_ok();

    let running = if installed {
        reqwest::Client::new()
            .get("http://127.0.0.1:11434/api/tags")
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
            .is_ok()
    } else {
        false
    };

    let models = if running {
        list_ollama_models().await.unwrap_or_default()
    } else {
        vec![]
    };

    Ok(OllamaStatus {
        installed,
        running,
        models,
    })
}

/// List locally available Ollama models.
async fn list_ollama_models() -> Result<Vec<String>, String> {
    let resp = reqwest::Client::new()
        .get("http://127.0.0.1:11434/api/tags")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let models = body["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(models)
}

/// Pull an Ollama model. Returns when the pull completes.
#[tauri::command]
pub async fn ollama_pull(model: String) -> Result<String, String> {
    let output = tokio::process::Command::new("ollama")
        .args(["pull", &model])
        .output()
        .await
        .map_err(|e| format!("ollama pull 실행 실패: {e}"))?;
    if output.status.success() {
        Ok(format!("{model} 다운로드 완료"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("ollama pull 실패: {stderr}"))
    }
}

/// Start Ollama server if not running.
#[tauri::command]
pub async fn ollama_start() -> Result<String, String> {
    // Check if already running.
    if reqwest::Client::new()
        .get("http://127.0.0.1:11434/api/tags")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok()
    {
        return Ok("이미 실행 중".into());
    }

    tokio::process::Command::new("ollama")
        .arg("serve")
        .spawn()
        .map_err(|e| format!("ollama serve 실행 실패: {e}"))?;

    // Wait for it to be ready.
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if reqwest::Client::new()
            .get("http://127.0.0.1:11434/api/tags")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .is_ok()
        {
            return Ok("Ollama 시작됨".into());
        }
    }
    Err("Ollama 시작 타임아웃".into())
}

/// Save onboarding config and start the gateway.
#[tauri::command]
pub async fn complete_onboarding(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::state::SharedState>,
    settings: OnboardingSettings,
) -> Result<String, String> {
    let config_dir = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".naraeclaw")
    };
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("디렉토리 생성 실패: {e}"))?;

    let toml = build_config_toml(&settings).map_err(|e| format!("config 생성 실패: {e}"))?;
    std::fs::write(config_dir.join("config.toml"), toml)
        .map_err(|e| format!("config.toml 저장 실패: {e}"))?;

    // Start the gateway sidecar now that config exists.
    crate::start_gateway_and_show(app, state.inner().clone());

    Ok("설정 완료".into())
}

/// Build onboarding config TOML via typed `toml::Table` to prevent config injection.
///
/// String interpolation with `format!` would allow values like
/// `"\n[gateway]\nrequire_pairing = false` to break out of the intended field.
/// Using `toml::Value` as the intermediate type ensures all strings are
/// properly escaped by the serializer.
fn build_config_toml(s: &OnboardingSettings) -> Result<String, String> {
    let mut root = toml::Table::new();

    root.insert(
        "default_provider".into(),
        toml::Value::String(s.provider.clone()),
    );
    root.insert("default_model".into(), toml::Value::String(s.model.clone()));
    if let Some(ref key) = s.api_key {
        root.insert("api_key".into(), toml::Value::String(key.clone()));
    }
    if s.provider == "ollama" {
        root.insert(
            "api_url".into(),
            toml::Value::String("http://127.0.0.1:11434".into()),
        );
    }
    root.insert("default_temperature".into(), toml::Value::Float(0.7));

    let mut memory = toml::Table::new();
    memory.insert("backend".into(), toml::Value::String("sqlite".into()));
    memory.insert("auto_save".into(), toml::Value::Boolean(true));
    root.insert("memory".into(), toml::Value::Table(memory));

    let mut knowledge = toml::Table::new();
    knowledge.insert("enabled".into(), toml::Value::Boolean(true));
    knowledge.insert("auto_capture".into(), toml::Value::Boolean(true));
    knowledge.insert("suggest_on_query".into(), toml::Value::Boolean(true));
    root.insert("knowledge".into(), toml::Value::Table(knowledge));

    let mut gateway = toml::Table::new();
    gateway.insert("port".into(), toml::Value::Integer(42617));
    gateway.insert("require_pairing".into(), toml::Value::Boolean(true));
    root.insert("gateway".into(), toml::Value::Table(gateway));

    toml::to_string_pretty(&toml::Value::Table(root))
        .map_err(|e| format!("config 직렬화 실패: {e}"))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub installed: bool,
    pub running: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct OnboardingSettings {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
}

// ── Self-healing commands ──────────────────────────────────────

/// Check if Ollama server is healthy.
#[tauri::command]
pub async fn ollama_health() -> bool {
    reqwest::Client::new()
        .get("http://127.0.0.1:11434/api/tags")
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Remove a broken model and re-pull it.
#[tauri::command]
pub async fn ollama_repair_model(model: String) -> Result<String, String> {
    // Remove the broken model.
    let rm = tokio::process::Command::new("ollama")
        .args(["rm", &model])
        .output()
        .await;
    if let Err(e) = rm {
        return Err(format!("모델 삭제 실패: {e}"));
    }

    // Re-pull.
    let pull = tokio::process::Command::new("ollama")
        .args(["pull", &model])
        .output()
        .await
        .map_err(|e| format!("모델 다운로드 실패: {e}"))?;

    if pull.status.success() {
        Ok(format!("{model} 재설치 완료"))
    } else {
        let stderr = String::from_utf8_lossy(&pull.stderr);
        Err(format!("모델 다운로드 실패: {stderr}"))
    }
}

/// Restart the gateway sidecar.
#[tauri::command]
pub async fn restart_gateway(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::state::SharedState>,
) -> Result<String, String> {
    crate::sidecar::shutdown_agent().await;
    crate::start_gateway_and_show(app, state.inner().clone());
    Ok("Gateway 재시작 중".into())
}

/// Get current config as JSON for the settings UI.
#[tauri::command]
pub fn get_config() -> Result<serde_json::Value, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let val: toml::Value = content
        .parse()
        .map_err(|e: toml::de::Error| e.to_string())?;
    // Convert TOML to JSON for the frontend.
    let json = serde_json::to_value(val).map_err(|e| e.to_string())?;
    Ok(json)
}

/// Update a config field and save.
#[tauri::command]
pub fn update_config(key: String, value: String) -> Result<String, String> {
    let path = config_path();
    let content = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let mut doc: toml::Table = content.parse().unwrap_or_default();
    doc.insert(key.clone(), toml::Value::String(value));
    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(format!("{key} 업데이트 완료"))
}
