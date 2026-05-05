//! 에이전트 프로필 CRUD — ~/.naraeclaw/profiles/<id>.toml + active_profile 마커

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn naraeclaw_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".naraeclaw")
}

fn profiles_dir() -> PathBuf {
    naraeclaw_dir().join("profiles")
}

fn active_profile_path() -> PathBuf {
    naraeclaw_dir().join("active_profile")
}

fn config_path() -> PathBuf {
    naraeclaw_dir().join("config.toml")
}

fn profile_path(id: &str) -> PathBuf {
    profiles_dir().join(format!("{}.toml", sanitize_id(id)))
}

/// Allow only alphanumerics, hyphens, and underscores to avoid path traversal.
fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key_set: bool,
    pub api_url: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProfileRequest {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_url: Option<String>,
    pub description: Option<String>,
}

/// Ensure profiles dir exists and migrate existing config.toml into "default" profile if needed.
fn ensure_profiles_dir() -> Result<(), String> {
    let dir = profiles_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("profiles 디렉토리 생성 실패: {e}"))?;

    // If no profiles exist yet but config.toml does, migrate it as "default".
    if dir.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        let cfg = config_path();
        if cfg.exists() {
            let content = std::fs::read_to_string(&cfg).unwrap_or_default();
            // Inject metadata header comment.
            let migrated = format!(
                "# name = \"기본 프로필\"\n# description = \"기존 설정에서 가져온 프로필\"\n# created_at = \"{}\"\n\n{}",
                chrono_now(),
                content
            );
            std::fs::write(dir.join("default.toml"), migrated)
                .map_err(|e| format!("기본 프로필 마이그레이션 실패: {e}"))?;

            // Mark as active.
            if !active_profile_path().exists() {
                std::fs::write(active_profile_path(), "default")
                    .map_err(|e| format!("active_profile 마커 생성 실패: {e}"))?;
            }
        }
    }
    Ok(())
}

fn chrono_now() -> String {
    // Simple ISO-8601 date without chrono dependency.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let y = 1970 + days / 365;
    format!("{y}-01-01T00:00:00Z") // approximate; good enough for display
}

fn active_profile_id() -> Option<String> {
    std::fs::read_to_string(active_profile_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read display metadata from a profile TOML (looks for comment-style headers or toml keys).
fn read_profile_meta(id: &str, content: &str, is_active: bool) -> ProfileMeta {
    // Parse as TOML to extract fields.
    let table: toml::Table = toml::from_str(content).unwrap_or_default();

    let get_str = |key: &str| -> Option<String> {
        table.get(key).and_then(|v| v.as_str()).map(String::from)
    };

    // Extract from comment headers as fallback (# name = "...").
    let comment_meta = |key: &str| -> Option<String> {
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix(&format!("# {} = ", key)) {
                return Some(rest.trim_matches('"').to_string());
            }
        }
        None
    };

    let name = get_str("_profile_name")
        .or_else(|| comment_meta("name"))
        .unwrap_or_else(|| id.to_string());

    let description = get_str("_profile_description").or_else(|| comment_meta("description"));
    let created_at = get_str("_profile_created_at")
        .or_else(|| comment_meta("created_at"))
        .unwrap_or_else(chrono_now);

    let provider = get_str("default_provider").unwrap_or_else(|| "unknown".into());
    let model = get_str("default_model").unwrap_or_else(|| "unknown".into());
    let api_key_set = get_str("api_key").map(|v| !v.is_empty()).unwrap_or(false);
    let api_url = get_str("api_url");

    ProfileMeta { id: id.to_string(), name, provider, model, api_key_set, api_url, is_active, created_at, description }
}

/// List all agent profiles.
#[tauri::command]
pub fn list_profiles() -> Result<Vec<ProfileMeta>, String> {
    ensure_profiles_dir()?;
    let active = active_profile_id();
    let dir = profiles_dir();

    let mut profiles: Vec<ProfileMeta> = std::fs::read_dir(&dir)
        .map_err(|e| format!("profiles 디렉토리 읽기 실패: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("toml"))
        .filter_map(|e| {
            let path = e.path();
            let id = path.file_stem()?.to_str()?.to_string();
            let content = std::fs::read_to_string(&path).ok()?;
            let is_active = active.as_deref() == Some(&id);
            Some(read_profile_meta(&id, &content, is_active))
        })
        .collect();

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}

/// Create a new agent profile.
#[tauri::command]
pub fn create_profile(req: CreateProfileRequest) -> Result<ProfileMeta, String> {
    ensure_profiles_dir()?;

    let id = sanitize_id(&req.id);
    if id.is_empty() {
        return Err("프로필 ID가 유효하지 않습니다".into());
    }

    let path = profile_path(&id);
    if path.exists() {
        return Err(format!("'{id}' 프로필이 이미 존재합니다"));
    }

    let created_at = chrono_now();
    let mut table = toml::Table::new();
    table.insert("_profile_name".into(), toml::Value::String(req.name.clone()));
    table.insert("_profile_created_at".into(), toml::Value::String(created_at.clone()));
    if let Some(ref desc) = req.description {
        table.insert("_profile_description".into(), toml::Value::String(desc.clone()));
    }
    table.insert("default_provider".into(), toml::Value::String(req.provider.clone()));
    table.insert("default_model".into(), toml::Value::String(req.model.clone()));
    if let Some(ref key) = req.api_key {
        if !key.is_empty() {
            table.insert("api_key".into(), toml::Value::String(key.clone()));
        }
    }
    if let Some(ref url) = req.api_url {
        if !url.is_empty() {
            table.insert("api_url".into(), toml::Value::String(url.clone()));
        }
    }
    if req.provider == "ollama" && req.api_url.as_deref().unwrap_or("").is_empty() {
        table.insert("api_url".into(), toml::Value::String("http://127.0.0.1:11434".into()));
    }

    let content = toml::to_string_pretty(&table).map_err(|e| format!("TOML 직렬화 실패: {e}"))?;
    std::fs::write(&path, content).map_err(|e| format!("프로필 저장 실패: {e}"))?;

    let meta = ProfileMeta {
        id,
        name: req.name,
        provider: req.provider,
        model: req.model,
        api_key_set: req.api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
        api_url: req.api_url,
        is_active: false,
        created_at,
        description: req.description,
    };
    Ok(meta)
}

/// Activate a profile: copy its TOML to config.toml and update the marker.
#[tauri::command]
pub async fn switch_profile(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::state::SharedState>,
    id: String,
) -> Result<String, String> {
    ensure_profiles_dir()?;
    let id = sanitize_id(&id);
    let path = profile_path(&id);
    if !path.exists() {
        return Err(format!("'{id}' 프로필을 찾을 수 없습니다"));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("프로필 읽기 실패: {e}"))?;

    // Strip internal metadata keys before writing to config.toml.
    let table: toml::Table = toml::from_str(&content).unwrap_or_default();
    let config_table: toml::Table = table
        .into_iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .collect();
    let config_content = toml::to_string_pretty(&config_table)
        .map_err(|e| format!("config 직렬화 실패: {e}"))?;

    std::fs::write(config_path(), config_content)
        .map_err(|e| format!("config.toml 저장 실패: {e}"))?;
    std::fs::write(active_profile_path(), &id)
        .map_err(|e| format!("active_profile 마커 저장 실패: {e}"))?;

    // Restart gateway to pick up new config.
    crate::start_gateway_and_show(app, state.inner().clone());

    Ok(format!("'{id}' 프로필로 전환했습니다"))
}

/// Delete a profile. Cannot delete the currently active profile.
#[tauri::command]
pub fn delete_profile(id: String) -> Result<(), String> {
    let id = sanitize_id(&id);
    if active_profile_id().as_deref() == Some(&id) {
        return Err("현재 활성 프로필은 삭제할 수 없습니다".into());
    }
    let path = profile_path(&id);
    if !path.exists() {
        return Err(format!("'{id}' 프로필을 찾을 수 없습니다"));
    }
    std::fs::remove_file(&path).map_err(|e| format!("프로필 삭제 실패: {e}"))?;
    Ok(())
}

/// Update profile display name / description (non-destructive metadata edit).
#[tauri::command]
pub fn update_profile_meta(id: String, name: String, description: Option<String>) -> Result<(), String> {
    let id = sanitize_id(&id);
    let path = profile_path(&id);
    if !path.exists() {
        return Err(format!("'{id}' 프로필을 찾을 수 없습니다"));
    }

    let content = std::fs::read_to_string(&path).map_err(|e| format!("읽기 실패: {e}"))?;
    let mut table: toml::Table = toml::from_str(&content).unwrap_or_default();
    table.insert("_profile_name".into(), toml::Value::String(name));
    if let Some(desc) = description {
        table.insert("_profile_description".into(), toml::Value::String(desc));
    } else {
        table.remove("_profile_description");
    }

    let updated = toml::to_string_pretty(&table).map_err(|e| format!("직렬화 실패: {e}"))?;
    std::fs::write(&path, updated).map_err(|e| format!("저장 실패: {e}"))?;
    Ok(())
}
