mod automation;
mod channels;
mod config_types;
mod providers;
mod security;
mod skillforge;
mod tools;

pub use automation::*;
pub use channels::*;
pub use config_types::*;
pub use providers::*;
pub use security::*;
pub use skillforge::*;
pub use tools::*;

use crate::autonomy::AutonomyLevel;
use crate::domain_matcher::DomainMatcher;
use crate::provider_aliases::{is_glm_alias, is_zai_alias};
use crate::traits::{HasPropKind, PropKind};
use anyhow::{Context, Result};
use directories::UserDirs;
#[cfg(feature = "schema-export")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
#[cfg(unix)]
use tokio::fs::File;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

const SUPPORTED_PROXY_SERVICE_KEYS: &[&str] = &[
    "provider.anthropic",
    "provider.compatible",
    "provider.copilot",
    "provider.gemini",
    "provider.glm",
    "provider.ollama",
    "provider.openai",
    "provider.openrouter",
    "channel.discord",
    "channel.matrix",
    "channel.mattermost",
    "channel.nextcloud_talk",
    "channel.signal",
    "channel.slack",
    "channel.telegram",
    "channel.wati",
    "channel.whatsapp",
    "tool.browser",
    "tool.http_request",
    "tool.pushover",
    "tool.web_search",
    "memory.embeddings",
    "tunnel.custom",
    "transcription.groq",
];

const SUPPORTED_PROXY_SERVICE_SELECTORS: &[&str] = &[
    "provider.*",
    "channel.*",
    "tool.*",
    "memory.*",
    "tunnel.*",
    "transcription.*",
];

static RUNTIME_PROXY_CONFIG: OnceLock<RwLock<ProxyConfig>> = OnceLock::new();
static RUNTIME_PROXY_CLIENT_CACHE: OnceLock<RwLock<HashMap<String, reqwest::Client>>> =
    OnceLock::new();

fn app_env(suffix: &str) -> std::result::Result<String, std::env::VarError> {
    std::env::var(format!("NARAECLAW_{suffix}"))
        .or_else(|_| std::env::var(format!("ZEROCLAW_{suffix}")))
}

// ── Config impl ──────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        let home =
            UserDirs::new().map_or_else(|| PathBuf::from("."), |u| u.home_dir().to_path_buf());
        let naraeclaw_dir = home.join(".naraeclaw");

        Self {
            workspace_dir: naraeclaw_dir.join("workspace"),
            config_path: naraeclaw_dir.join("config.toml"),
            api_key: None,
            api_url: None,
            api_path: None,
            default_provider: Some("openrouter".to_string()),
            default_model: Some("anthropic/claude-sonnet-4.6".to_string()),
            model_providers: HashMap::new(),
            default_temperature: default_temperature(),
            provider_timeout_secs: default_provider_timeout_secs(),
            provider_max_tokens: None,
            extra_headers: HashMap::new(),
            observability: ObservabilityConfig::default(),
            autonomy: AutonomyConfig::default(),
            trust: crate::scattered_types::TrustConfig::default(),
            backup: BackupConfig::default(),
            data_retention: DataRetentionConfig::default(),
            cloud_ops: CloudOpsConfig::default(),
            security: SecurityConfig::default(),
            security_ops: SecurityOpsConfig::default(),
            runtime: RuntimeConfig::default(),
            reliability: ReliabilityConfig::default(),
            scheduler: SchedulerConfig::default(),
            agent: AgentConfig::default(),
            pacing: PacingConfig::default(),
            skills: SkillsConfig::default(),
            pipeline: PipelineConfig::default(),
            model_routes: Vec::new(),
            embedding_routes: Vec::new(),
            heartbeat: HeartbeatConfig::default(),
            cron: CronConfig::default(),
            skillforge: SkillForgeConfig::default(),
            channels_config: ChannelsConfig::default(),
            memory: MemoryConfig::default(),
            storage: StorageConfig::default(),
            tunnel: TunnelConfig::default(),
            gateway: GatewayConfig::default(),
            secrets: SecretsConfig::default(),
            browser: BrowserConfig::default(),
            browser_delegate: crate::scattered_types::BrowserDelegateConfig::default(),
            http_request: HttpRequestConfig::default(),
            multimodal: MultimodalConfig::default(),
            media_pipeline: MediaPipelineConfig::default(),
            web_fetch: WebFetchConfig::default(),
            link_enricher: LinkEnricherConfig::default(),
            text_browser: TextBrowserConfig::default(),
            web_search: WebSearchConfig::default(),
            google_workspace: GoogleWorkspaceConfig::default(),
            proxy: ProxyConfig::default(),
            identity: IdentityConfig::default(),
            cost: CostConfig::default(),
            delegate: DelegateToolConfig::default(),
            agents: HashMap::new(),
            swarms: HashMap::new(),
            hooks: HooksConfig::default(),
            query_classification: QueryClassificationConfig::default(),
            transcription: TranscriptionConfig::default(),
            tts: TtsConfig::default(),
            mcp: McpConfig::default(),
            nodes: NodesConfig::default(),
            workspace: WorkspaceConfig::default(),
            jira: JiraConfig::default(),
            redash: RedashConfig::default(),
            node_transport: NodeTransportConfig::default(),
            knowledge: KnowledgeConfig::default(),
            linkedin: LinkedInConfig::default(),
            image_gen: ImageGenConfig::default(),
            plugins: PluginsConfig::default(),
            locale: None,
            verifiable_intent: VerifiableIntentConfig::default(),
            claude_code: ClaudeCodeConfig::default(),
            claude_code_runner: ClaudeCodeRunnerConfig::default(),
            codex_cli: CodexCliConfig::default(),
            gemini_cli: GeminiCliConfig::default(),
            opencode_cli: OpenCodeCliConfig::default(),
            sop: SopConfig::default(),
            shell_tool: ShellToolConfig::default(),
        }
    }
}

fn default_config_and_workspace_dirs() -> Result<(PathBuf, PathBuf)> {
    let config_dir = default_config_dir()?;
    Ok((config_dir.clone(), config_dir.join("workspace")))
}

const ACTIVE_WORKSPACE_STATE_FILE: &str = "active_workspace.toml";

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
struct ActiveWorkspaceState {
    config_dir: String,
}

fn default_config_dir() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return Ok(PathBuf::from(home).join(".naraeclaw"));
    }

    let home = UserDirs::new()
        .map(|u| u.home_dir().to_path_buf())
        .context("Could not find home directory")?;
    Ok(home.join(".naraeclaw"))
}

fn active_workspace_state_path(default_dir: &Path) -> PathBuf {
    default_dir.join(ACTIVE_WORKSPACE_STATE_FILE)
}

/// Returns `true` if `path` lives under the OS temp directory.
fn is_temp_directory(path: &Path) -> bool {
    let temp = std::env::temp_dir();
    // Canonicalize when possible to handle symlinks (macOS /var → /private/var)
    let canon_temp = temp.canonicalize().unwrap_or_else(|_| temp.clone());
    let canon_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canon_path.starts_with(&canon_temp)
}

async fn load_persisted_workspace_dirs(
    default_config_dir: &Path,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let state_path = active_workspace_state_path(default_config_dir);
    if !state_path.exists() {
        return Ok(None);
    }

    let contents = match fs::read_to_string(&state_path).await {
        Ok(contents) => contents,
        Err(error) => {
            tracing::warn!(
                "Failed to read active workspace marker {}: {error}",
                state_path.display()
            );
            return Ok(None);
        }
    };

    let state: ActiveWorkspaceState = match toml::from_str(&contents) {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!(
                "Failed to parse active workspace marker {}: {error}",
                state_path.display()
            );
            return Ok(None);
        }
    };

    let raw_config_dir = state.config_dir.trim();
    if raw_config_dir.is_empty() {
        tracing::warn!(
            "Ignoring active workspace marker {} because config_dir is empty",
            state_path.display()
        );
        return Ok(None);
    }

    let parsed_dir = expand_tilde_path(raw_config_dir);
    let config_dir = if parsed_dir.is_absolute() {
        parsed_dir
    } else {
        default_config_dir.join(parsed_dir)
    };
    Ok(Some((config_dir.clone(), config_dir.join("workspace"))))
}

pub async fn persist_active_workspace_config_dir(config_dir: &Path) -> Result<()> {
    persist_active_workspace_config_dir_in(config_dir, &default_config_dir()?).await
}

/// Inner implementation that accepts the default config directory explicitly,
/// so callers (including tests) control where the marker is written without
/// manipulating process-wide environment variables.
async fn persist_active_workspace_config_dir_in(
    config_dir: &Path,
    default_config_dir: &Path,
) -> Result<()> {
    let state_path = active_workspace_state_path(default_config_dir);

    // Guard: refuse to write a temp-directory config_dir into a non-temp
    // default location. This prevents transient test runs or one-off
    // invocations from hijacking the real user's daemon config resolution.
    // When both paths are temp (e.g. in tests), the write is harmless.
    if is_temp_directory(config_dir) && !is_temp_directory(default_config_dir) {
        tracing::warn!(
            path = %config_dir.display(),
            "Refusing to persist temp directory as active workspace marker"
        );
        return Ok(());
    }

    if config_dir == default_config_dir {
        if state_path.exists() {
            fs::remove_file(&state_path).await.with_context(|| {
                format!(
                    "Failed to clear active workspace marker: {}",
                    state_path.display()
                )
            })?;
        }
        return Ok(());
    }

    fs::create_dir_all(&default_config_dir)
        .await
        .with_context(|| {
            format!(
                "Failed to create default config directory: {}",
                default_config_dir.display()
            )
        })?;

    let state = ActiveWorkspaceState {
        config_dir: config_dir.to_string_lossy().into_owned(),
    };
    let serialized =
        toml::to_string_pretty(&state).context("Failed to serialize active workspace marker")?;

    let temp_path = default_config_dir.join(format!(
        ".{ACTIVE_WORKSPACE_STATE_FILE}.tmp-{}",
        uuid::Uuid::new_v4()
    ));
    fs::write(&temp_path, serialized).await.with_context(|| {
        format!(
            "Failed to write temporary active workspace marker: {}",
            temp_path.display()
        )
    })?;

    if let Err(error) = fs::rename(&temp_path, &state_path).await {
        let _ = fs::remove_file(&temp_path).await;
        anyhow::bail!(
            "Failed to atomically persist active workspace marker {}: {error}",
            state_path.display()
        );
    }

    sync_directory(default_config_dir).await?;
    Ok(())
}

pub fn resolve_config_dir_for_workspace(workspace_dir: &Path) -> (PathBuf, PathBuf) {
    let workspace_config_dir = workspace_dir.to_path_buf();
    if workspace_config_dir.join("config.toml").exists() {
        return (
            workspace_config_dir.clone(),
            workspace_config_dir.join("workspace"),
        );
    }

    let naraeclaw_config_dir = workspace_dir
        .parent()
        .map(|parent| parent.join(".naraeclaw"));
    if let Some(config_dir) = naraeclaw_config_dir {
        if config_dir.join("config.toml").exists() {
            return (config_dir, workspace_config_dir);
        }

        if workspace_dir
            .file_name()
            .is_some_and(|name| name == std::ffi::OsStr::new("workspace"))
        {
            return (config_dir, workspace_config_dir);
        }
    }

    let legacy_config_dir = workspace_dir
        .parent()
        .map(|parent| parent.join(".naraeclaw"));
    if let Some(legacy_dir) = legacy_config_dir
        && legacy_dir.join("config.toml").exists()
    {
        return (legacy_dir, workspace_config_dir);
    }

    (
        workspace_config_dir.clone(),
        workspace_config_dir.join("workspace"),
    )
}

/// Resolve the current runtime config/workspace directories for onboarding flows.
///
/// This mirrors the same precedence used by `Config::load_or_init()`:
/// `NARAECLAW_CONFIG_DIR`/`ZEROCLAW_CONFIG_DIR` > `NARAECLAW_WORKSPACE`/`ZEROCLAW_WORKSPACE` > active workspace marker > defaults.
pub async fn resolve_runtime_dirs_for_onboarding() -> Result<(PathBuf, PathBuf)> {
    let (default_config_dir, default_workspace_dir) = default_config_and_workspace_dirs()?;
    let (config_dir, workspace_dir, _) =
        resolve_runtime_config_dirs(&default_config_dir, &default_workspace_dir).await?;
    Ok((config_dir, workspace_dir))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigResolutionSource {
    EnvConfigDir,
    EnvWorkspace,
    ActiveWorkspaceMarker,
    DefaultConfigDir,
}

impl ConfigResolutionSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::EnvConfigDir => "NARAECLAW_CONFIG_DIR",
            Self::EnvWorkspace => "NARAECLAW_WORKSPACE",
            Self::ActiveWorkspaceMarker => "active_workspace.toml",
            Self::DefaultConfigDir => "default",
        }
    }
}

/// Expand tilde in paths, falling back to `UserDirs` when HOME is unset.
///
/// In non-TTY environments (e.g. cron), HOME may not be set, causing
/// `shellexpand::tilde` to return the literal `~` unexpanded. This helper
/// detects that case and uses `directories::UserDirs` as a fallback.
fn expand_tilde_path(path: &str) -> PathBuf {
    let expanded = shellexpand::tilde(path);
    let expanded_str = expanded.as_ref();

    // If the path still starts with '~', tilde expansion failed (HOME unset)
    if expanded_str.starts_with('~') {
        if let Some(user_dirs) = UserDirs::new() {
            let home = user_dirs.home_dir();
            // Replace leading ~ with home directory
            if let Some(rest) = expanded_str.strip_prefix('~') {
                return home.join(rest.trim_start_matches(['/', '\\']));
            }
        }
        // If UserDirs also fails, log a warning and use the literal path
        tracing::warn!(
            path = path,
            "Failed to expand tilde: HOME environment variable is not set and UserDirs failed. \
             In cron/non-TTY environments, use absolute paths or set HOME explicitly."
        );
    }

    PathBuf::from(expanded_str)
}

async fn resolve_runtime_config_dirs(
    default_config_dir: &Path,
    default_workspace_dir: &Path,
) -> Result<(PathBuf, PathBuf, ConfigResolutionSource)> {
    if let Ok(custom_config_dir) =
        std::env::var("NARAECLAW_CONFIG_DIR").or_else(|_| app_env("CONFIG_DIR"))
    {
        let custom_config_dir = custom_config_dir.trim();
        if !custom_config_dir.is_empty() {
            let config_dir = expand_tilde_path(custom_config_dir);
            return Ok((
                config_dir.clone(),
                config_dir.join("workspace"),
                ConfigResolutionSource::EnvConfigDir,
            ));
        }
    }

    if let Ok(custom_workspace) =
        std::env::var("NARAECLAW_WORKSPACE").or_else(|_| app_env("WORKSPACE"))
        && !custom_workspace.is_empty()
    {
        let expanded = expand_tilde_path(&custom_workspace);
        let (config_dir, workspace_dir) = resolve_config_dir_for_workspace(&expanded);
        return Ok((
            config_dir,
            workspace_dir,
            ConfigResolutionSource::EnvWorkspace,
        ));
    }

    if let Some((config_dir, workspace_dir)) =
        load_persisted_workspace_dirs(default_config_dir).await?
    {
        return Ok((
            config_dir,
            workspace_dir,
            ConfigResolutionSource::ActiveWorkspaceMarker,
        ));
    }

    Ok((
        default_config_dir.to_path_buf(),
        default_workspace_dir.to_path_buf(),
        ConfigResolutionSource::DefaultConfigDir,
    ))
}

fn config_dir_creation_error(path: &Path) -> String {
    format!(
        "Failed to create config directory: {}. Ensure the configured path is writable by the \
         user running naraeclaw.",
        path.display()
    )
}

fn is_local_ollama_endpoint(api_url: Option<&str>) -> bool {
    let Some(raw) = api_url.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };

    reqwest::Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
        .is_some_and(|host| matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1" | "0.0.0.0"))
}

fn has_ollama_cloud_credential(config_api_key: Option<&str>) -> bool {
    let config_key_present = config_api_key
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if config_key_present {
        return true;
    }

    [
        "OLLAMA_API_KEY",
        "NARAECLAW_API_KEY",
        "ZEROCLAW_API_KEY",
        "API_KEY",
    ]
    .iter()
    .any(|name| {
        std::env::var(name)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
    })
}

/// Parse the `NARAECLAW_EXTRA_HEADERS`/`ZEROCLAW_EXTRA_HEADERS` environment variable value.
///
/// Format: `Key:Value,Key2:Value2`
///
/// Entries without a colon or with an empty key are silently skipped.
/// Leading/trailing whitespace on both key and value is trimmed.
pub fn parse_extra_headers_env(raw: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some((key, value)) = entry.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() {
                tracing::warn!(
                    "Ignoring extra header with empty name in NARAECLAW_EXTRA_HEADERS/ZEROCLAW_EXTRA_HEADERS"
                );
                continue;
            }
            result.push((key.to_string(), value.to_string()));
        } else {
            tracing::warn!("Ignoring malformed extra header entry (missing ':'): {entry}");
        }
    }
    result
}

fn normalize_wire_api(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "responses" | "openai-responses" | "open-ai-responses" => Some("responses"),
        "chat_completions"
        | "chat-completions"
        | "chat"
        | "chatcompletions"
        | "openai-chat-completions"
        | "open-ai-chat-completions" => Some("chat_completions"),
        _ => None,
    }
}

fn read_codex_openai_api_key() -> Option<String> {
    let home = UserDirs::new()?.home_dir().to_path_buf();
    let auth_path = home.join(".codex").join("auth.json");
    let raw = std::fs::read_to_string(auth_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;

    parsed
        .get("OPENAI_API_KEY")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// Ensure that essential bootstrap files exist in the workspace directory.
///
/// When the workspace is created outside of `naraeclaw onboard` (e.g., non-tty
/// daemon/cron sessions), these files would otherwise be missing. This function
/// creates sensible defaults that allow the agent to operate with a basic identity.
async fn ensure_bootstrap_files(workspace_dir: &Path) -> Result<()> {
    let defaults: &[(&str, &str)] = &[
        (
            "IDENTITY.md",
            "# IDENTITY.md — Who Am I?\n\n\
             I am NaraeClaw, an autonomous AI agent.\n\n\
             ## Traits\n\
             - Helpful, precise, and safety-conscious\n\
             - I prioritize clarity and correctness\n",
        ),
        (
            "SOUL.md",
            "# SOUL.md — Who You Are\n\n\
             You are NaraeClaw, an autonomous AI agent.\n\n\
             ## Core Principles\n\
             - Be helpful and accurate\n\
             - Respect user intent and boundaries\n\
             - Ask before taking destructive actions\n\
             - Prefer safe, reversible operations\n",
        ),
    ];

    for (filename, content) in defaults {
        let path = workspace_dir.join(filename);
        if !path.exists() {
            fs::write(&path, content)
                .await
                .with_context(|| format!("Failed to create default {filename} in workspace"))?;
        }
    }

    Ok(())
}

impl Config {
    /// Return top-level TOML keys in `raw_toml` that Config does not recognise.
    ///
    /// Keys present in `Config::default()` serialization pass immediately.
    /// Remaining keys are probed: the key is deserialized in isolation and
    /// the result compared to the default — a changed output means serde
    /// consumed it (covers `Option<T>` fields and `#[serde(alias)]` names).
    pub fn unknown_keys(raw_toml: &str) -> Vec<String> {
        let raw: toml::Table = match raw_toml.parse() {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        static DEFAULTS: OnceLock<toml::Table> = OnceLock::new();
        let defaults = DEFAULTS.get_or_init(|| {
            toml::to_string(&Config::default())
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_default()
        });
        raw.keys()
            .filter(|key| {
                if defaults.contains_key(key.as_str()) {
                    return false;
                }
                let mut t = toml::Table::new();
                t.insert((*key).clone(), raw[key.as_str()].clone());
                let consumed = toml::to_string(&t)
                    .ok()
                    .and_then(|s| toml::from_str::<Config>(&s).ok())
                    .and_then(|c| toml::to_string(&c).ok())
                    .and_then(|s| s.parse::<toml::Table>().ok())
                    .is_some_and(|t| t != *defaults);
                !consumed
            })
            .cloned()
            .collect()
    }

    pub async fn load_or_init() -> Result<Self> {
        let (default_config_dir, default_workspace_dir) = default_config_and_workspace_dirs()?;

        let (config_dir, workspace_dir, resolution_source) =
            resolve_runtime_config_dirs(&default_config_dir, &default_workspace_dir).await?;

        let config_path = config_dir.join("config.toml");

        fs::create_dir_all(&config_dir)
            .await
            .with_context(|| config_dir_creation_error(&config_dir))?;
        fs::create_dir_all(&workspace_dir)
            .await
            .context("Failed to create workspace directory")?;

        ensure_bootstrap_files(&workspace_dir).await?;

        if config_path.exists() {
            // Warn if config file is world-readable (may contain API keys)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(&config_path).await
                    && meta.permissions().mode() & 0o004 != 0
                {
                    tracing::warn!(
                        "Config file {:?} is world-readable (mode {:o}). \
                             Consider restricting with: chmod 600 {:?}",
                        config_path,
                        meta.permissions().mode() & 0o777,
                        config_path,
                    );
                }
            }

            let contents = fs::read_to_string(&config_path)
                .await
                .context("Failed to read config file")?;

            // Deserialize the config with the standard TOML parser.
            //
            // Previously this used `serde_ignored::deserialize` for both
            // deserialization and unknown-key detection.  However,
            // `serde_ignored` silently drops field values inside nested
            // structs that carry `#[serde(default)]` (e.g. the entire
            // `[autonomy]` table), causing user-supplied values to be
            // replaced by defaults.  See #4171.
            //
            // We now deserialize with `toml::from_str` (which is correct)
            // and run `serde_ignored` separately just for diagnostics.
            let mut config: Config =
                toml::from_str(&contents).context("Failed to deserialize config file")?;

            // Ensure the built-in default auto_approve entries are always
            // present.  When a user specifies `auto_approve` in their TOML
            // (e.g. to add a custom tool), serde replaces the default list
            // instead of merging.  This caused default-safe tools like
            // `weather` or `calculator` to lose their auto-approve status
            // and get silently denied in non-interactive channel runs.
            // See #4247.
            //
            // Users who want to require approval for a default tool can
            // add it to `always_ask`, which takes precedence over
            // `auto_approve` in the approval decision (see approval/mod.rs).
            config.autonomy.ensure_default_auto_approve();

            // Backward-compatible `enabled` backfill: if a channel section
            // exists in the TOML but has no explicit `enabled` key, the user
            // configured it before `enabled` was introduced — treat it as
            // enabled so existing setups don't silently break.
            config.channels_config.backfill_enabled(&contents);

            // Detect unknown top-level config keys by comparing the raw
            // TOML table keys against what Config actually deserializes.
            // This replaces the previous serde_ignored-based approach which
            // had false-positive issues with #[serde(default)] nested structs.
            for key in Self::unknown_keys(&contents) {
                tracing::warn!(
                    "Unknown config key ignored: \"{key}\". Check config.toml for typos or deprecated options.",
                );
            }
            // Set computed paths that are skipped during serialization
            config.config_path = config_path.clone();
            config.workspace_dir = workspace_dir;
            let store = crate::secrets::SecretStore::new(&config_dir, config.secrets.encrypt);
            // Decrypt all #[secret]-annotated fields via Configurable derive
            config.decrypt_secrets(&store)?;

            config.apply_env_overrides();
            config.validate()?;
            tracing::info!(
                path = %config.config_path.display(),
                workspace = %config.workspace_dir.display(),
                source = resolution_source.as_str(),
                initialized = true,
                "Config loaded"
            );
            Ok(config)
        } else {
            // Note: struct-update syntax (`..Config::default()`) is disallowed
            // for types that implement Drop (Rust E0509). Use field assignment instead.
            let mut config = Config::default();
            config.config_path = config_path.clone();
            config.workspace_dir = workspace_dir;
            config.save().await?;

            // Restrict permissions on newly created config file (may contain API keys)
            #[cfg(unix)]
            {
                use std::{fs::Permissions, os::unix::fs::PermissionsExt};
                let _ = fs::set_permissions(&config_path, Permissions::from_mode(0o600)).await;
            }

            config.apply_env_overrides();
            config.validate()?;
            tracing::info!(
                path = %config.config_path.display(),
                workspace = %config.workspace_dir.display(),
                source = resolution_source.as_str(),
                initialized = true,
                "Config loaded"
            );
            Ok(config)
        }
    }

    fn lookup_model_provider_profile(
        &self,
        provider_name: &str,
    ) -> Option<(String, ModelProviderConfig)> {
        let needle = provider_name.trim();
        if needle.is_empty() {
            return None;
        }

        self.model_providers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(needle))
            .map(|(name, profile)| (name.clone(), profile.clone()))
    }

    fn apply_named_model_provider_profile(&mut self) {
        let Some(current_provider) = self.default_provider.clone() else {
            return;
        };

        let Some((profile_key, profile)) = self.lookup_model_provider_profile(&current_provider)
        else {
            return;
        };

        let base_url = profile
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);

        if self
            .api_url
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty())
            && let Some(base_url) = base_url.as_ref()
        {
            self.api_url = Some(base_url.clone());
        }

        // Propagate api_path from the profile when not already set at top level.
        if self.api_path.is_none()
            && let Some(ref path) = profile.api_path
        {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                self.api_path = Some(trimmed.to_string());
            }
        }

        // Propagate max_tokens from the profile when not already set at top level.
        if self.provider_max_tokens.is_none()
            && let Some(max_tokens) = profile.max_tokens
        {
            self.provider_max_tokens = Some(max_tokens);
        }

        if profile.requires_openai_auth
            && self
                .api_key
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty())
        {
            let codex_key = std::env::var("OPENAI_API_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .or_else(read_codex_openai_api_key);
            if let Some(codex_key) = codex_key {
                self.api_key = Some(codex_key);
            }
        }

        let normalized_wire_api = profile.wire_api.as_deref().and_then(normalize_wire_api);
        let profile_name = profile
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        if normalized_wire_api == Some("responses") {
            self.default_provider = Some("openai-codex".to_string());
            return;
        }

        if let Some(profile_name) = profile_name
            && !profile_name.eq_ignore_ascii_case(&profile_key)
        {
            self.default_provider = Some(profile_name.to_string());
            return;
        }

        if let Some(base_url) = base_url {
            self.default_provider = Some(format!("custom:{base_url}"));
        }
    }

    /// Validate configuration values that would cause runtime failures.
    ///
    /// Called after TOML deserialization and env-override application to catch
    /// obviously invalid values early instead of failing at arbitrary runtime points.
    pub fn validate(&self) -> Result<()> {
        self.validate_tunnel_config()?;
        self.validate_gateway_config()?;
        self.validate_security_config()?;
        self.validate_routing_config()?;
        self.validate_integration_config()?;
        self.validate_agent_config()?;
        Ok(())
    }

    fn validate_tunnel_config(&self) -> Result<()> {
        if self.tunnel.provider.trim() == "openvpn" {
            let openvpn = self.tunnel.openvpn.as_ref().ok_or_else(|| {
                anyhow::anyhow!("tunnel.provider='openvpn' requires [tunnel.openvpn]")
            })?;
            if openvpn.config_file.trim().is_empty() {
                anyhow::bail!("tunnel.openvpn.config_file must not be empty");
            }
            if openvpn.connect_timeout_secs == 0 {
                anyhow::bail!("tunnel.openvpn.connect_timeout_secs must be greater than 0");
            }
        }

        if let Some(ref pinggy) = self.tunnel.pinggy
            && let Some(ref region) = pinggy.region
        {
            let r = region.trim().to_ascii_lowercase();
            if !r.is_empty() && !matches!(r.as_str(), "us" | "eu" | "ap" | "br" | "au") {
                anyhow::bail!(
                    "tunnel.pinggy.region must be one of: us, eu, ap, br, au (or omitted for auto)"
                );
            }
        }

        Ok(())
    }

    fn validate_gateway_config(&self) -> Result<()> {
        if self.gateway.host.trim().is_empty() {
            anyhow::bail!("gateway.host must not be empty");
        }
        if let Some(ref prefix) = self.gateway.path_prefix
            && !prefix.is_empty()
        {
            if !prefix.starts_with('/') {
                anyhow::bail!("gateway.path_prefix must start with '/'");
            }
            if prefix.ends_with('/') {
                anyhow::bail!("gateway.path_prefix must not end with '/' (including bare '/')");
            }
            // Reject characters unsafe for URL paths or HTML/JS injection.
            // Whitespace is intentionally excluded from the allowed set.
            if let Some(bad) = prefix.chars().find(|c| {
                !matches!(c, '/' | '-' | '_' | '.' | '~'
                        | 'a'..='z' | 'A'..='Z' | '0'..='9'
                        | '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '='
                        | ':' | '@')
            }) {
                anyhow::bail!(
                    "gateway.path_prefix contains invalid character '{bad}'; \
                         only unreserved and sub-delim URI characters are allowed"
                );
            }
        }
        Ok(())
    }

    fn validate_security_config(&self) -> Result<()> {
        if self.autonomy.max_actions_per_hour == 0 {
            anyhow::bail!("autonomy.max_actions_per_hour must be greater than 0");
        }
        for (i, env_name) in self.autonomy.shell_env_passthrough.iter().enumerate() {
            if !security::is_valid_env_var_name(env_name) {
                anyhow::bail!(
                    "autonomy.shell_env_passthrough[{i}] is invalid ({env_name}); expected [A-Za-z_][A-Za-z0-9_]*"
                );
            }
        }

        if self.security.otp.challenge_max_attempts == 0 {
            anyhow::bail!("security.otp.challenge_max_attempts must be greater than 0");
        }
        if self.security.otp.token_ttl_secs == 0 {
            anyhow::bail!("security.otp.token_ttl_secs must be greater than 0");
        }
        if self.security.otp.cache_valid_secs == 0 {
            anyhow::bail!("security.otp.cache_valid_secs must be greater than 0");
        }
        if self.security.otp.cache_valid_secs < self.security.otp.token_ttl_secs {
            anyhow::bail!(
                "security.otp.cache_valid_secs must be greater than or equal to security.otp.token_ttl_secs"
            );
        }
        for (i, action) in self.security.otp.gated_actions.iter().enumerate() {
            let normalized = action.trim();
            if normalized.is_empty() {
                anyhow::bail!("security.otp.gated_actions[{i}] must not be empty");
            }
            if !normalized
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                anyhow::bail!(
                    "security.otp.gated_actions[{i}] contains invalid characters: {normalized}"
                );
            }
        }
        DomainMatcher::new(
            &self.security.otp.gated_domains,
            &self.security.otp.gated_domain_categories,
        )
        .with_context(
            || "Invalid security.otp.gated_domains or security.otp.gated_domain_categories",
        )?;
        if self.security.estop.state_file.trim().is_empty() {
            anyhow::bail!("security.estop.state_file must not be empty");
        }

        if let Err(msg) = self.security.nevis.validate() {
            anyhow::bail!("security.nevis: {msg}");
        }

        Ok(())
    }

    fn validate_routing_config(&self) -> Result<()> {
        if self.scheduler.max_concurrent == 0 {
            anyhow::bail!("scheduler.max_concurrent must be greater than 0");
        }
        if self.scheduler.max_tasks == 0 {
            anyhow::bail!("scheduler.max_tasks must be greater than 0");
        }

        for (i, route) in self.model_routes.iter().enumerate() {
            if route.hint.trim().is_empty() {
                anyhow::bail!("model_routes[{i}].hint must not be empty");
            }
            if route.provider.trim().is_empty() {
                anyhow::bail!("model_routes[{i}].provider must not be empty");
            }
            if route.model.trim().is_empty() {
                anyhow::bail!("model_routes[{i}].model must not be empty");
            }
        }

        for (i, route) in self.embedding_routes.iter().enumerate() {
            if route.hint.trim().is_empty() {
                anyhow::bail!("embedding_routes[{i}].hint must not be empty");
            }
            if route.provider.trim().is_empty() {
                anyhow::bail!("embedding_routes[{i}].provider must not be empty");
            }
            if route.model.trim().is_empty() {
                anyhow::bail!("embedding_routes[{i}].model must not be empty");
            }
        }

        for (profile_key, profile) in &self.model_providers {
            let profile_name = profile_key.trim();
            if profile_name.is_empty() {
                anyhow::bail!("model_providers contains an empty profile name");
            }
            let has_name = profile
                .name
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            let has_base_url = profile
                .base_url
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            if !has_name && !has_base_url {
                anyhow::bail!(
                    "model_providers.{profile_name} must define at least one of `name` or `base_url`"
                );
            }
            if let Some(base_url) = profile.base_url.as_deref().map(str::trim)
                && !base_url.is_empty()
            {
                let parsed = reqwest::Url::parse(base_url).with_context(|| {
                    format!("model_providers.{profile_name}.base_url is not a valid URL")
                })?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    anyhow::bail!("model_providers.{profile_name}.base_url must use http/https");
                }
            }
            if let Some(wire_api) = profile.wire_api.as_deref().map(str::trim)
                && !wire_api.is_empty()
                && normalize_wire_api(wire_api).is_none()
            {
                anyhow::bail!(
                    "model_providers.{profile_name}.wire_api must be one of: responses, chat_completions"
                );
            }
        }

        if self
            .default_provider
            .as_deref()
            .is_some_and(|provider| provider.trim().eq_ignore_ascii_case("ollama"))
            && self
                .default_model
                .as_deref()
                .is_some_and(|model| model.trim().ends_with(":cloud"))
        {
            if is_local_ollama_endpoint(self.api_url.as_deref()) {
                anyhow::bail!(
                    "default_model uses ':cloud' with provider 'ollama', but api_url is local or unset. Set api_url to a remote Ollama endpoint (for example https://ollama.com)."
                );
            }
            if !has_ollama_cloud_credential(self.api_key.as_deref()) {
                anyhow::bail!(
                    "default_model uses ':cloud' with provider 'ollama', but no API key is configured. Set api_key or OLLAMA_API_KEY."
                );
            }
        }

        Ok(())
    }

    fn validate_integration_config(&self) -> Result<()> {
        if self.mcp.enabled {
            tools::validate_mcp_config(&self.mcp)?;
        }

        if self.knowledge.enabled {
            if self.knowledge.max_nodes == 0 {
                anyhow::bail!("knowledge.max_nodes must be greater than 0");
            }
            if self.knowledge.db_path.trim().is_empty() {
                anyhow::bail!("knowledge.db_path must not be empty");
            }
        }

        self.validate_google_workspace_config()?;

        self.proxy.validate()?;
        self.cloud_ops.validate()?;

        if self.jira.enabled {
            if self.jira.base_url.trim().is_empty() {
                anyhow::bail!("jira.base_url must not be empty when jira.enabled = true");
            }
            if self.jira.email.trim().is_empty() {
                anyhow::bail!("jira.email must not be empty when jira.enabled = true");
            }
            if self.jira.api_token.trim().is_empty()
                && std::env::var("JIRA_API_TOKEN")
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
            {
                anyhow::bail!(
                    "jira.api_token must be set (or JIRA_API_TOKEN env var) when jira.enabled = true"
                );
            }
            let valid_actions = ["get_ticket", "search_tickets", "comment_ticket"];
            for action in &self.jira.allowed_actions {
                if !valid_actions.contains(&action.as_str()) {
                    anyhow::bail!(
                        "jira.allowed_actions contains unknown action: '{}'. \
                         Valid: get_ticket, search_tickets, comment_ticket",
                        action
                    );
                }
            }
        }

        Ok(())
    }

    fn validate_google_workspace_config(&self) -> Result<()> {
        let mut seen_services = std::collections::HashSet::new();
        for (i, service) in self.google_workspace.allowed_services.iter().enumerate() {
            let normalized = service.trim();
            if normalized.is_empty() {
                anyhow::bail!("google_workspace.allowed_services[{i}] must not be empty");
            }
            if !normalized
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
            {
                anyhow::bail!(
                    "google_workspace.allowed_services[{i}] contains invalid characters: {normalized}"
                );
            }
            if !seen_services.insert(normalized.to_string()) {
                anyhow::bail!(
                    "google_workspace.allowed_services contains duplicate entry: {normalized}"
                );
            }
        }

        // When allowed_services is empty the tool falls back to DEFAULT_GWS_SERVICES;
        // use the same set here so validation is consistent at runtime.
        let effective_services: std::collections::HashSet<&str> =
            if self.google_workspace.allowed_services.is_empty() {
                DEFAULT_GWS_SERVICES.iter().copied().collect()
            } else {
                self.google_workspace
                    .allowed_services
                    .iter()
                    .map(|s| s.trim())
                    .collect()
            };

        let mut seen_operations = std::collections::HashSet::new();
        for (i, op) in self.google_workspace.allowed_operations.iter().enumerate() {
            let service = op.service.trim();
            let resource = op.resource.trim();

            if service.is_empty() {
                anyhow::bail!("google_workspace.allowed_operations[{i}].service must not be empty");
            }
            if resource.is_empty() {
                anyhow::bail!(
                    "google_workspace.allowed_operations[{i}].resource must not be empty"
                );
            }
            if !effective_services.contains(service) {
                anyhow::bail!(
                    "google_workspace.allowed_operations[{i}].service '{service}' is not in the \
                     effective allowed_services; this entry can never match at runtime"
                );
            }

            for (field, value) in [("service", service), ("resource", resource)] {
                if !value
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
                {
                    anyhow::bail!(
                        "google_workspace.allowed_operations[{i}].{field} contains invalid characters: {value}"
                    );
                }
            }

            if let Some(ref sub_resource) = op.sub_resource {
                let sub = sub_resource.trim();
                if sub.is_empty() {
                    anyhow::bail!(
                        "google_workspace.allowed_operations[{i}].sub_resource must not be empty when present"
                    );
                }
                if !sub
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
                {
                    anyhow::bail!(
                        "google_workspace.allowed_operations[{i}].sub_resource contains invalid characters: {sub}"
                    );
                }
            }

            if op.methods.is_empty() {
                anyhow::bail!("google_workspace.allowed_operations[{i}].methods must not be empty");
            }
            let mut seen_methods = std::collections::HashSet::new();
            for (j, method) in op.methods.iter().enumerate() {
                let normalized = method.trim();
                if normalized.is_empty() {
                    anyhow::bail!(
                        "google_workspace.allowed_operations[{i}].methods[{j}] must not be empty"
                    );
                }
                if !normalized
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
                {
                    anyhow::bail!(
                        "google_workspace.allowed_operations[{i}].methods[{j}] contains invalid characters: {normalized}"
                    );
                }
                if !seen_methods.insert(normalized.to_string()) {
                    anyhow::bail!(
                        "google_workspace.allowed_operations[{i}].methods contains duplicate entry: {normalized}"
                    );
                }
            }

            let sub_key = op.sub_resource.as_deref().map(str::trim).unwrap_or("");
            let operation_key = format!("{service}:{resource}:{sub_key}");
            if !seen_operations.insert(operation_key.clone()) {
                anyhow::bail!(
                    "google_workspace.allowed_operations contains duplicate service/resource/sub_resource entry: {operation_key}"
                );
            }
        }

        Ok(())
    }

    fn validate_agent_config(&self) -> Result<()> {
        const MAX_TIMEOUT_SECS: u64 = 3600;

        for (name, agent) in &self.agents {
            if let Some(timeout) = agent.timeout_secs {
                if timeout == 0 {
                    anyhow::bail!("agents.{name}.timeout_secs must be greater than 0");
                }
                if timeout > MAX_TIMEOUT_SECS {
                    anyhow::bail!("agents.{name}.timeout_secs exceeds max {MAX_TIMEOUT_SECS}");
                }
            }
            if let Some(timeout) = agent.agentic_timeout_secs {
                if timeout == 0 {
                    anyhow::bail!("agents.{name}.agentic_timeout_secs must be greater than 0");
                }
                if timeout > MAX_TIMEOUT_SECS {
                    anyhow::bail!(
                        "agents.{name}.agentic_timeout_secs exceeds max {MAX_TIMEOUT_SECS}"
                    );
                }
            }
        }

        let dp = self.transcription.default_provider.trim();
        match dp {
            "groq" | "openai" | "deepgram" | "assemblyai" | "google" | "local_whisper" => {}
            other => {
                anyhow::bail!(
                    "transcription.default_provider must be one of: groq, openai, deepgram, assemblyai, google, local_whisper (got '{other}')"
                );
            }
        }

        if self.delegate.timeout_secs == 0 {
            anyhow::bail!("delegate.timeout_secs must be greater than 0");
        }
        if self.delegate.agentic_timeout_secs == 0 {
            anyhow::bail!("delegate.agentic_timeout_secs must be greater than 0");
        }

        Ok(())
    }

    /// Apply environment variable overrides to config
    pub fn apply_env_overrides(&mut self) {
        // API Key: NARAECLAW_API_KEY, ZEROCLAW_API_KEY, or API_KEY (generic)
        if let Ok(key) = app_env("API_KEY").or_else(|_| std::env::var("API_KEY"))
            && !key.is_empty()
        {
            self.api_key = Some(key);
        }
        // API Key: GLM_API_KEY overrides when provider is a GLM/Zhipu variant.
        if self.default_provider.as_deref().is_some_and(is_glm_alias)
            && let Ok(key) = std::env::var("GLM_API_KEY")
            && !key.is_empty()
        {
            self.api_key = Some(key);
        }

        // API Key: ZAI_API_KEY overrides when provider is a Z.AI variant.
        if self.default_provider.as_deref().is_some_and(is_zai_alias)
            && let Ok(key) = std::env::var("ZAI_API_KEY")
            && !key.is_empty()
        {
            self.api_key = Some(key);
        }

        // Provider override precedence:
        // 1) NARAECLAW_PROVIDER always wins when set; ZEROCLAW_PROVIDER remains a fallback.
        // 2) NARAECLAW_MODEL_PROVIDER/ZEROCLAW_MODEL_PROVIDER/MODEL_PROVIDER (Codex app-server style).
        // 3) Legacy PROVIDER is honored only when config still uses default provider.
        if let Ok(provider) = app_env("PROVIDER") {
            if !provider.is_empty() {
                self.default_provider = Some(provider);
            }
        } else if let Ok(provider) =
            app_env("MODEL_PROVIDER").or_else(|_| std::env::var("MODEL_PROVIDER"))
        {
            if !provider.is_empty() {
                self.default_provider = Some(provider);
            }
        } else if let Ok(provider) = std::env::var("PROVIDER") {
            let should_apply_legacy_provider = self
                .default_provider
                .as_deref()
                .is_none_or(|configured| configured.trim().eq_ignore_ascii_case("openrouter"));
            if should_apply_legacy_provider && !provider.is_empty() {
                self.default_provider = Some(provider);
            }
        }

        // Model: NARAECLAW_MODEL, ZEROCLAW_MODEL, or MODEL
        if let Ok(model) = app_env("MODEL").or_else(|_| std::env::var("MODEL"))
            && !model.is_empty()
        {
            self.default_model = Some(model);
        }

        // Provider HTTP timeout: NARAECLAW_PROVIDER_TIMEOUT_SECS or ZEROCLAW_PROVIDER_TIMEOUT_SECS
        if let Ok(timeout_secs) = app_env("PROVIDER_TIMEOUT_SECS")
            && let Ok(timeout_secs) = timeout_secs.parse::<u64>()
            && timeout_secs > 0
        {
            self.provider_timeout_secs = timeout_secs;
        }

        // Extra provider headers: NARAECLAW_EXTRA_HEADERS or ZEROCLAW_EXTRA_HEADERS
        // Format: "Key:Value,Key2:Value2"
        // Env var headers override config file headers with the same name.
        if let Ok(raw) = app_env("EXTRA_HEADERS") {
            for header in parse_extra_headers_env(&raw) {
                self.extra_headers.insert(header.0, header.1);
            }
        }

        // Apply named provider profile remapping (Codex app-server compatibility).
        self.apply_named_model_provider_profile();

        // Workspace directory: NARAECLAW_WORKSPACE or ZEROCLAW_WORKSPACE
        if let Ok(workspace) = app_env("WORKSPACE")
            && !workspace.is_empty()
        {
            let expanded = expand_tilde_path(&workspace);
            let (_, workspace_dir) = resolve_config_dir_for_workspace(&expanded);
            self.workspace_dir = workspace_dir;
        }

        // Open-skills opt-in flag: NARAECLAW_OPEN_SKILLS_ENABLED or ZEROCLAW_OPEN_SKILLS_ENABLED
        if let Ok(flag) = app_env("OPEN_SKILLS_ENABLED")
            && !flag.trim().is_empty()
        {
            match flag.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => self.skills.open_skills_enabled = true,
                "0" | "false" | "no" | "off" => self.skills.open_skills_enabled = false,
                _ => tracing::warn!(
                    "Ignoring invalid NARAECLAW_OPEN_SKILLS_ENABLED/ZEROCLAW_OPEN_SKILLS_ENABLED (valid: 1|0|true|false|yes|no|on|off)"
                ),
            }
        }

        // Open-skills directory override: NARAECLAW_OPEN_SKILLS_DIR or ZEROCLAW_OPEN_SKILLS_DIR
        if let Ok(path) = app_env("OPEN_SKILLS_DIR") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                self.skills.open_skills_dir = Some(trimmed.to_string());
            }
        }

        // Skills script-file audit override: NARAECLAW_SKILLS_ALLOW_SCRIPTS or ZEROCLAW_SKILLS_ALLOW_SCRIPTS
        if let Ok(flag) = app_env("SKILLS_ALLOW_SCRIPTS")
            && !flag.trim().is_empty()
        {
            match flag.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => self.skills.allow_scripts = true,
                "0" | "false" | "no" | "off" => self.skills.allow_scripts = false,
                _ => tracing::warn!(
                    "Ignoring invalid NARAECLAW_SKILLS_ALLOW_SCRIPTS/ZEROCLAW_SKILLS_ALLOW_SCRIPTS (valid: 1|0|true|false|yes|no|on|off)"
                ),
            }
        }

        // Skills prompt mode override: NARAECLAW_SKILLS_PROMPT_MODE or ZEROCLAW_SKILLS_PROMPT_MODE
        if let Ok(mode) = app_env("SKILLS_PROMPT_MODE")
            && !mode.trim().is_empty()
        {
            if let Some(parsed) = parse_skills_prompt_injection_mode(&mode) {
                self.skills.prompt_injection_mode = parsed;
            } else {
                tracing::warn!(
                    "Ignoring invalid NARAECLAW_SKILLS_PROMPT_MODE/ZEROCLAW_SKILLS_PROMPT_MODE (valid: full|compact)"
                );
            }
        }

        // Gateway port: NARAECLAW_GATEWAY_PORT, ZEROCLAW_GATEWAY_PORT, or PORT
        if let Ok(port_str) = app_env("GATEWAY_PORT").or_else(|_| std::env::var("PORT"))
            && let Ok(port) = port_str.parse::<u16>()
        {
            self.gateway.port = port;
        }

        // Gateway host: NARAECLAW_GATEWAY_HOST, ZEROCLAW_GATEWAY_HOST, or HOST
        if let Ok(host) = app_env("GATEWAY_HOST").or_else(|_| std::env::var("HOST"))
            && !host.is_empty()
        {
            self.gateway.host = host;
        }

        // Allow public bind: NARAECLAW_ALLOW_PUBLIC_BIND or ZEROCLAW_ALLOW_PUBLIC_BIND
        if let Ok(val) = app_env("ALLOW_PUBLIC_BIND") {
            self.gateway.allow_public_bind = val == "1" || val.eq_ignore_ascii_case("true");
        }

        // Require pairing: NARAECLAW_REQUIRE_PAIRING or ZEROCLAW_REQUIRE_PAIRING
        if let Ok(val) = app_env("REQUIRE_PAIRING") {
            self.gateway.require_pairing = val == "1" || val.eq_ignore_ascii_case("true");
        }

        // Web dist dir: NARAECLAW_WEB_DIST_DIR or ZEROCLAW_WEB_DIST_DIR
        if let Ok(path) = app_env("WEB_DIST_DIR") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                self.gateway.web_dist_dir = Some(trimmed.to_string());
            }
        }

        // Temperature: NARAECLAW_TEMPERATURE or ZEROCLAW_TEMPERATURE
        if let Ok(temp_str) = app_env("TEMPERATURE") {
            match temp_str.parse::<f64>() {
                Ok(temp) if TEMPERATURE_RANGE.contains(&temp) => {
                    self.default_temperature = temp;
                }
                Ok(temp) => {
                    tracing::warn!(
                        "Ignoring NARAECLAW_TEMPERATURE/ZEROCLAW_TEMPERATURE={temp}: \
                         value out of range (expected {}..={})",
                        TEMPERATURE_RANGE.start(),
                        TEMPERATURE_RANGE.end()
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "Ignoring NARAECLAW_TEMPERATURE/ZEROCLAW_TEMPERATURE={temp_str:?}: not a valid number"
                    );
                }
            }
        }

        // Reasoning override: NARAECLAW_REASONING_ENABLED, ZEROCLAW_REASONING_ENABLED, or REASONING_ENABLED
        if let Ok(flag) =
            app_env("REASONING_ENABLED").or_else(|_| std::env::var("REASONING_ENABLED"))
        {
            let normalized = flag.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => self.runtime.reasoning_enabled = Some(true),
                "0" | "false" | "no" | "off" => self.runtime.reasoning_enabled = Some(false),
                _ => {}
            }
        }

        if let Ok(raw) = app_env("REASONING_EFFORT")
            .or_else(|_| std::env::var("REASONING_EFFORT"))
            .or_else(|_| app_env("CODEX_REASONING_EFFORT"))
        {
            match normalize_reasoning_effort(&raw) {
                Ok(effort) => self.runtime.reasoning_effort = Some(effort),
                Err(message) => tracing::warn!("Ignoring reasoning effort env override: {message}"),
            }
        }

        // Web search enabled: NARAECLAW_WEB_SEARCH_ENABLED, ZEROCLAW_WEB_SEARCH_ENABLED, or WEB_SEARCH_ENABLED
        if let Ok(enabled) =
            app_env("WEB_SEARCH_ENABLED").or_else(|_| std::env::var("WEB_SEARCH_ENABLED"))
        {
            self.web_search.enabled = enabled == "1" || enabled.eq_ignore_ascii_case("true");
        }

        // Web search provider: NARAECLAW_WEB_SEARCH_PROVIDER, ZEROCLAW_WEB_SEARCH_PROVIDER, or WEB_SEARCH_PROVIDER
        if let Ok(provider) =
            app_env("WEB_SEARCH_PROVIDER").or_else(|_| std::env::var("WEB_SEARCH_PROVIDER"))
        {
            let provider = provider.trim();
            if !provider.is_empty() {
                self.web_search.provider = provider.to_string();
            }
        }

        // Brave API key: NARAECLAW_BRAVE_API_KEY, ZEROCLAW_BRAVE_API_KEY, or BRAVE_API_KEY
        if let Ok(api_key) = app_env("BRAVE_API_KEY").or_else(|_| std::env::var("BRAVE_API_KEY")) {
            let api_key = api_key.trim();
            if !api_key.is_empty() {
                self.web_search.brave_api_key = Some(api_key.to_string());
            }
        }

        // SearXNG instance URL: NARAECLAW_SEARXNG_INSTANCE_URL, ZEROCLAW_SEARXNG_INSTANCE_URL, or SEARXNG_INSTANCE_URL
        if let Ok(instance_url) =
            app_env("SEARXNG_INSTANCE_URL").or_else(|_| std::env::var("SEARXNG_INSTANCE_URL"))
        {
            let instance_url = instance_url.trim();
            if !instance_url.is_empty() {
                self.web_search.searxng_instance_url = Some(instance_url.to_string());
            }
        }

        // Web search max results: NARAECLAW_WEB_SEARCH_MAX_RESULTS, ZEROCLAW_WEB_SEARCH_MAX_RESULTS, or WEB_SEARCH_MAX_RESULTS
        if let Ok(max_results) =
            app_env("WEB_SEARCH_MAX_RESULTS").or_else(|_| std::env::var("WEB_SEARCH_MAX_RESULTS"))
            && let Ok(max_results) = max_results.parse::<usize>()
            && (1..=10).contains(&max_results)
        {
            self.web_search.max_results = max_results;
        }

        // Web search timeout: NARAECLAW_WEB_SEARCH_TIMEOUT_SECS, ZEROCLAW_WEB_SEARCH_TIMEOUT_SECS, or WEB_SEARCH_TIMEOUT_SECS
        if let Ok(timeout_secs) =
            app_env("WEB_SEARCH_TIMEOUT_SECS").or_else(|_| std::env::var("WEB_SEARCH_TIMEOUT_SECS"))
            && let Ok(timeout_secs) = timeout_secs.parse::<u64>()
            && timeout_secs > 0
        {
            self.web_search.timeout_secs = timeout_secs;
        }

        // Storage provider key (optional backend override): NARAECLAW_STORAGE_PROVIDER or ZEROCLAW_STORAGE_PROVIDER
        if let Ok(provider) = app_env("STORAGE_PROVIDER") {
            let provider = provider.trim();
            if !provider.is_empty() {
                self.storage.provider.config.provider = provider.to_string();
            }
        }

        // Storage connection URL (for remote backends): NARAECLAW_STORAGE_DB_URL or ZEROCLAW_STORAGE_DB_URL
        if let Ok(db_url) = app_env("STORAGE_DB_URL") {
            let db_url = db_url.trim();
            if !db_url.is_empty() {
                self.storage.provider.config.db_url = Some(db_url.to_string());
            }
        }

        // Storage connect timeout: NARAECLAW_STORAGE_CONNECT_TIMEOUT_SECS or ZEROCLAW_STORAGE_CONNECT_TIMEOUT_SECS
        if let Ok(timeout_secs) = app_env("STORAGE_CONNECT_TIMEOUT_SECS")
            && let Ok(timeout_secs) = timeout_secs.parse::<u64>()
            && timeout_secs > 0
        {
            self.storage.provider.config.connect_timeout_secs = Some(timeout_secs);
        }
        // Proxy enabled flag: NARAECLAW_PROXY_ENABLED or ZEROCLAW_PROXY_ENABLED
        let explicit_proxy_enabled = app_env("PROXY_ENABLED")
            .ok()
            .as_deref()
            .and_then(tools::parse_proxy_enabled);
        if let Some(enabled) = explicit_proxy_enabled {
            self.proxy.enabled = enabled;
        }

        // Proxy URLs: NARAECLAW_* wins, then ZEROCLAW_* fallback, then generic *PROXY vars.
        let mut proxy_url_overridden = false;
        if let Ok(proxy_url) = app_env("HTTP_PROXY").or_else(|_| std::env::var("HTTP_PROXY")) {
            self.proxy.http_proxy = normalize_proxy_url_option(Some(&proxy_url));
            proxy_url_overridden = true;
        }
        if let Ok(proxy_url) = app_env("HTTPS_PROXY").or_else(|_| std::env::var("HTTPS_PROXY")) {
            self.proxy.https_proxy = normalize_proxy_url_option(Some(&proxy_url));
            proxy_url_overridden = true;
        }
        if let Ok(proxy_url) = app_env("ALL_PROXY").or_else(|_| std::env::var("ALL_PROXY")) {
            self.proxy.all_proxy = normalize_proxy_url_option(Some(&proxy_url));
            proxy_url_overridden = true;
        }
        if let Ok(no_proxy) = app_env("NO_PROXY").or_else(|_| std::env::var("NO_PROXY")) {
            self.proxy.no_proxy = normalize_no_proxy_list(vec![no_proxy]);
        }

        if explicit_proxy_enabled.is_none()
            && proxy_url_overridden
            && self.proxy.has_any_proxy_url()
        {
            self.proxy.enabled = true;
        }

        // Proxy scope and service selectors.
        if let Ok(scope_raw) = app_env("PROXY_SCOPE") {
            if let Some(scope) = tools::parse_proxy_scope(&scope_raw) {
                self.proxy.scope = scope;
            } else {
                tracing::warn!(
                    scope = %scope_raw,
                    "Ignoring invalid NARAECLAW_PROXY_SCOPE/ZEROCLAW_PROXY_SCOPE (valid: environment|naraeclaw|services)"
                );
            }
        }

        if let Ok(services_raw) = app_env("PROXY_SERVICES") {
            self.proxy.services = normalize_service_list(vec![services_raw]);
        }

        if let Err(error) = self.proxy.validate() {
            tracing::warn!("Invalid proxy configuration ignored: {error}");
            self.proxy.enabled = false;
        }

        if self.proxy.enabled && self.proxy.scope == ProxyScope::Environment {
            self.proxy.apply_to_process_env();
        }

        set_runtime_proxy_config(self.proxy.clone());
    }

    async fn resolve_config_path_for_save(&self) -> Result<PathBuf> {
        if self
            .config_path
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
        {
            return Ok(self.config_path.clone());
        }

        let (default_config_dir, default_workspace_dir) = default_config_and_workspace_dirs()?;
        let (config_dir, _workspace_dir, source) =
            resolve_runtime_config_dirs(&default_config_dir, &default_workspace_dir).await?;
        let file_name = self
            .config_path
            .file_name()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| std::ffi::OsStr::new("config.toml"));
        let resolved = config_dir.join(file_name);
        tracing::warn!(
            path = %self.config_path.display(),
            resolved = %resolved.display(),
            source = source.as_str(),
            "Config path missing parent directory; resolving from runtime environment"
        );
        Ok(resolved)
    }

    pub async fn save(&self) -> Result<()> {
        // Encrypt secrets before serialization
        let mut config_to_save = self.clone();
        let config_path = self.resolve_config_path_for_save().await?;
        let config_dir = config_path
            .parent()
            .context("Config path must have a parent directory")?;
        let store = crate::secrets::SecretStore::new(config_dir, self.secrets.encrypt);

        // Encrypt all #[secret]-annotated fields via Configurable derive
        config_to_save.encrypt_secrets(&store)?;

        let toml_str =
            toml::to_string_pretty(&config_to_save).context("Failed to serialize config")?;

        let parent_dir = config_path
            .parent()
            .context("Config path must have a parent directory")?;

        fs::create_dir_all(parent_dir).await.with_context(|| {
            format!(
                "Failed to create config directory: {}",
                parent_dir.display()
            )
        })?;

        let file_name = config_path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("config.toml");
        let temp_path = parent_dir.join(format!(".{file_name}.tmp-{}", uuid::Uuid::new_v4()));
        let backup_path = parent_dir.join(format!("{file_name}.bak"));

        let mut temp_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to create temporary config file: {}",
                    temp_path.display()
                )
            })?;
        temp_file
            .write_all(toml_str.as_bytes())
            .await
            .context("Failed to write temporary config contents")?;
        temp_file
            .sync_all()
            .await
            .context("Failed to fsync temporary config file")?;
        drop(temp_file);

        let had_existing_config = config_path.exists();
        if had_existing_config {
            fs::copy(&config_path, &backup_path)
                .await
                .with_context(|| {
                    format!(
                        "Failed to create config backup before atomic replace: {}",
                        backup_path.display()
                    )
                })?;
        }

        if let Err(e) = fs::rename(&temp_path, &config_path).await {
            let _ = fs::remove_file(&temp_path).await;
            if had_existing_config && backup_path.exists() {
                fs::copy(&backup_path, &config_path)
                    .await
                    .context("Failed to restore config backup")?;
            }
            anyhow::bail!("Failed to atomically replace config file: {e}");
        }

        #[cfg(unix)]
        {
            use std::{fs::Permissions, os::unix::fs::PermissionsExt};
            if let Err(err) = fs::set_permissions(&config_path, Permissions::from_mode(0o600)).await
            {
                tracing::warn!(
                    "Failed to harden config permissions to 0600 at {}: {}",
                    config_path.display(),
                    err
                );
            }
        }

        sync_directory(parent_dir).await?;

        if had_existing_config {
            let _ = fs::remove_file(&backup_path).await;
        }

        Ok(())
    }
}

#[allow(clippy::unused_async)] // async needed on unix for tokio File I/O; no-op on other platforms
async fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let dir = File::open(path)
            .await
            .with_context(|| format!("Failed to open directory for fsync: {}", path.display()))?;
        dir.sync_all()
            .await
            .with_context(|| format!("Failed to fsync directory metadata: {}", path.display()))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
        let dir = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .with_context(|| format!("Failed to open directory for fsync: {}", path.display()))?;
        // FlushFileBuffers on directory handles returns ERROR_ACCESS_DENIED on
        // Windows (OS Error 5). This is expected — NTFS does not support
        // flushing directory metadata the same way Unix does. The individual
        // files have already been synced, so it is safe to ignore this error.
        if let Err(e) = dir.sync_all() {
            if e.raw_os_error() == Some(5) {
                tracing::trace!(
                    "Ignoring expected ACCESS_DENIED when fsyncing directory on Windows: {}",
                    path.display()
                );
            } else {
                return Err(e).with_context(|| {
                    format!("Failed to fsync directory metadata: {}", path.display())
                });
            }
        }
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Ok(())
    }
}

// ── HasPropKind impls for config enums ──
// Scalars (bool, String, integers, floats) are covered by impl_prop_kind! in traits.rs.
// Config enums serialize as TOML strings and are classified as PropKind::Enum.
macro_rules! impl_enum_prop_kind {
    ($($ty:ty),+ $(,)?) => {
        $(impl HasPropKind for $ty { const PROP_KIND: PropKind = PropKind::Enum; })+
    };
}
impl_enum_prop_kind!(
    SwarmStrategy,
    McpTransport,
    ToolFilterGroupMode,
    SkillsPromptInjectionMode,
    FirecrawlMode,
    ProxyScope,
    SearchMode,
    CronScheduleDecl,
    OtpMethod,
    SandboxBackend,
    AutonomyLevel,
);

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]

    use super::*;
    use std::io;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex as StdMutex};
    use tempfile::TempDir;
    use tokio::sync::{Mutex, MutexGuard};
    use tokio::test;
    use tokio_stream::StreamExt;
    use tokio_stream::wrappers::ReadDirStream;

    // ── Tilde expansion ───────────────────────────────────────

    #[test]
    async fn expand_tilde_path_handles_absolute_path() {
        let path = expand_tilde_path("/absolute/path");
        assert_eq!(path, PathBuf::from("/absolute/path"));
    }

    #[test]
    async fn expand_tilde_path_handles_relative_path() {
        let path = expand_tilde_path("relative/path");
        assert_eq!(path, PathBuf::from("relative/path"));
    }

    #[test]
    async fn expand_tilde_path_expands_tilde_when_home_set() {
        // This test verifies that tilde expansion works when HOME is set.
        // In normal environments, HOME is set, so ~ should expand.
        let path = expand_tilde_path("~/.naraeclaw");
        // The path should not literally start with '~' if HOME is set
        // (it should be expanded to the actual home directory)
        if std::env::var("HOME").is_ok() {
            assert!(
                !path.to_string_lossy().starts_with('~'),
                "Tilde should be expanded when HOME is set"
            );
        }
    }

    // ── Defaults ─────────────────────────────────────────────

    fn has_test_table(raw: &str, table: &str) -> bool {
        let exact = format!("[{table}]");
        let nested = format!("[{table}.");
        raw.lines()
            .map(str::trim)
            .any(|line| line == exact || line.starts_with(&nested))
    }

    fn parse_test_config(raw: &str) -> Config {
        let mut merged = raw.trim().to_string();
        for table in ["data_retention", "cloud_ops", "security", "security_ops"] {
            if has_test_table(&merged, table) {
                continue;
            }
            if !merged.is_empty() {
                merged.push_str("\n\n");
            }
            merged.push('[');
            merged.push_str(table);
            merged.push(']');
        }
        merged.push('\n');
        let mut config: Config = toml::from_str(&merged).unwrap();
        config.autonomy.ensure_default_auto_approve();
        config
    }

    #[test]
    async fn http_request_config_default_has_correct_values() {
        let cfg = HttpRequestConfig::default();
        assert_eq!(cfg.timeout_secs, 30);
        assert_eq!(cfg.max_response_size, 1_000_000);
        assert!(cfg.enabled);
        assert_eq!(cfg.allowed_domains, vec!["*".to_string()]);
    }

    #[test]
    async fn config_default_has_sane_values() {
        let c = Config::default();
        assert_eq!(c.default_provider.as_deref(), Some("openrouter"));
        assert!(c.default_model.as_deref().unwrap().contains("claude"));
        assert!((c.default_temperature - 0.7).abs() < f64::EPSILON);
        assert!(c.api_key.is_none());
        assert!(!c.skills.open_skills_enabled);
        assert!(!c.skills.allow_scripts);
        assert_eq!(
            c.skills.prompt_injection_mode,
            SkillsPromptInjectionMode::Full
        );
        assert_eq!(c.provider_timeout_secs, 120);
        assert!(c.workspace_dir.to_string_lossy().contains("workspace"));
        assert!(c.config_path.to_string_lossy().contains("config.toml"));
    }

    #[derive(Clone, Default)]
    struct SharedLogBuffer(Arc<StdMutex<Vec<u8>>>);

    struct SharedLogWriter(Arc<StdMutex<Vec<u8>>>);

    impl SharedLogBuffer {
        fn captured(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogBuffer {
        type Writer = SharedLogWriter;

        fn make_writer(&'a self) -> Self::Writer {
            SharedLogWriter(self.0.clone())
        }
    }

    impl io::Write for SharedLogWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    async fn config_dir_creation_error_mentions_path() {
        let msg = config_dir_creation_error(Path::new("/etc/naraeclaw"));
        assert!(msg.contains("/etc/naraeclaw"));
        assert!(msg.contains("naraeclaw"));
    }

    #[test]
    async fn config_schema_export_contains_expected_contract_shape() {
        #[cfg(feature = "schema-export")]
        let schema = schemars::schema_for!(Config);
        let schema_json = serde_json::to_value(&schema).expect("schema should serialize to json");

        assert_eq!(
            schema_json
                .get("$schema")
                .and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema")
        );

        let properties = schema_json
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("schema should expose top-level properties");

        assert!(properties.contains_key("default_provider"));
        assert!(properties.contains_key("skills"));
        assert!(properties.contains_key("gateway"));
        assert!(properties.contains_key("channels_config"));
        assert!(!properties.contains_key("workspace_dir"));
        assert!(!properties.contains_key("config_path"));

        assert!(
            schema_json
                .get("$defs")
                .and_then(serde_json::Value::as_object)
                .is_some(),
            "schema should include reusable type definitions"
        );
    }

    #[cfg(unix)]
    #[test]
    async fn save_sets_config_permissions_on_new_file() {
        let temp = TempDir::new().expect("temp dir");
        let config_path = temp.path().join("config.toml");
        let workspace_dir = temp.path().join("workspace");

        let mut config = Config::default();
        config.config_path = config_path.clone();
        config.workspace_dir = workspace_dir;

        config.save().await.expect("save config");

        let mode = std::fs::metadata(&config_path)
            .expect("config metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    async fn observability_config_default() {
        let o = ObservabilityConfig::default();
        assert_eq!(o.backend, "none");
        assert_eq!(o.runtime_trace_mode, "none");
        assert_eq!(o.runtime_trace_path, "state/runtime-trace.jsonl");
        assert_eq!(o.runtime_trace_max_entries, 200);
    }

    #[test]
    async fn autonomy_config_default() {
        let a = AutonomyConfig::default();
        assert_eq!(a.level, AutonomyLevel::Supervised);
        assert!(a.workspace_only);
        assert!(a.allowed_commands.contains(&"git".to_string()));
        assert!(a.allowed_commands.contains(&"cargo".to_string()));
        assert!(a.forbidden_paths.contains(&"/etc".to_string()));
        assert_eq!(a.max_actions_per_hour, 100);
        assert_eq!(a.max_cost_per_day_cents, 500);
        assert!(a.require_approval_for_medium_risk);
        assert!(a.block_high_risk_commands);
        assert!(a.shell_env_passthrough.is_empty());
    }

    #[test]
    async fn runtime_config_default() {
        let r = RuntimeConfig::default();
        assert_eq!(r.kind, "native");
        assert_eq!(r.docker.image, "debian:bookworm-slim");
        assert_eq!(r.docker.network, "none");
        assert_eq!(r.docker.memory_limit_mb, Some(512));
        assert_eq!(r.docker.cpu_limit, Some(1.0));
        assert!(r.docker.read_only_rootfs);
        assert!(r.docker.mount_workspace);
    }

    #[test]
    async fn heartbeat_config_default() {
        let h = HeartbeatConfig::default();
        assert!(h.enabled);
        assert_eq!(h.interval_minutes, 30);
        assert!(h.message.is_none());
        assert!(h.target.is_none());
        assert!(h.to.is_none());
    }

    #[test]
    async fn heartbeat_config_parses_delivery_aliases() {
        let raw = r#"
enabled = true
interval_minutes = 10
message = "Ping"
channel = "telegram"
recipient = "42"
"#;
        let parsed: HeartbeatConfig = toml::from_str(raw).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.interval_minutes, 10);
        assert_eq!(parsed.message.as_deref(), Some("Ping"));
        assert_eq!(parsed.target.as_deref(), Some("telegram"));
        assert_eq!(parsed.to.as_deref(), Some("42"));
    }

    #[test]
    async fn cron_config_default() {
        let c = CronConfig::default();
        assert!(c.enabled);
        assert_eq!(c.max_run_history, 50);
    }

    #[test]
    async fn cron_config_serde_roundtrip() {
        let c = CronConfig {
            enabled: false,
            catch_up_on_startup: false,
            max_run_history: 100,
            jobs: Vec::new(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let parsed: CronConfig = serde_json::from_str(&json).unwrap();
        assert!(!parsed.enabled);
        assert!(!parsed.catch_up_on_startup);
        assert_eq!(parsed.max_run_history, 100);
    }

    #[test]
    async fn config_defaults_cron_when_section_missing() {
        let toml_str = r#"
workspace_dir = "/tmp/workspace"
config_path = "/tmp/config.toml"
default_temperature = 0.7
"#;

        let parsed = parse_test_config(toml_str);
        assert!(parsed.cron.enabled);
        assert!(parsed.cron.catch_up_on_startup);
        assert_eq!(parsed.cron.max_run_history, 50);
    }

    #[test]
    async fn memory_config_default_hygiene_settings() {
        let m = MemoryConfig::default();
        assert_eq!(m.backend, "sqlite");
        assert!(m.auto_save);
        assert!(m.hygiene_enabled);
        assert_eq!(m.archive_after_days, 7);
        assert_eq!(m.purge_after_days, 30);
        assert_eq!(m.conversation_retention_days, 30);
        assert!(m.sqlite_open_timeout_secs.is_none());
        assert_eq!(m.search_mode, SearchMode::Hybrid);
    }

    #[test]
    async fn search_mode_config_deserialization() {
        let toml_str = r#"
workspace_dir = "/tmp/workspace"
config_path = "/tmp/config.toml"
default_temperature = 0.7

[memory]
backend = "sqlite"
auto_save = true
search_mode = "bm25"
"#;
        let parsed = parse_test_config(toml_str);
        assert_eq!(parsed.memory.search_mode, SearchMode::Bm25);

        let toml_str_embedding = r#"
workspace_dir = "/tmp/workspace"
config_path = "/tmp/config.toml"
default_temperature = 0.7

[memory]
backend = "sqlite"
auto_save = true
search_mode = "embedding"
"#;
        let parsed = parse_test_config(toml_str_embedding);
        assert_eq!(parsed.memory.search_mode, SearchMode::Embedding);

        let toml_str_hybrid = r#"
workspace_dir = "/tmp/workspace"
config_path = "/tmp/config.toml"
default_temperature = 0.7

[memory]
backend = "sqlite"
auto_save = true
search_mode = "hybrid"
"#;
        let parsed = parse_test_config(toml_str_hybrid);
        assert_eq!(parsed.memory.search_mode, SearchMode::Hybrid);
    }

    #[test]
    async fn search_mode_defaults_to_hybrid_when_omitted() {
        let toml_str = r#"
workspace_dir = "/tmp/workspace"
config_path = "/tmp/config.toml"
default_temperature = 0.7

[memory]
backend = "sqlite"
auto_save = true
"#;
        let parsed = parse_test_config(toml_str);
        assert_eq!(parsed.memory.search_mode, SearchMode::Hybrid);
    }

    #[test]
    async fn search_mode_serde_roundtrip() {
        let json_bm25 = serde_json::to_string(&SearchMode::Bm25).unwrap();
        assert_eq!(json_bm25, "\"bm25\"");
        let parsed: SearchMode = serde_json::from_str(&json_bm25).unwrap();
        assert_eq!(parsed, SearchMode::Bm25);

        let json_embedding = serde_json::to_string(&SearchMode::Embedding).unwrap();
        assert_eq!(json_embedding, "\"embedding\"");
        let parsed: SearchMode = serde_json::from_str(&json_embedding).unwrap();
        assert_eq!(parsed, SearchMode::Embedding);

        let json_hybrid = serde_json::to_string(&SearchMode::Hybrid).unwrap();
        assert_eq!(json_hybrid, "\"hybrid\"");
        let parsed: SearchMode = serde_json::from_str(&json_hybrid).unwrap();
        assert_eq!(parsed, SearchMode::Hybrid);
    }

    #[test]
    async fn storage_provider_config_defaults() {
        let storage = StorageConfig::default();
        assert!(storage.provider.config.provider.is_empty());
        assert!(storage.provider.config.db_url.is_none());
        assert_eq!(storage.provider.config.schema, "public");
        assert_eq!(storage.provider.config.table, "memories");
        assert!(storage.provider.config.connect_timeout_secs.is_none());
    }

    #[test]
    async fn channels_config_default() {
        let c = ChannelsConfig::default();
        assert!(c.cli);
        assert!(!c.show_tool_calls);
    }

    // ── Serde round-trip ─────────────────────────────────────

    #[test]
    async fn config_toml_roundtrip() {
        let config = Config {
            workspace_dir: PathBuf::from("/tmp/test/workspace"),
            config_path: PathBuf::from("/tmp/test/config.toml"),
            api_key: Some("sk-test-key".into()),
            api_url: None,
            api_path: None,
            default_provider: Some("openrouter".into()),
            default_model: Some("gpt-4o".into()),
            model_providers: HashMap::new(),
            default_temperature: 0.5,
            provider_timeout_secs: 120,
            provider_max_tokens: None,
            extra_headers: HashMap::new(),
            observability: ObservabilityConfig {
                backend: "log".into(),
                ..ObservabilityConfig::default()
            },
            autonomy: AutonomyConfig {
                level: AutonomyLevel::Full,
                workspace_only: false,
                allowed_commands: vec!["docker".into()],
                forbidden_paths: vec!["/secret".into()],
                max_actions_per_hour: 50,
                max_cost_per_day_cents: 1000,
                require_approval_for_medium_risk: false,
                block_high_risk_commands: true,
                shell_env_passthrough: vec!["DATABASE_URL".into()],
                auto_approve: vec!["file_read".into()],
                always_ask: vec![],
                allowed_roots: vec![],
                non_cli_excluded_tools: vec![],
                shell_timeout_secs: default_shell_timeout_secs(),
            },
            trust: crate::scattered_types::TrustConfig::default(),
            backup: BackupConfig::default(),
            data_retention: DataRetentionConfig::default(),
            cloud_ops: CloudOpsConfig::default(),
            security: SecurityConfig::default(),
            security_ops: SecurityOpsConfig::default(),
            runtime: RuntimeConfig {
                kind: "docker".into(),
                ..RuntimeConfig::default()
            },
            reliability: ReliabilityConfig::default(),
            scheduler: SchedulerConfig::default(),
            skills: SkillsConfig::default(),
            pipeline: PipelineConfig::default(),
            model_routes: Vec::new(),
            embedding_routes: Vec::new(),
            query_classification: QueryClassificationConfig::default(),
            heartbeat: HeartbeatConfig {
                enabled: true,
                interval_minutes: 15,
                two_phase: true,
                message: Some("Check London time".into()),
                target: Some("telegram".into()),
                to: Some("123456".into()),
                ..HeartbeatConfig::default()
            },
            cron: CronConfig::default(),
            skillforge: SkillForgeConfig::default(),
            channels_config: ChannelsConfig {
                cli: true,
                slack: None,
                message_timeout_secs: 300,
                ack_reactions: true,
                show_tool_calls: true,
                session_persistence: true,
                session_backend: default_session_backend(),
                session_ttl_hours: 0,
                debounce_ms: 0,
            },
            memory: MemoryConfig::default(),
            storage: StorageConfig::default(),
            tunnel: TunnelConfig::default(),
            gateway: GatewayConfig::default(),
            secrets: SecretsConfig::default(),
            browser: BrowserConfig::default(),
            browser_delegate: crate::scattered_types::BrowserDelegateConfig::default(),
            http_request: HttpRequestConfig::default(),
            multimodal: MultimodalConfig::default(),
            media_pipeline: MediaPipelineConfig::default(),
            web_fetch: WebFetchConfig::default(),
            link_enricher: LinkEnricherConfig::default(),
            text_browser: TextBrowserConfig::default(),
            web_search: WebSearchConfig::default(),
            google_workspace: GoogleWorkspaceConfig::default(),
            proxy: ProxyConfig::default(),
            agent: AgentConfig::default(),
            pacing: PacingConfig::default(),
            identity: IdentityConfig::default(),
            cost: CostConfig::default(),
            delegate: DelegateToolConfig::default(),
            agents: HashMap::new(),
            swarms: HashMap::new(),
            hooks: HooksConfig::default(),
            transcription: TranscriptionConfig::default(),
            tts: TtsConfig::default(),
            mcp: McpConfig::default(),
            nodes: NodesConfig::default(),
            workspace: WorkspaceConfig::default(),
            jira: JiraConfig::default(),
            redash: RedashConfig::default(),
            node_transport: NodeTransportConfig::default(),
            knowledge: KnowledgeConfig::default(),
            linkedin: LinkedInConfig::default(),
            image_gen: ImageGenConfig::default(),
            plugins: PluginsConfig::default(),
            locale: None,
            verifiable_intent: VerifiableIntentConfig::default(),
            claude_code: ClaudeCodeConfig::default(),
            claude_code_runner: ClaudeCodeRunnerConfig::default(),
            codex_cli: CodexCliConfig::default(),
            gemini_cli: GeminiCliConfig::default(),
            opencode_cli: OpenCodeCliConfig::default(),
            sop: SopConfig::default(),
            shell_tool: ShellToolConfig::default(),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed = parse_test_config(&toml_str);

        assert_eq!(parsed.api_key, config.api_key);
        assert_eq!(parsed.default_provider, config.default_provider);
        assert_eq!(parsed.default_model, config.default_model);
        assert!((parsed.default_temperature - config.default_temperature).abs() < f64::EPSILON);
        assert_eq!(parsed.observability.backend, "log");
        assert_eq!(parsed.observability.runtime_trace_mode, "none");
        assert_eq!(parsed.autonomy.level, AutonomyLevel::Full);
        assert!(!parsed.autonomy.workspace_only);
        assert_eq!(parsed.runtime.kind, "docker");
        assert!(parsed.heartbeat.enabled);
        assert_eq!(parsed.heartbeat.interval_minutes, 15);
        assert_eq!(
            parsed.heartbeat.message.as_deref(),
            Some("Check London time")
        );
        assert_eq!(parsed.heartbeat.target.as_deref(), Some("telegram"));
        assert_eq!(parsed.heartbeat.to.as_deref(), Some("123456"));
        assert!(parsed.channels_config.slack.is_none());
    }

    #[test]
    async fn config_minimal_toml_uses_defaults() {
        let minimal = r#"
workspace_dir = "/tmp/ws"
config_path = "/tmp/config.toml"
default_temperature = 0.7
"#;
        let parsed = parse_test_config(minimal);
        assert!(parsed.api_key.is_none());
        assert!(parsed.default_provider.is_none());
        assert_eq!(parsed.observability.backend, "none");
        assert_eq!(parsed.observability.runtime_trace_mode, "none");
        assert_eq!(parsed.autonomy.level, AutonomyLevel::Supervised);
        assert_eq!(parsed.runtime.kind, "native");
        assert!(parsed.heartbeat.enabled);
        assert!(parsed.channels_config.cli);
        assert!(parsed.memory.hygiene_enabled);
        assert_eq!(parsed.memory.archive_after_days, 7);
        assert_eq!(parsed.memory.purge_after_days, 30);
        assert_eq!(parsed.memory.conversation_retention_days, 30);
        // provider_timeout_secs defaults to 120 when not specified
        assert_eq!(parsed.provider_timeout_secs, 120);
    }

    /// Regression test for #4171: the `[autonomy]` section must not be
    /// silently dropped when parsing config TOML.
    #[test]
    async fn autonomy_section_is_not_silently_ignored() {
        let raw = r#"
default_temperature = 0.7

[autonomy]
level = "full"
max_actions_per_hour = 99
auto_approve = ["file_read", "memory_recall", "http_request"]
"#;
        let parsed = parse_test_config(raw);
        assert_eq!(
            parsed.autonomy.level,
            AutonomyLevel::Full,
            "autonomy.level must be parsed from config (was silently defaulting to Supervised)"
        );
        assert_eq!(
            parsed.autonomy.max_actions_per_hour, 99,
            "autonomy.max_actions_per_hour must be parsed from config"
        );
        assert!(
            parsed
                .autonomy
                .auto_approve
                .contains(&"http_request".to_string()),
            "autonomy.auto_approve must include http_request from config"
        );
    }

    /// Regression test for #4247: when a user provides a custom auto_approve
    /// list, the built-in defaults must still be present.
    #[test]
    async fn auto_approve_merges_user_entries_with_defaults() {
        let raw = r#"
default_temperature = 0.7

[autonomy]
auto_approve = ["my_custom_tool", "another_tool"]
"#;
        let parsed = parse_test_config(raw);
        // User entries are preserved
        assert!(
            parsed
                .autonomy
                .auto_approve
                .contains(&"my_custom_tool".to_string()),
            "user-supplied tool must remain in auto_approve"
        );
        assert!(
            parsed
                .autonomy
                .auto_approve
                .contains(&"another_tool".to_string()),
            "user-supplied tool must remain in auto_approve"
        );
        // Defaults are merged in
        for default_tool in &[
            "file_read",
            "memory_recall",
            "weather",
            "calculator",
            "web_fetch",
        ] {
            assert!(
                parsed
                    .autonomy
                    .auto_approve
                    .contains(&String::from(*default_tool)),
                "default tool '{default_tool}' must be present in auto_approve even when user provides custom list"
            );
        }
    }

    /// Regression test: empty auto_approve still gets defaults merged.
    #[test]
    async fn auto_approve_empty_list_gets_defaults() {
        let raw = r#"
default_temperature = 0.7

[autonomy]
auto_approve = []
"#;
        let parsed = parse_test_config(raw);
        let defaults = default_auto_approve();
        for tool in &defaults {
            assert!(
                parsed.autonomy.auto_approve.contains(tool),
                "default tool '{tool}' must be present even when user sets auto_approve = []"
            );
        }
    }

    /// When no autonomy section is provided, defaults are applied normally.
    #[test]
    async fn auto_approve_defaults_when_no_autonomy_section() {
        let raw = r#"
default_temperature = 0.7
"#;
        let parsed = parse_test_config(raw);
        let defaults = default_auto_approve();
        for tool in &defaults {
            assert!(
                parsed.autonomy.auto_approve.contains(tool),
                "default tool '{tool}' must be present when no [autonomy] section"
            );
        }
    }

    /// Duplicates are not introduced when ensure_default_auto_approve runs
    /// on a list that already contains the defaults.
    #[test]
    async fn auto_approve_no_duplicates() {
        let raw = r#"
default_temperature = 0.7

[autonomy]
auto_approve = ["weather", "file_read"]
"#;
        let parsed = parse_test_config(raw);
        let weather_count = parsed
            .autonomy
            .auto_approve
            .iter()
            .filter(|t| *t == "weather")
            .count();
        assert_eq!(weather_count, 1, "weather must not be duplicated");
        let file_read_count = parsed
            .autonomy
            .auto_approve
            .iter()
            .filter(|t| *t == "file_read")
            .count();
        assert_eq!(file_read_count, 1, "file_read must not be duplicated");
    }

    #[test]
    async fn provider_timeout_secs_parses_from_toml() {
        let raw = r#"
default_temperature = 0.7
provider_timeout_secs = 300
"#;
        let parsed = parse_test_config(raw);
        assert_eq!(parsed.provider_timeout_secs, 300);
    }

    #[test]
    async fn parse_extra_headers_env_basic() {
        let headers = parse_extra_headers_env("User-Agent:MyApp/1.0,X-Title:zeroclaw");
        assert_eq!(headers.len(), 2);
        assert_eq!(
            headers[0],
            ("User-Agent".to_string(), "MyApp/1.0".to_string())
        );
        assert_eq!(headers[1], ("X-Title".to_string(), "zeroclaw".to_string()));
    }

    #[test]
    async fn parse_extra_headers_env_with_url_value() {
        let headers =
            parse_extra_headers_env("HTTP-Referer:https://github.com/naraeclaw/naraeclaw");
        assert_eq!(headers.len(), 1);
        // Only splits on first colon, preserving URL colons in value
        assert_eq!(headers[0].0, "HTTP-Referer");
        assert_eq!(headers[0].1, "https://github.com/naraeclaw/naraeclaw");
    }

    #[test]
    async fn parse_extra_headers_env_empty_string() {
        let headers = parse_extra_headers_env("");
        assert!(headers.is_empty());
    }

    #[test]
    async fn parse_extra_headers_env_whitespace_trimming() {
        let headers = parse_extra_headers_env("  X-Title : zeroclaw , User-Agent : cli/1.0 ");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], ("X-Title".to_string(), "zeroclaw".to_string()));
        assert_eq!(
            headers[1],
            ("User-Agent".to_string(), "cli/1.0".to_string())
        );
    }

    #[test]
    async fn parse_extra_headers_env_skips_malformed() {
        let headers = parse_extra_headers_env("X-Valid:value,no-colon-here,Another:ok");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], ("X-Valid".to_string(), "value".to_string()));
        assert_eq!(headers[1], ("Another".to_string(), "ok".to_string()));
    }

    #[test]
    async fn parse_extra_headers_env_skips_empty_key() {
        let headers = parse_extra_headers_env(":value,X-Valid:ok");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], ("X-Valid".to_string(), "ok".to_string()));
    }

    #[test]
    async fn parse_extra_headers_env_allows_empty_value() {
        let headers = parse_extra_headers_env("X-Empty:");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], ("X-Empty".to_string(), String::new()));
    }

    #[test]
    async fn parse_extra_headers_env_trailing_comma() {
        let headers = parse_extra_headers_env("X-Title:zeroclaw,");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], ("X-Title".to_string(), "zeroclaw".to_string()));
    }

    #[test]
    async fn extra_headers_parses_from_toml() {
        let raw = r#"
default_temperature = 0.7

[extra_headers]
User-Agent = "MyApp/1.0"
X-Title = "zeroclaw"
"#;
        let parsed = parse_test_config(raw);
        assert_eq!(parsed.extra_headers.len(), 2);
        assert_eq!(parsed.extra_headers.get("User-Agent").unwrap(), "MyApp/1.0");
        assert_eq!(parsed.extra_headers.get("X-Title").unwrap(), "zeroclaw");
    }

    #[test]
    async fn extra_headers_defaults_to_empty() {
        let raw = r#"
default_temperature = 0.7
"#;
        let parsed = parse_test_config(raw);
        assert!(parsed.extra_headers.is_empty());
    }

    #[test]
    async fn storage_provider_dburl_alias_deserializes() {
        let raw = r#"
default_temperature = 0.7

[storage.provider.config]
provider = "qdrant"
dbURL = "http://localhost:6333"
schema = "public"
table = "memories"
connect_timeout_secs = 12
"#;

        let parsed = parse_test_config(raw);
        assert_eq!(parsed.storage.provider.config.provider, "qdrant");
        assert_eq!(
            parsed.storage.provider.config.db_url.as_deref(),
            Some("http://localhost:6333")
        );
        assert_eq!(parsed.storage.provider.config.schema, "public");
        assert_eq!(parsed.storage.provider.config.table, "memories");
        assert_eq!(
            parsed.storage.provider.config.connect_timeout_secs,
            Some(12)
        );
    }

    #[test]
    async fn runtime_reasoning_enabled_deserializes() {
        let raw = r#"
default_temperature = 0.7

[runtime]
reasoning_enabled = false
"#;

        let parsed = parse_test_config(raw);
        assert_eq!(parsed.runtime.reasoning_enabled, Some(false));
    }

    #[test]
    async fn runtime_reasoning_effort_deserializes() {
        let raw = r#"
default_temperature = 0.7

[runtime]
reasoning_effort = "HIGH"
"#;

        let parsed: Config = toml::from_str(raw).unwrap();
        assert_eq!(parsed.runtime.reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    async fn runtime_reasoning_effort_rejects_invalid_values() {
        let raw = r#"
default_temperature = 0.7

[runtime]
reasoning_effort = "turbo"
"#;

        let error = toml::from_str::<Config>(raw).expect_err("invalid value should fail");
        assert!(error.to_string().contains("reasoning_effort"));
    }

    #[test]
    async fn agent_config_defaults() {
        let cfg = AgentConfig::default();
        assert!(cfg.compact_context);
        assert_eq!(cfg.max_tool_iterations, 10);
        assert_eq!(cfg.max_history_messages, 50);
        assert!(!cfg.parallel_tools);
        assert_eq!(cfg.tool_dispatcher, "auto");
    }

    #[test]
    async fn agent_config_deserializes() {
        let raw = r#"
default_temperature = 0.7
[agent]
compact_context = true
max_tool_iterations = 20
max_history_messages = 80
parallel_tools = true
tool_dispatcher = "xml"
"#;
        let parsed = parse_test_config(raw);
        assert!(parsed.agent.compact_context);
        assert_eq!(parsed.agent.max_tool_iterations, 20);
        assert_eq!(parsed.agent.max_history_messages, 80);
        assert!(parsed.agent.parallel_tools);
        assert_eq!(parsed.agent.tool_dispatcher, "xml");
    }

    #[test]
    async fn pacing_config_defaults_are_all_none_or_empty() {
        let cfg = PacingConfig::default();
        assert!(cfg.step_timeout_secs.is_none());
        assert!(cfg.loop_detection_min_elapsed_secs.is_none());
        assert!(cfg.loop_ignore_tools.is_empty());
        assert!(cfg.message_timeout_scale_max.is_none());
    }

    #[test]
    async fn pacing_config_deserializes_from_toml() {
        let raw = r#"
default_temperature = 0.7
[pacing]
step_timeout_secs = 120
loop_detection_min_elapsed_secs = 60
loop_ignore_tools = ["browser_screenshot", "browser_navigate"]
message_timeout_scale_max = 8
"#;
        let parsed: Config = toml::from_str(raw).unwrap();
        assert_eq!(parsed.pacing.step_timeout_secs, Some(120));
        assert_eq!(parsed.pacing.loop_detection_min_elapsed_secs, Some(60));
        assert_eq!(
            parsed.pacing.loop_ignore_tools,
            vec!["browser_screenshot", "browser_navigate"]
        );
        assert_eq!(parsed.pacing.message_timeout_scale_max, Some(8));
    }

    #[test]
    async fn pacing_config_absent_preserves_defaults() {
        let raw = r#"
default_temperature = 0.7
"#;
        let parsed: Config = toml::from_str(raw).unwrap();
        assert!(parsed.pacing.step_timeout_secs.is_none());
        assert!(parsed.pacing.loop_detection_min_elapsed_secs.is_none());
        assert!(parsed.pacing.loop_ignore_tools.is_empty());
        assert!(parsed.pacing.message_timeout_scale_max.is_none());
    }

    #[tokio::test]
    async fn sync_directory_handles_existing_directory() {
        let dir = std::env::temp_dir().join(format!(
            "zeroclaw_test_sync_directory_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).await.unwrap();

        sync_directory(&dir).await.unwrap();

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn config_save_and_load_tmpdir() {
        let dir = std::env::temp_dir().join("zeroclaw_test_config");
        let _ = fs::remove_dir_all(&dir).await;
        fs::create_dir_all(&dir).await.unwrap();

        let config_path = dir.join("config.toml");
        let config = Config {
            workspace_dir: dir.join("workspace"),
            config_path: config_path.clone(),
            api_key: Some("sk-roundtrip".into()),
            api_url: None,
            api_path: None,
            default_provider: Some("openrouter".into()),
            default_model: Some("test-model".into()),
            model_providers: HashMap::new(),
            default_temperature: 0.9,
            provider_timeout_secs: 120,
            provider_max_tokens: None,
            extra_headers: HashMap::new(),
            observability: ObservabilityConfig::default(),
            autonomy: AutonomyConfig::default(),
            trust: crate::scattered_types::TrustConfig::default(),
            backup: BackupConfig::default(),
            data_retention: DataRetentionConfig::default(),
            cloud_ops: CloudOpsConfig::default(),
            security: SecurityConfig::default(),
            security_ops: SecurityOpsConfig::default(),
            runtime: RuntimeConfig::default(),
            reliability: ReliabilityConfig::default(),
            scheduler: SchedulerConfig::default(),
            skills: SkillsConfig::default(),
            pipeline: PipelineConfig::default(),
            model_routes: Vec::new(),
            embedding_routes: Vec::new(),
            query_classification: QueryClassificationConfig::default(),
            heartbeat: HeartbeatConfig::default(),
            cron: CronConfig::default(),
            skillforge: SkillForgeConfig::default(),
            channels_config: ChannelsConfig::default(),
            memory: MemoryConfig::default(),
            storage: StorageConfig::default(),
            tunnel: TunnelConfig::default(),
            gateway: GatewayConfig::default(),
            secrets: SecretsConfig::default(),
            browser: BrowserConfig::default(),
            browser_delegate: crate::scattered_types::BrowserDelegateConfig::default(),
            http_request: HttpRequestConfig::default(),
            multimodal: MultimodalConfig::default(),
            media_pipeline: MediaPipelineConfig::default(),
            web_fetch: WebFetchConfig::default(),
            link_enricher: LinkEnricherConfig::default(),
            text_browser: TextBrowserConfig::default(),
            web_search: WebSearchConfig::default(),
            google_workspace: GoogleWorkspaceConfig::default(),
            proxy: ProxyConfig::default(),
            agent: AgentConfig::default(),
            pacing: PacingConfig::default(),
            identity: IdentityConfig::default(),
            cost: CostConfig::default(),
            delegate: DelegateToolConfig::default(),
            agents: HashMap::new(),
            swarms: HashMap::new(),
            hooks: HooksConfig::default(),
            transcription: TranscriptionConfig::default(),
            tts: TtsConfig::default(),
            mcp: McpConfig::default(),
            nodes: NodesConfig::default(),
            workspace: WorkspaceConfig::default(),
            jira: JiraConfig::default(),
            redash: RedashConfig::default(),
            node_transport: NodeTransportConfig::default(),
            knowledge: KnowledgeConfig::default(),
            linkedin: LinkedInConfig::default(),
            image_gen: ImageGenConfig::default(),
            plugins: PluginsConfig::default(),
            locale: None,
            verifiable_intent: VerifiableIntentConfig::default(),
            claude_code: ClaudeCodeConfig::default(),
            claude_code_runner: ClaudeCodeRunnerConfig::default(),
            codex_cli: CodexCliConfig::default(),
            gemini_cli: GeminiCliConfig::default(),
            opencode_cli: OpenCodeCliConfig::default(),
            sop: SopConfig::default(),
            shell_tool: ShellToolConfig::default(),
        };

        config.save().await.unwrap();
        assert!(config_path.exists());

        let contents = tokio::fs::read_to_string(&config_path).await.unwrap();
        let loaded: Config = toml::from_str(&contents).unwrap();
        assert!(
            loaded
                .api_key
                .as_deref()
                .is_some_and(crate::secrets::SecretStore::is_encrypted)
        );
        let store = crate::secrets::SecretStore::new(&dir, true);
        let decrypted = store.decrypt(loaded.api_key.as_deref().unwrap()).unwrap();
        assert_eq!(decrypted, "sk-roundtrip");
        assert_eq!(loaded.default_model.as_deref(), Some("test-model"));
        assert!((loaded.default_temperature - 0.9).abs() < f64::EPSILON);

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn config_save_encrypts_nested_credentials() {
        let dir = std::env::temp_dir().join(format!(
            "zeroclaw_test_nested_credentials_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).await.unwrap();

        let mut config = Config::default();
        config.workspace_dir = dir.join("workspace");
        config.config_path = dir.join("config.toml");
        config.api_key = Some("root-credential".into());
        config.browser.computer_use.api_key = Some("browser-credential".into());
        config.web_search.brave_api_key = Some("brave-credential".into());
        config.storage.provider.config.db_url = Some("postgres://user:pw@host/db".into());

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn config_save_atomic_cleanup() {
        let dir =
            std::env::temp_dir().join(format!("zeroclaw_test_config_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).await.unwrap();

        let config_path = dir.join("config.toml");
        let mut config = Config::default();
        config.workspace_dir = dir.join("workspace");
        config.config_path = config_path.clone();
        config.default_model = Some("model-a".into());
        config.save().await.unwrap();
        assert!(config_path.exists());

        config.default_model = Some("model-b".into());
        config.save().await.unwrap();

        let contents = tokio::fs::read_to_string(&config_path).await.unwrap();
        assert!(contents.contains("model-b"));

        let names: Vec<String> = ReadDirStream::new(fs::read_dir(&dir).await.unwrap())
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect()
            .await;
        assert!(!names.iter().any(|name| name.contains(".tmp-")));
        assert!(!names.iter().any(|name| name.ends_with(".bak")));

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[test]
    async fn slack_config_with_tokens() {
        let json = r#"{"app_token":"xapp-1-test","bot_token":"xoxb-test"}"#;
        let parsed: SlackConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.app_token, "xapp-1-test");
        assert_eq!(parsed.bot_token, "xoxb-test");
        assert!(parsed.signing_secret.is_none());
    }

    #[test]
    async fn slack_config_with_signing_secret() {
        let json =
            r#"{"app_token":"xapp-1-test","bot_token":"xoxb-test","signing_secret":"abc123"}"#;
        let parsed: SlackConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.signing_secret.as_deref(), Some("abc123"));
    }

    // ══════════════════════════════════════════════════════════
    // SECURITY CHECKLIST TESTS — Gateway config
    // ══════════════════════════════════════════════════════════

    #[test]
    async fn checklist_gateway_default_requires_pairing() {
        let g = GatewayConfig::default();
        assert!(g.require_pairing, "Pairing must be required by default");
    }

    #[test]
    async fn checklist_gateway_default_blocks_public_bind() {
        let g = GatewayConfig::default();
        assert!(
            !g.allow_public_bind,
            "Public bind must be blocked by default"
        );
    }

    #[test]
    async fn checklist_gateway_default_no_tokens() {
        let g = GatewayConfig::default();
        assert!(
            g.paired_tokens.is_empty(),
            "No pre-paired tokens by default"
        );
        assert_eq!(g.pair_rate_limit_per_minute, 10);
        assert_eq!(g.webhook_rate_limit_per_minute, 60);
        assert!(!g.trust_forwarded_headers);
        assert_eq!(g.rate_limit_max_keys, 10_000);
        assert_eq!(g.idempotency_ttl_secs, 300);
        assert_eq!(g.idempotency_max_keys, 10_000);
    }

    #[test]
    async fn checklist_gateway_cli_default_host_is_localhost() {
        // The CLI default for --host is 127.0.0.1 (checked in main.rs)
        // Here we verify the config default matches
        let c = Config::default();
        assert!(
            c.gateway.require_pairing,
            "Config default must require pairing"
        );
        assert!(
            !c.gateway.allow_public_bind,
            "Config default must block public bind"
        );
    }

    #[test]
    async fn checklist_gateway_serde_roundtrip() {
        let g = GatewayConfig {
            port: 42617,
            host: "127.0.0.1".into(),
            require_pairing: true,
            allow_public_bind: false,
            paired_tokens: vec!["zc_test_token".into()],
            pair_rate_limit_per_minute: 12,
            webhook_rate_limit_per_minute: 80,
            trust_forwarded_headers: true,
            path_prefix: Some("/zeroclaw".into()),
            rate_limit_max_keys: 2048,
            idempotency_ttl_secs: 600,
            idempotency_max_keys: 4096,
            session_persistence: true,
            session_ttl_hours: 0,
            pairing_dashboard: PairingDashboardConfig::default(),
            web_dist_dir: None,
            tls: None,
        };
        let toml_str = toml::to_string(&g).unwrap();
        let parsed: GatewayConfig = toml::from_str(&toml_str).unwrap();
        assert!(parsed.require_pairing);
        assert!(parsed.session_persistence);
        assert_eq!(parsed.session_ttl_hours, 0);
        assert!(!parsed.allow_public_bind);
        assert_eq!(parsed.paired_tokens, vec!["zc_test_token"]);
        assert_eq!(parsed.pair_rate_limit_per_minute, 12);
        assert_eq!(parsed.webhook_rate_limit_per_minute, 80);
        assert!(parsed.trust_forwarded_headers);
        assert_eq!(parsed.path_prefix.as_deref(), Some("/zeroclaw"));
        assert_eq!(parsed.rate_limit_max_keys, 2048);
        assert_eq!(parsed.idempotency_ttl_secs, 600);
        assert_eq!(parsed.idempotency_max_keys, 4096);
    }

    #[test]
    async fn checklist_gateway_backward_compat_no_gateway_section() {
        // Old configs without [gateway] should get secure defaults
        let minimal = r#"
workspace_dir = "/tmp/ws"
config_path = "/tmp/config.toml"
default_temperature = 0.7
"#;
        let parsed = parse_test_config(minimal);
        assert!(
            parsed.gateway.require_pairing,
            "Missing [gateway] must default to require_pairing=true"
        );
        assert!(
            !parsed.gateway.allow_public_bind,
            "Missing [gateway] must default to allow_public_bind=false"
        );
    }

    #[test]
    async fn checklist_autonomy_default_is_workspace_scoped() {
        let a = AutonomyConfig::default();
        assert!(a.workspace_only, "Default autonomy must be workspace_only");
        assert!(
            a.forbidden_paths.contains(&"/etc".to_string()),
            "Must block /etc"
        );
        assert!(
            a.forbidden_paths.contains(&"/proc".to_string()),
            "Must block /proc"
        );
        assert!(
            a.forbidden_paths.contains(&"~/.ssh".to_string()),
            "Must block ~/.ssh"
        );
    }

    // ══════════════════════════════════════════════════════════
    // SECRETS CONFIG TESTS
    // ══════════════════════════════════════════════════════════

    #[test]
    async fn secrets_config_default_encrypts() {
        let s = SecretsConfig::default();
        assert!(s.encrypt, "Encryption must be enabled by default");
    }

    #[test]
    async fn secrets_config_serde_roundtrip() {
        let s = SecretsConfig { encrypt: false };
        let toml_str = toml::to_string(&s).unwrap();
        let parsed: SecretsConfig = toml::from_str(&toml_str).unwrap();
        assert!(!parsed.encrypt);
    }

    #[test]
    async fn secrets_config_backward_compat_missing_section() {
        let minimal = r#"
workspace_dir = "/tmp/ws"
config_path = "/tmp/config.toml"
default_temperature = 0.7
"#;
        let parsed = parse_test_config(minimal);
        assert!(
            parsed.secrets.encrypt,
            "Missing [secrets] must default to encrypt=true"
        );
    }

    #[test]
    async fn browser_config_default_enabled() {
        let b = BrowserConfig::default();
        assert!(b.enabled);
        assert_eq!(b.allowed_domains, vec!["*".to_string()]);
        assert_eq!(b.backend, "agent_browser");
        assert!(b.native_headless);
        assert_eq!(b.native_webdriver_url, "http://127.0.0.1:9515");
        assert!(b.native_chrome_path.is_none());
        assert_eq!(b.computer_use.endpoint, "http://127.0.0.1:8787/v1/actions");
        assert_eq!(b.computer_use.timeout_ms, 15_000);
        assert!(!b.computer_use.allow_remote_endpoint);
        assert!(b.computer_use.window_allowlist.is_empty());
        assert!(b.computer_use.max_coordinate_x.is_none());
        assert!(b.computer_use.max_coordinate_y.is_none());
    }

    #[test]
    async fn browser_config_serde_roundtrip() {
        let b = BrowserConfig {
            enabled: true,
            allowed_domains: vec!["example.com".into(), "docs.example.com".into()],
            session_name: None,
            backend: "auto".into(),
            native_headless: false,
            native_webdriver_url: "http://localhost:4444".into(),
            native_chrome_path: Some("/usr/bin/chromium".into()),
            computer_use: BrowserComputerUseConfig {
                endpoint: "https://computer-use.example.com/v1/actions".into(),
                api_key: Some("test-token".into()),
                timeout_ms: 8_000,
                allow_remote_endpoint: true,
                window_allowlist: vec!["Chrome".into(), "Visual Studio Code".into()],
                max_coordinate_x: Some(3840),
                max_coordinate_y: Some(2160),
            },
        };
        let toml_str = toml::to_string(&b).unwrap();
        let parsed: BrowserConfig = toml::from_str(&toml_str).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.allowed_domains.len(), 2);
        assert_eq!(parsed.allowed_domains[0], "example.com");
        assert_eq!(parsed.backend, "auto");
        assert!(!parsed.native_headless);
        assert_eq!(parsed.native_webdriver_url, "http://localhost:4444");
        assert_eq!(
            parsed.native_chrome_path.as_deref(),
            Some("/usr/bin/chromium")
        );
        assert_eq!(
            parsed.computer_use.endpoint,
            "https://computer-use.example.com/v1/actions"
        );
        assert_eq!(parsed.computer_use.api_key.as_deref(), Some("test-token"));
        assert_eq!(parsed.computer_use.timeout_ms, 8_000);
        assert!(parsed.computer_use.allow_remote_endpoint);
        assert_eq!(parsed.computer_use.window_allowlist.len(), 2);
        assert_eq!(parsed.computer_use.max_coordinate_x, Some(3840));
        assert_eq!(parsed.computer_use.max_coordinate_y, Some(2160));
    }

    #[test]
    async fn browser_config_backward_compat_missing_section() {
        let minimal = r#"
workspace_dir = "/tmp/ws"
config_path = "/tmp/config.toml"
default_temperature = 0.7
"#;
        let parsed = parse_test_config(minimal);
        assert!(parsed.browser.enabled);
        assert_eq!(parsed.browser.allowed_domains, vec!["*".to_string()]);
    }

    // ── Environment variable overrides (Docker support) ─────────

    async fn env_override_lock() -> MutexGuard<'static, ()> {
        static ENV_OVERRIDE_TEST_LOCK: Mutex<()> = Mutex::const_new(());
        ENV_OVERRIDE_TEST_LOCK.lock().await
    }

    fn clear_proxy_env_test_vars() {
        for key in [
            "NARAECLAW_PROXY_ENABLED",
            "NARAECLAW_HTTP_PROXY",
            "NARAECLAW_HTTPS_PROXY",
            "NARAECLAW_ALL_PROXY",
            "NARAECLAW_NO_PROXY",
            "NARAECLAW_PROXY_SCOPE",
            "NARAECLAW_PROXY_SERVICES",
            "ZEROCLAW_PROXY_ENABLED",
            "ZEROCLAW_HTTP_PROXY",
            "ZEROCLAW_HTTPS_PROXY",
            "ZEROCLAW_ALL_PROXY",
            "ZEROCLAW_NO_PROXY",
            "ZEROCLAW_PROXY_SCOPE",
            "ZEROCLAW_PROXY_SERVICES",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
        ] {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var(key) };
        }
    }

    #[test]
    async fn env_override_api_key() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        assert!(config.api_key.is_none());

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("NARAECLAW_API_KEY", "sk-test-env-key") };
        config.apply_env_overrides();
        assert_eq!(config.api_key.as_deref(), Some("sk-test-env-key"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("NARAECLAW_API_KEY") };
    }

    #[test]
    async fn env_override_api_key_fallback() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("NARAECLAW_API_KEY") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_API_KEY") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("API_KEY", "sk-fallback-key") };
        config.apply_env_overrides();
        assert_eq!(config.api_key.as_deref(), Some("sk-fallback-key"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("API_KEY") };
    }

    #[test]
    async fn env_override_provider() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_PROVIDER", "anthropic") };
        config.apply_env_overrides();
        assert_eq!(config.default_provider.as_deref(), Some("anthropic"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_PROVIDER") };
    }

    #[test]
    async fn env_override_model_provider_alias() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_PROVIDER") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_MODEL_PROVIDER", "openai-codex") };
        config.apply_env_overrides();
        assert_eq!(config.default_provider.as_deref(), Some("openai-codex"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_MODEL_PROVIDER") };
    }

    #[test]
    async fn toml_supports_model_provider_and_model_alias_fields() {
        let raw = r#"
default_temperature = 0.7
model_provider = "sub2api"
model = "gpt-5.3-codex"

[model_providers.sub2api]
name = "sub2api"
base_url = "https://api.tonsof.blue/v1"
wire_api = "responses"
requires_openai_auth = true
"#;

        let parsed = parse_test_config(raw);
        assert_eq!(parsed.default_provider.as_deref(), Some("sub2api"));
        assert_eq!(parsed.default_model.as_deref(), Some("gpt-5.3-codex"));
        let profile = parsed
            .model_providers
            .get("sub2api")
            .expect("profile should exist");
        assert_eq!(profile.wire_api.as_deref(), Some("responses"));
        assert!(profile.requires_openai_auth);
    }

    #[test]
    async fn env_override_open_skills_enabled_and_dir() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        assert!(!config.skills.open_skills_enabled);
        assert!(config.skills.open_skills_dir.is_none());
        assert_eq!(
            config.skills.prompt_injection_mode,
            SkillsPromptInjectionMode::Full
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_OPEN_SKILLS_ENABLED", "true") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_OPEN_SKILLS_DIR", "/tmp/open-skills") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_SKILLS_ALLOW_SCRIPTS", "yes") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_SKILLS_PROMPT_MODE", "compact") };
        config.apply_env_overrides();

        assert!(config.skills.open_skills_enabled);
        assert!(config.skills.allow_scripts);
        assert_eq!(
            config.skills.open_skills_dir.as_deref(),
            Some("/tmp/open-skills")
        );
        assert_eq!(
            config.skills.prompt_injection_mode,
            SkillsPromptInjectionMode::Compact
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_OPEN_SKILLS_ENABLED") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_OPEN_SKILLS_DIR") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_SKILLS_ALLOW_SCRIPTS") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_SKILLS_PROMPT_MODE") };
    }

    #[test]
    async fn env_override_open_skills_enabled_invalid_value_keeps_existing_value() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.skills.open_skills_enabled = true;
        config.skills.allow_scripts = true;
        config.skills.prompt_injection_mode = SkillsPromptInjectionMode::Compact;

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_OPEN_SKILLS_ENABLED", "maybe") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_SKILLS_ALLOW_SCRIPTS", "maybe") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_SKILLS_PROMPT_MODE", "invalid") };
        config.apply_env_overrides();

        assert!(config.skills.open_skills_enabled);
        assert!(config.skills.allow_scripts);
        assert_eq!(
            config.skills.prompt_injection_mode,
            SkillsPromptInjectionMode::Compact
        );
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_OPEN_SKILLS_ENABLED") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_SKILLS_ALLOW_SCRIPTS") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_SKILLS_PROMPT_MODE") };
    }

    #[test]
    async fn env_override_provider_fallback() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_PROVIDER") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("PROVIDER", "openai") };
        config.apply_env_overrides();
        assert_eq!(config.default_provider.as_deref(), Some("openai"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("PROVIDER") };
    }

    #[test]
    async fn env_override_provider_fallback_does_not_replace_non_default_provider() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.default_provider = Some("custom:https://proxy.example.com/v1".to_string());

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_PROVIDER") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("PROVIDER", "openrouter") };
        config.apply_env_overrides();
        assert_eq!(
            config.default_provider.as_deref(),
            Some("custom:https://proxy.example.com/v1")
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("PROVIDER") };
    }

    #[test]
    async fn env_override_zero_claw_provider_overrides_non_default_provider() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.default_provider = Some("custom:https://proxy.example.com/v1".to_string());

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_PROVIDER", "openrouter") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("PROVIDER", "anthropic") };
        config.apply_env_overrides();
        assert_eq!(config.default_provider.as_deref(), Some("openrouter"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_PROVIDER") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("PROVIDER") };
    }

    #[test]
    async fn env_override_glm_api_key_for_global_alias() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.default_provider = Some("glm".to_string());

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("GLM_API_KEY", "glm-key") };
        config.apply_env_overrides();
        assert_eq!(config.api_key.as_deref(), Some("glm-key"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("GLM_API_KEY") };
    }

    #[test]
    async fn env_override_zai_api_key_for_global_alias() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.default_provider = Some("zai".to_string());

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZAI_API_KEY", "zai-key") };
        config.apply_env_overrides();
        assert_eq!(config.api_key.as_deref(), Some("zai-key"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZAI_API_KEY") };
    }

    #[test]
    async fn env_override_model() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_MODEL", "gpt-4o") };
        config.apply_env_overrides();
        assert_eq!(config.default_model.as_deref(), Some("gpt-4o"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_MODEL") };
    }

    #[test]
    async fn model_provider_profile_maps_to_custom_endpoint() {
        let _env_guard = env_override_lock().await;
        let mut profile = ModelProviderConfig::default();
        profile.name = Some("sub2api".to_string());
        profile.base_url = Some("https://api.tonsof.blue/v1".to_string());
        profile.wire_api = None;
        profile.requires_openai_auth = false;
        profile.azure_openai_resource = None;
        profile.azure_openai_deployment = None;
        profile.azure_openai_api_version = None;
        profile.api_path = None;
        profile.max_tokens = None;

        let mut config = Config::default();
        config.default_provider = Some("sub2api".to_string());
        config.model_providers = HashMap::from([("sub2api".to_string(), profile)]);

        config.apply_env_overrides();
        assert_eq!(
            config.default_provider.as_deref(),
            Some("custom:https://api.tonsof.blue/v1")
        );
        assert_eq!(
            config.api_url.as_deref(),
            Some("https://api.tonsof.blue/v1")
        );
    }

    #[test]
    async fn model_provider_profile_responses_uses_openai_codex_and_openai_key() {
        let _env_guard = env_override_lock().await;
        let mut profile = ModelProviderConfig::default();
        profile.name = Some("sub2api".to_string());
        profile.base_url = Some("https://api.tonsof.blue".to_string());
        profile.wire_api = Some("responses".to_string());
        profile.requires_openai_auth = true;
        profile.azure_openai_resource = None;
        profile.azure_openai_deployment = None;
        profile.azure_openai_api_version = None;
        profile.api_path = None;
        profile.max_tokens = None;

        let mut config = Config::default();
        config.default_provider = Some("sub2api".to_string());
        config.model_providers = HashMap::from([("sub2api".to_string(), profile)]);
        config.api_key = None;

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("OPENAI_API_KEY", "sk-test-codex-key") };
        config.apply_env_overrides();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("OPENAI_API_KEY") };

        assert_eq!(config.default_provider.as_deref(), Some("openai-codex"));
        assert_eq!(config.api_url.as_deref(), Some("https://api.tonsof.blue"));
        assert_eq!(config.api_key.as_deref(), Some("sk-test-codex-key"));
    }

    #[test]
    async fn save_repairs_bare_config_filename_using_runtime_resolution() {
        let _env_guard = env_override_lock().await;
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let workspace_dir = temp_home.join("workspace");
        let resolved_config_path = temp_home.join(".naraeclaw").join("config.toml");

        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOME", &temp_home) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", &workspace_dir) };

        let mut config = Config::default();
        config.workspace_dir = workspace_dir;
        config.config_path = PathBuf::from("config.toml");
        config.default_temperature = 0.5;
        config.save().await.unwrap();

        assert!(resolved_config_path.exists());
        let saved = tokio::fs::read_to_string(&resolved_config_path)
            .await
            .unwrap();
        let parsed = parse_test_config(&saved);
        assert_eq!(parsed.default_temperature, 0.5);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        if let Some(home) = original_home {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::set_var("HOME", home) };
        } else {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = tokio::fs::remove_dir_all(temp_home).await;
    }

    #[test]
    async fn validate_ollama_cloud_model_requires_remote_api_url() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.default_provider = Some("ollama".to_string());
        config.default_model = Some("glm-5:cloud".to_string());
        config.api_url = None;
        config.api_key = Some("ollama-key".to_string());

        let error = config.validate().expect_err("expected validation to fail");
        assert!(error.to_string().contains(
            "default_model uses ':cloud' with provider 'ollama', but api_url is local or unset"
        ));
    }

    #[test]
    async fn validate_ollama_cloud_model_accepts_remote_endpoint_and_env_key() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.default_provider = Some("ollama".to_string());
        config.default_model = Some("glm-5:cloud".to_string());
        config.api_url = Some("https://ollama.com/api".to_string());
        config.api_key = None;

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("OLLAMA_API_KEY", "ollama-env-key") };
        let result = config.validate();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("OLLAMA_API_KEY") };

        assert!(result.is_ok(), "expected validation to pass: {result:?}");
    }

    #[test]
    async fn validate_rejects_unknown_model_provider_wire_api() {
        let _env_guard = env_override_lock().await;
        let mut profile = ModelProviderConfig::default();
        profile.name = Some("sub2api".to_string());
        profile.base_url = Some("https://api.tonsof.blue/v1".to_string());
        profile.wire_api = Some("ws".to_string());
        profile.requires_openai_auth = false;
        profile.azure_openai_resource = None;
        profile.azure_openai_deployment = None;
        profile.azure_openai_api_version = None;
        profile.api_path = None;
        profile.max_tokens = None;

        let mut config = Config::default();
        config.default_provider = Some("sub2api".to_string());
        config.model_providers = HashMap::from([("sub2api".to_string(), profile)]);

        let error = config.validate().expect_err("expected validation failure");
        assert!(
            error
                .to_string()
                .contains("wire_api must be one of: responses, chat_completions")
        );
    }

    #[test]
    async fn env_override_model_fallback() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_MODEL") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("MODEL", "anthropic/claude-3.5-sonnet") };
        config.apply_env_overrides();
        assert_eq!(
            config.default_model.as_deref(),
            Some("anthropic/claude-3.5-sonnet")
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("MODEL") };
    }

    #[test]
    async fn env_override_workspace() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", "/custom/workspace") };
        config.apply_env_overrides();
        assert_eq!(config.workspace_dir, PathBuf::from("/custom/workspace"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
    }

    #[test]
    async fn resolve_runtime_config_dirs_uses_env_workspace_first() {
        let _env_guard = env_override_lock().await;
        let default_config_dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let default_workspace_dir = default_config_dir.join("workspace");
        let workspace_dir = default_config_dir.join("profile-a");

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", &workspace_dir) };
        let (config_dir, resolved_workspace_dir, source) =
            resolve_runtime_config_dirs(&default_config_dir, &default_workspace_dir)
                .await
                .unwrap();

        assert_eq!(source, ConfigResolutionSource::EnvWorkspace);
        assert_eq!(config_dir, workspace_dir);
        assert_eq!(resolved_workspace_dir, workspace_dir.join("workspace"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        let _ = fs::remove_dir_all(default_config_dir).await;
    }

    #[test]
    async fn resolve_runtime_config_dirs_uses_env_config_dir_first() {
        let _env_guard = env_override_lock().await;
        let default_config_dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let default_workspace_dir = default_config_dir.join("workspace");
        let explicit_config_dir = default_config_dir.join("explicit-config");
        let marker_config_dir = default_config_dir.join("profiles").join("alpha");
        let state_path = default_config_dir.join(ACTIVE_WORKSPACE_STATE_FILE);

        fs::create_dir_all(&default_config_dir).await.unwrap();
        let state = ActiveWorkspaceState {
            config_dir: marker_config_dir.to_string_lossy().into_owned(),
        };
        fs::write(&state_path, toml::to_string(&state).unwrap())
            .await
            .unwrap();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_CONFIG_DIR", &explicit_config_dir) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };

        let (config_dir, resolved_workspace_dir, source) =
            resolve_runtime_config_dirs(&default_config_dir, &default_workspace_dir)
                .await
                .unwrap();

        assert_eq!(source, ConfigResolutionSource::EnvConfigDir);
        assert_eq!(config_dir, explicit_config_dir);
        assert_eq!(
            resolved_workspace_dir,
            explicit_config_dir.join("workspace")
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_CONFIG_DIR") };
        let _ = fs::remove_dir_all(default_config_dir).await;
    }

    #[test]
    async fn resolve_runtime_config_dirs_uses_active_workspace_marker() {
        let _env_guard = env_override_lock().await;
        let default_config_dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let default_workspace_dir = default_config_dir.join("workspace");
        let marker_config_dir = default_config_dir.join("profiles").join("alpha");
        let state_path = default_config_dir.join(ACTIVE_WORKSPACE_STATE_FILE);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        fs::create_dir_all(&default_config_dir).await.unwrap();
        let state = ActiveWorkspaceState {
            config_dir: marker_config_dir.to_string_lossy().into_owned(),
        };
        fs::write(&state_path, toml::to_string(&state).unwrap())
            .await
            .unwrap();

        let (config_dir, resolved_workspace_dir, source) =
            resolve_runtime_config_dirs(&default_config_dir, &default_workspace_dir)
                .await
                .unwrap();

        assert_eq!(source, ConfigResolutionSource::ActiveWorkspaceMarker);
        assert_eq!(config_dir, marker_config_dir);
        assert_eq!(resolved_workspace_dir, marker_config_dir.join("workspace"));

        let _ = fs::remove_dir_all(default_config_dir).await;
    }

    #[test]
    async fn resolve_runtime_config_dirs_falls_back_to_default_layout() {
        let _env_guard = env_override_lock().await;
        let default_config_dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let default_workspace_dir = default_config_dir.join("workspace");

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        let (config_dir, resolved_workspace_dir, source) =
            resolve_runtime_config_dirs(&default_config_dir, &default_workspace_dir)
                .await
                .unwrap();

        assert_eq!(source, ConfigResolutionSource::DefaultConfigDir);
        assert_eq!(config_dir, default_config_dir);
        assert_eq!(resolved_workspace_dir, default_workspace_dir);

        let _ = fs::remove_dir_all(default_config_dir).await;
    }

    #[test]
    async fn load_or_init_workspace_override_uses_workspace_root_for_config() {
        let _env_guard = env_override_lock().await;
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let workspace_dir = temp_home.join("profile-a");

        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOME", &temp_home) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", &workspace_dir) };

        let config = Box::pin(Config::load_or_init()).await.unwrap();

        assert_eq!(config.workspace_dir, workspace_dir.join("workspace"));
        assert_eq!(config.config_path, workspace_dir.join("config.toml"));
        assert!(workspace_dir.join("config.toml").exists());

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        if let Some(home) = original_home {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::set_var("HOME", home) };
        } else {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = fs::remove_dir_all(temp_home).await;
    }

    #[test]
    async fn load_or_init_workspace_suffix_uses_legacy_config_layout() {
        let _env_guard = env_override_lock().await;
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let workspace_dir = temp_home.join("workspace");
        let legacy_config_path = temp_home.join(".naraeclaw").join("config.toml");

        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOME", &temp_home) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", &workspace_dir) };

        let config = Box::pin(Config::load_or_init()).await.unwrap();

        assert_eq!(config.workspace_dir, workspace_dir);
        assert_eq!(config.config_path, legacy_config_path);
        assert!(config.config_path.exists());

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        if let Some(home) = original_home {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::set_var("HOME", home) };
        } else {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = fs::remove_dir_all(temp_home).await;
    }

    #[test]
    async fn load_or_init_workspace_override_keeps_existing_legacy_config() {
        let _env_guard = env_override_lock().await;
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let workspace_dir = temp_home.join("custom-workspace");
        let legacy_config_dir = temp_home.join(".naraeclaw");
        let legacy_config_path = legacy_config_dir.join("config.toml");

        fs::create_dir_all(&legacy_config_dir).await.unwrap();
        fs::write(
            &legacy_config_path,
            r#"default_temperature = 0.7
default_model = "legacy-model"
"#,
        )
        .await
        .unwrap();

        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOME", &temp_home) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", &workspace_dir) };

        let config = Box::pin(Config::load_or_init()).await.unwrap();

        assert_eq!(config.workspace_dir, workspace_dir);
        assert_eq!(config.config_path, legacy_config_path);
        assert_eq!(config.default_model.as_deref(), Some("legacy-model"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        if let Some(home) = original_home {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::set_var("HOME", home) };
        } else {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = fs::remove_dir_all(temp_home).await;
    }

    #[test]
    async fn load_or_init_uses_persisted_active_workspace_marker() {
        let _env_guard = env_override_lock().await;
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let temp_default_dir = temp_home.join(".naraeclaw");
        let custom_config_dir = temp_home.join("profiles").join("agent-alpha");

        fs::create_dir_all(&custom_config_dir).await.unwrap();
        // Pre-create the default dir so is_temp_directory() can canonicalize
        // the path on macOS (where /var → /private/var symlink requires
        // the directory to exist for canonicalize to resolve correctly).
        fs::create_dir_all(&temp_default_dir).await.unwrap();
        fs::write(
            custom_config_dir.join("config.toml"),
            "default_temperature = 0.7\ndefault_model = \"persisted-profile\"\n",
        )
        .await
        .unwrap();

        // Write the marker using the explicit default dir (no HOME manipulation
        // needed for the persist call itself).
        persist_active_workspace_config_dir_in(&custom_config_dir, &temp_default_dir)
            .await
            .unwrap();

        // Config::load_or_init still reads HOME to find the marker, so we
        // must override HOME here. The persist above already wrote to the
        // correct temp location, so no stale marker can leak.
        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOME", &temp_home) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };

        let config = Box::pin(Config::load_or_init()).await.unwrap();

        assert_eq!(config.config_path, custom_config_dir.join("config.toml"));
        assert_eq!(config.workspace_dir, custom_config_dir.join("workspace"));
        assert_eq!(config.default_model.as_deref(), Some("persisted-profile"));

        if let Some(home) = original_home {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::set_var("HOME", home) };
        } else {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = fs::remove_dir_all(temp_home).await;
    }

    #[test]
    async fn load_or_init_env_workspace_override_takes_priority_over_marker() {
        let _env_guard = env_override_lock().await;
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let temp_default_dir = temp_home.join(".naraeclaw");
        let marker_config_dir = temp_home.join("profiles").join("persisted-profile");
        let env_workspace_dir = temp_home.join("env-workspace");

        fs::create_dir_all(&marker_config_dir).await.unwrap();
        fs::write(
            marker_config_dir.join("config.toml"),
            "default_temperature = 0.7\ndefault_model = \"marker-model\"\n",
        )
        .await
        .unwrap();

        // Write marker via explicit default dir, then set HOME for load_or_init.
        persist_active_workspace_config_dir_in(&marker_config_dir, &temp_default_dir)
            .await
            .unwrap();

        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOME", &temp_home) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", &env_workspace_dir) };

        let config = Box::pin(Config::load_or_init()).await.unwrap();

        assert_eq!(config.workspace_dir, env_workspace_dir.join("workspace"));
        assert_eq!(config.config_path, env_workspace_dir.join("config.toml"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        if let Some(home) = original_home {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::set_var("HOME", home) };
        } else {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = fs::remove_dir_all(temp_home).await;
    }

    #[test]
    async fn persist_active_workspace_marker_is_cleared_for_default_config_dir() {
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let default_config_dir = temp_home.join(".naraeclaw");
        let custom_config_dir = temp_home.join("profiles").join("custom-profile");
        let marker_path = default_config_dir.join(ACTIVE_WORKSPACE_STATE_FILE);

        // Use the _in variant directly -- no HOME manipulation needed since
        // this test only exercises persist/clear logic, not Config::load_or_init.
        persist_active_workspace_config_dir_in(&custom_config_dir, &default_config_dir)
            .await
            .unwrap();
        assert!(marker_path.exists());

        persist_active_workspace_config_dir_in(&default_config_dir, &default_config_dir)
            .await
            .unwrap();
        assert!(!marker_path.exists());

        let _ = fs::remove_dir_all(temp_home).await;
    }

    #[test]
    #[allow(clippy::large_futures)]
    async fn load_or_init_logs_existing_config_as_initialized() {
        let _env_guard = env_override_lock().await;
        let temp_home =
            std::env::temp_dir().join(format!("zeroclaw_test_home_{}", uuid::Uuid::new_v4()));
        let workspace_dir = temp_home.join("profile-a");
        let config_path = workspace_dir.join("config.toml");

        fs::create_dir_all(&workspace_dir).await.unwrap();
        fs::write(
            &config_path,
            r#"default_temperature = 0.7
default_model = "persisted-profile"
"#,
        )
        .await
        .unwrap();

        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOME", &temp_home) };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_WORKSPACE", &workspace_dir) };

        let capture = SharedLogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_writer(capture.clone())
            .finish();
        let dispatch = tracing::Dispatch::new(subscriber);
        let guard = tracing::dispatcher::set_default(&dispatch);

        let config = Box::pin(Config::load_or_init()).await.unwrap();

        drop(guard);
        let logs = capture.captured();

        assert_eq!(config.workspace_dir, workspace_dir.join("workspace"));
        assert_eq!(config.config_path, config_path);
        assert_eq!(config.default_model.as_deref(), Some("persisted-profile"));
        assert!(logs.contains("Config loaded"), "{logs}");
        assert!(logs.contains("initialized=true"), "{logs}");
        assert!(!logs.contains("initialized=false"), "{logs}");

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_WORKSPACE") };
        if let Some(home) = original_home {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::set_var("HOME", home) };
        } else {
            // SAFETY: test-only, single-threaded test runner.
            unsafe { std::env::remove_var("HOME") };
        }
        let _ = fs::remove_dir_all(temp_home).await;
    }

    #[test]
    async fn env_override_empty_values_ignored() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        let original_provider = config.default_provider.clone();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_PROVIDER", "") };
        config.apply_env_overrides();
        assert_eq!(config.default_provider, original_provider);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_PROVIDER") };
    }

    #[test]
    async fn env_override_gateway_port() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        assert_eq!(config.gateway.port, 42617);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_GATEWAY_PORT", "8080") };
        config.apply_env_overrides();
        assert_eq!(config.gateway.port, 8080);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_GATEWAY_PORT") };
    }

    #[test]
    async fn env_override_port_fallback() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_GATEWAY_PORT") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("PORT", "9000") };
        config.apply_env_overrides();
        assert_eq!(config.gateway.port, 9000);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("PORT") };
    }

    #[test]
    async fn env_override_gateway_host() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        assert_eq!(config.gateway.host, "127.0.0.1");

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_GATEWAY_HOST", "0.0.0.0") };
        config.apply_env_overrides();
        assert_eq!(config.gateway.host, "0.0.0.0");

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_GATEWAY_HOST") };
    }

    #[test]
    async fn env_override_host_fallback() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_GATEWAY_HOST") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("HOST", "0.0.0.0") };
        config.apply_env_overrides();
        assert_eq!(config.gateway.host, "0.0.0.0");

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("HOST") };
    }

    #[test]
    async fn env_override_require_pairing() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        assert!(config.gateway.require_pairing);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_REQUIRE_PAIRING", "false") };
        config.apply_env_overrides();
        assert!(!config.gateway.require_pairing);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_REQUIRE_PAIRING", "true") };
        config.apply_env_overrides();
        assert!(config.gateway.require_pairing);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_REQUIRE_PAIRING") };
    }

    #[test]
    async fn env_override_temperature() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_TEMPERATURE", "0.5") };
        config.apply_env_overrides();
        assert!((config.default_temperature - 0.5).abs() < f64::EPSILON);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_TEMPERATURE") };
    }

    #[test]
    async fn env_override_temperature_out_of_range_ignored() {
        let _env_guard = env_override_lock().await;
        // Clean up any leftover env vars from other tests
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_TEMPERATURE") };

        let mut config = Config::default();
        let original_temp = config.default_temperature;

        // Temperature > 2.0 should be ignored
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_TEMPERATURE", "3.0") };
        config.apply_env_overrides();
        assert!(
            (config.default_temperature - original_temp).abs() < f64::EPSILON,
            "Temperature 3.0 should be ignored (out of range)"
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_TEMPERATURE") };
    }

    #[test]
    async fn env_override_reasoning_enabled() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        assert_eq!(config.runtime.reasoning_enabled, None);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_REASONING_ENABLED", "false") };
        config.apply_env_overrides();
        assert_eq!(config.runtime.reasoning_enabled, Some(false));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_REASONING_ENABLED", "true") };
        config.apply_env_overrides();
        assert_eq!(config.runtime.reasoning_enabled, Some(true));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_REASONING_ENABLED") };
    }

    #[test]
    async fn env_override_reasoning_invalid_value_ignored() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        config.runtime.reasoning_enabled = Some(false);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_REASONING_ENABLED", "maybe") };
        config.apply_env_overrides();
        assert_eq!(config.runtime.reasoning_enabled, Some(false));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_REASONING_ENABLED") };
    }

    #[test]
    async fn env_override_reasoning_effort() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        assert_eq!(config.runtime.reasoning_effort, None);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_REASONING_EFFORT", "HIGH") };
        config.apply_env_overrides();
        assert_eq!(config.runtime.reasoning_effort.as_deref(), Some("high"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_REASONING_EFFORT") };
    }

    #[test]
    async fn env_override_reasoning_effort_legacy_codex_env() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_CODEX_REASONING_EFFORT", "minimal") };
        config.apply_env_overrides();
        assert_eq!(config.runtime.reasoning_effort.as_deref(), Some("minimal"));

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_CODEX_REASONING_EFFORT") };
    }

    #[test]
    async fn env_override_invalid_port_ignored() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        let original_port = config.gateway.port;

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("PORT", "not_a_number") };
        config.apply_env_overrides();
        assert_eq!(config.gateway.port, original_port);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("PORT") };
    }

    #[test]
    async fn env_override_web_search_config() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("WEB_SEARCH_ENABLED", "false") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("WEB_SEARCH_PROVIDER", "brave") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("WEB_SEARCH_MAX_RESULTS", "7") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("WEB_SEARCH_TIMEOUT_SECS", "20") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("BRAVE_API_KEY", "brave-test-key") };

        config.apply_env_overrides();

        assert!(!config.web_search.enabled);
        assert_eq!(config.web_search.provider, "brave");
        assert_eq!(config.web_search.max_results, 7);
        assert_eq!(config.web_search.timeout_secs, 20);
        assert_eq!(
            config.web_search.brave_api_key.as_deref(),
            Some("brave-test-key")
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("WEB_SEARCH_ENABLED") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("WEB_SEARCH_PROVIDER") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("WEB_SEARCH_MAX_RESULTS") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("WEB_SEARCH_TIMEOUT_SECS") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("BRAVE_API_KEY") };
    }

    #[test]
    async fn env_override_web_search_invalid_values_ignored() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();
        let original_max_results = config.web_search.max_results;
        let original_timeout = config.web_search.timeout_secs;

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("WEB_SEARCH_MAX_RESULTS", "99") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("WEB_SEARCH_TIMEOUT_SECS", "0") };

        config.apply_env_overrides();

        assert_eq!(config.web_search.max_results, original_max_results);
        assert_eq!(config.web_search.timeout_secs, original_timeout);

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("WEB_SEARCH_MAX_RESULTS") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("WEB_SEARCH_TIMEOUT_SECS") };
    }

    #[test]
    async fn env_override_storage_provider_config() {
        let _env_guard = env_override_lock().await;
        let mut config = Config::default();

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_STORAGE_PROVIDER", "qdrant") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_STORAGE_DB_URL", "http://localhost:6333") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_STORAGE_CONNECT_TIMEOUT_SECS", "15") };

        config.apply_env_overrides();

        assert_eq!(config.storage.provider.config.provider, "qdrant");
        assert_eq!(
            config.storage.provider.config.db_url.as_deref(),
            Some("http://localhost:6333")
        );
        assert_eq!(
            config.storage.provider.config.connect_timeout_secs,
            Some(15)
        );

        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_STORAGE_PROVIDER") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_STORAGE_DB_URL") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::remove_var("ZEROCLAW_STORAGE_CONNECT_TIMEOUT_SECS") };
    }

    #[test]
    async fn proxy_config_scope_services_requires_entries_when_enabled() {
        let proxy = ProxyConfig {
            enabled: true,
            http_proxy: Some("http://127.0.0.1:7890".into()),
            https_proxy: None,
            all_proxy: None,
            no_proxy: Vec::new(),
            scope: ProxyScope::Services,
            services: Vec::new(),
        };

        let error = proxy.validate().unwrap_err().to_string();
        assert!(error.contains("proxy.scope='services'"));
    }

    #[test]
    async fn env_override_proxy_scope_services() {
        let _env_guard = env_override_lock().await;
        clear_proxy_env_test_vars();

        let mut config = Config::default();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_PROXY_ENABLED", "true") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_HTTP_PROXY", "http://127.0.0.1:7890") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe {
            std::env::set_var(
                "ZEROCLAW_PROXY_SERVICES",
                "provider.openai, tool.http_request",
            );
        }
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_PROXY_SCOPE", "services") };

        config.apply_env_overrides();

        assert!(config.proxy.enabled);
        assert_eq!(config.proxy.scope, ProxyScope::Services);
        assert_eq!(
            config.proxy.http_proxy.as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert!(config.proxy.should_apply_to_service("provider.openai"));
        assert!(config.proxy.should_apply_to_service("tool.http_request"));
        assert!(!config.proxy.should_apply_to_service("provider.anthropic"));

        clear_proxy_env_test_vars();
    }

    #[test]
    async fn env_override_proxy_scope_environment_applies_process_env() {
        let _env_guard = env_override_lock().await;
        clear_proxy_env_test_vars();

        let mut config = Config::default();
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_PROXY_ENABLED", "true") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_PROXY_SCOPE", "environment") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_HTTP_PROXY", "http://127.0.0.1:7890") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_HTTPS_PROXY", "http://127.0.0.1:7891") };
        // SAFETY: test-only, single-threaded test runner.
        unsafe { std::env::set_var("ZEROCLAW_NO_PROXY", "localhost,127.0.0.1") };

        config.apply_env_overrides();

        assert_eq!(config.proxy.scope, ProxyScope::Environment);
        assert_eq!(
            std::env::var("HTTP_PROXY").ok().as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(
            std::env::var("HTTPS_PROXY").ok().as_deref(),
            Some("http://127.0.0.1:7891")
        );
        assert!(
            std::env::var("NO_PROXY")
                .ok()
                .is_some_and(|value| value.contains("localhost"))
        );

        clear_proxy_env_test_vars();
    }

    #[test]
    async fn google_workspace_allowed_operations_require_methods() {
        let mut config = Config::default();
        config.google_workspace.allowed_operations = vec![GoogleWorkspaceAllowedOperation {
            service: "gmail".into(),
            resource: "users".into(),
            sub_resource: Some("drafts".into()),
            methods: Vec::new(),
        }];

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("google_workspace.allowed_operations[0].methods"));
    }

    #[test]
    async fn google_workspace_allowed_operations_reject_duplicate_service_resource_sub_resource_entries()
     {
        let mut config = Config::default();
        config.google_workspace.allowed_operations = vec![
            GoogleWorkspaceAllowedOperation {
                service: "gmail".into(),
                resource: "users".into(),
                sub_resource: Some("drafts".into()),
                methods: vec!["create".into()],
            },
            GoogleWorkspaceAllowedOperation {
                service: "gmail".into(),
                resource: "users".into(),
                sub_resource: Some("drafts".into()),
                methods: vec!["update".into()],
            },
        ];

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("duplicate service/resource/sub_resource entry"));
    }

    #[test]
    async fn google_workspace_allowed_operations_allow_same_resource_different_sub_resource() {
        let mut config = Config::default();
        config.google_workspace.allowed_operations = vec![
            GoogleWorkspaceAllowedOperation {
                service: "gmail".into(),
                resource: "users".into(),
                sub_resource: Some("messages".into()),
                methods: vec!["list".into(), "get".into()],
            },
            GoogleWorkspaceAllowedOperation {
                service: "gmail".into(),
                resource: "users".into(),
                sub_resource: Some("drafts".into()),
                methods: vec!["create".into(), "update".into()],
            },
        ];

        assert!(config.validate().is_ok());
    }

    #[test]
    async fn google_workspace_allowed_operations_reject_duplicate_methods_within_entry() {
        let mut config = Config::default();
        config.google_workspace.allowed_operations = vec![GoogleWorkspaceAllowedOperation {
            service: "gmail".into(),
            resource: "users".into(),
            sub_resource: Some("drafts".into()),
            methods: vec!["create".into(), "create".into()],
        }];

        let err = config.validate().unwrap_err().to_string();
        assert!(
            err.contains("duplicate entry"),
            "expected duplicate entry error, got: {err}"
        );
    }

    #[test]
    async fn google_workspace_allowed_operations_accept_valid_entries() {
        let mut config = Config::default();
        config.google_workspace.allowed_operations = vec![
            GoogleWorkspaceAllowedOperation {
                service: "gmail".into(),
                resource: "users".into(),
                sub_resource: Some("messages".into()),
                methods: vec!["list".into(), "get".into()],
            },
            GoogleWorkspaceAllowedOperation {
                service: "drive".into(),
                resource: "files".into(),
                sub_resource: None,
                methods: vec!["list".into(), "get".into()],
            },
        ];

        assert!(config.validate().is_ok());
    }

    #[test]
    async fn google_workspace_allowed_operations_reject_invalid_sub_resource_characters() {
        let mut config = Config::default();
        config.google_workspace.allowed_operations = vec![GoogleWorkspaceAllowedOperation {
            service: "gmail".into(),
            resource: "users".into(),
            sub_resource: Some("bad resource!".into()),
            methods: vec!["list".into()],
        }];

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("sub_resource contains invalid characters"));
    }

    fn runtime_proxy_cache_contains(cache_key: &str) -> bool {
        match runtime_proxy_client_cache().read() {
            Ok(guard) => guard.contains_key(cache_key),
            Err(poisoned) => poisoned.into_inner().contains_key(cache_key),
        }
    }

    #[test]
    async fn runtime_proxy_client_cache_reuses_default_profile_key() {
        let service_key = format!(
            "provider.cache_test.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let cache_key = runtime_proxy_cache_key(&service_key, None, None);

        clear_runtime_proxy_client_cache();
        assert!(!runtime_proxy_cache_contains(&cache_key));

        let _ = build_runtime_proxy_client(&service_key);
        assert!(runtime_proxy_cache_contains(&cache_key));

        let _ = build_runtime_proxy_client(&service_key);
        assert!(runtime_proxy_cache_contains(&cache_key));
    }

    #[test]
    async fn set_runtime_proxy_config_clears_runtime_proxy_client_cache() {
        let service_key = format!(
            "provider.cache_timeout_test.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let cache_key = runtime_proxy_cache_key(&service_key, Some(30), Some(5));

        clear_runtime_proxy_client_cache();
        let _ = build_runtime_proxy_client_with_timeouts(&service_key, 30, 5);
        assert!(runtime_proxy_cache_contains(&cache_key));

        set_runtime_proxy_config(ProxyConfig::default());
        assert!(!runtime_proxy_cache_contains(&cache_key));
    }

    #[test]
    async fn gateway_config_default_values() {
        let g = GatewayConfig::default();
        assert_eq!(g.port, 42617);
        assert_eq!(g.host, "127.0.0.1");
        assert!(g.require_pairing);
        assert!(!g.allow_public_bind);
        assert!(g.paired_tokens.is_empty());
        assert!(!g.trust_forwarded_headers);
        assert_eq!(g.rate_limit_max_keys, 10_000);
        assert_eq!(g.idempotency_max_keys, 10_000);
    }

    // ── LINE ──────────────────────────────────────────────────

    // ── Config file permission hardening (Unix only) ───────────────

    #[cfg(unix)]
    #[test]
    async fn new_config_file_has_restricted_permissions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        // Create a config and save it
        let mut config = Config::default();
        config.config_path = config_path.clone();
        config.save().await.unwrap();

        let meta = fs::metadata(&config_path).await.unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "New config file should be owner-only (0600), got {mode:o}"
        );
    }

    #[cfg(unix)]
    #[test]
    async fn save_restricts_existing_world_readable_config_to_owner_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let mut config = Config::default();
        config.config_path = config_path.clone();
        config.save().await.unwrap();

        // Simulate the regression state observed in issue #1345.
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let loose_mode = std::fs::metadata(&config_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            loose_mode, 0o644,
            "test setup requires world-readable config"
        );

        config.default_temperature = 0.6;
        config.save().await.unwrap();

        let hardened_mode = std::fs::metadata(&config_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            hardened_mode, 0o600,
            "Saving config should restore owner-only permissions (0600)"
        );
    }

    #[cfg(unix)]
    #[test]
    async fn world_readable_config_is_detectable() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        // Create a config file with intentionally loose permissions
        std::fs::write(&config_path, "# test config").unwrap();
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let meta = std::fs::metadata(&config_path).unwrap();
        let mode = meta.permissions().mode();
        assert!(
            mode & 0o004 != 0,
            "Test setup: file should be world-readable (mode {mode:o})"
        );
    }

    #[test]
    async fn transcription_config_defaults() {
        let tc = TranscriptionConfig::default();
        assert!(!tc.enabled);
        assert!(tc.api_url.contains("groq.com"));
        assert_eq!(tc.model, "whisper-large-v3-turbo");
        assert!(tc.language.is_none());
        assert_eq!(tc.max_duration_secs, 120);
        assert!(!tc.transcribe_non_ptt_audio);
    }

    #[test]
    async fn config_roundtrip_with_transcription() {
        let mut config = Config::default();
        config.transcription.enabled = true;
        config.transcription.language = Some("en".into());

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed = parse_test_config(&toml_str);

        assert!(parsed.transcription.enabled);
        assert_eq!(parsed.transcription.language.as_deref(), Some("en"));
        assert_eq!(parsed.transcription.model, "whisper-large-v3-turbo");
    }

    #[test]
    async fn config_without_transcription_uses_defaults() {
        let toml_str = r#"
            default_provider = "openrouter"
            default_model = "test-model"
            default_temperature = 0.7
        "#;
        let parsed = parse_test_config(toml_str);
        assert!(!parsed.transcription.enabled);
        assert_eq!(parsed.transcription.max_duration_secs, 120);
    }

    #[test]
    async fn security_defaults_are_backward_compatible() {
        let parsed = parse_test_config(
            r#"
default_provider = "openrouter"
default_model = "anthropic/claude-sonnet-4.6"
default_temperature = 0.7
"#,
        );

        assert!(!parsed.security.otp.enabled);
        assert_eq!(parsed.security.otp.method, OtpMethod::Totp);
        assert!(!parsed.security.estop.enabled);
        assert!(parsed.security.estop.require_otp_to_resume);
    }

    #[test]
    async fn security_toml_parses_otp_and_estop_sections() {
        let parsed = parse_test_config(
            r#"
default_provider = "openrouter"
default_model = "anthropic/claude-sonnet-4.6"
default_temperature = 0.7

[security.otp]
enabled = true
method = "totp"
token_ttl_secs = 30
cache_valid_secs = 120
gated_actions = ["shell", "browser_open"]
gated_domains = ["*.chase.com", "accounts.google.com"]
gated_domain_categories = ["banking"]

[security.estop]
enabled = true
state_file = "~/.naraeclaw/estop-state.json"
require_otp_to_resume = true
"#,
        );

        assert!(parsed.security.otp.enabled);
        assert!(parsed.security.estop.enabled);
        assert_eq!(parsed.security.otp.gated_actions.len(), 2);
        assert_eq!(parsed.security.otp.gated_domains.len(), 2);
        parsed.validate().unwrap();
    }

    #[test]
    async fn security_validation_rejects_invalid_domain_glob() {
        let mut config = Config::default();
        config.security.otp.gated_domains = vec!["bad domain.com".into()];

        let err = config.validate().expect_err("expected invalid domain glob");
        assert!(err.to_string().contains("gated_domains"));
    }

    #[test]
    async fn validate_accepts_local_whisper_as_transcription_default_provider() {
        let mut config = Config::default();
        config.transcription.default_provider = "local_whisper".to_string();

        config.validate().expect(
            "local_whisper must be accepted by the transcription.default_provider allowlist",
        );
    }

    #[test]
    async fn validate_rejects_unknown_transcription_default_provider() {
        let mut config = Config::default();
        config.transcription.default_provider = "unknown_stt".to_string();

        let err = config
            .validate()
            .expect_err("expected validation to reject unknown transcription provider");
        assert!(
            err.to_string().contains("transcription.default_provider"),
            "got: {err}"
        );
    }

    #[test]
    async fn security_validation_rejects_unknown_domain_category() {
        let mut config = Config::default();
        config.security.otp.gated_domain_categories = vec!["not_real".into()];

        let err = config
            .validate()
            .expect_err("expected unknown domain category");
        assert!(err.to_string().contains("gated_domain_categories"));
    }

    #[test]
    async fn security_validation_rejects_zero_token_ttl() {
        let mut config = Config::default();
        config.security.otp.token_ttl_secs = 0;

        let err = config
            .validate()
            .expect_err("expected ttl validation failure");
        assert!(err.to_string().contains("token_ttl_secs"));
    }

    // ── MCP config validation ─────────────────────────────────────────────

    fn stdio_server(name: &str, command: &str) -> McpServerConfig {
        let mut config = McpServerConfig::default();
        config.name = name.to_string();
        config.transport = McpTransport::Stdio;
        config.command = command.to_string();
        config
    }

    fn http_server(name: &str, url: &str) -> McpServerConfig {
        let mut config = McpServerConfig::default();
        config.name = name.to_string();
        config.transport = McpTransport::Http;
        config.url = Some(url.to_string());
        config
    }

    fn sse_server(name: &str, url: &str) -> McpServerConfig {
        let mut config = McpServerConfig::default();
        config.name = name.to_string();
        config.transport = McpTransport::Sse;
        config.url = Some(url.to_string());
        config
    }

    #[test]
    async fn validate_mcp_config_empty_servers_ok() {
        let cfg = McpConfig::default();
        assert!(validate_mcp_config(&cfg).is_ok());
    }

    #[test]
    async fn validate_mcp_config_valid_stdio_ok() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![stdio_server("fs", "/usr/bin/mcp-fs")];
        assert!(validate_mcp_config(&cfg).is_ok());
    }

    #[test]
    async fn validate_mcp_config_valid_http_ok() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![http_server("svc", "http://localhost:8080/mcp")];
        assert!(validate_mcp_config(&cfg).is_ok());
    }

    #[test]
    async fn validate_mcp_config_valid_sse_ok() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![sse_server("svc", "https://example.com/events")];
        assert!(validate_mcp_config(&cfg).is_ok());
    }

    #[test]
    async fn validate_mcp_config_rejects_empty_name() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![stdio_server("", "/usr/bin/tool")];
        let err = validate_mcp_config(&cfg).expect_err("empty name should fail");
        assert!(
            err.to_string().contains("name must not be empty"),
            "got: {err}"
        );
    }

    #[test]
    async fn validate_mcp_config_rejects_whitespace_name() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![stdio_server("   ", "/usr/bin/tool")];
        let err = validate_mcp_config(&cfg).expect_err("whitespace name should fail");
        assert!(
            err.to_string().contains("name must not be empty"),
            "got: {err}"
        );
    }

    #[test]
    async fn validate_mcp_config_rejects_duplicate_names() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![
            stdio_server("fs", "/usr/bin/mcp-a"),
            stdio_server("fs", "/usr/bin/mcp-b"),
        ];
        let err = validate_mcp_config(&cfg).expect_err("duplicate name should fail");
        assert!(err.to_string().contains("duplicate name"), "got: {err}");
    }

    #[test]
    async fn validate_mcp_config_rejects_zero_timeout() {
        let mut server = stdio_server("fs", "/usr/bin/mcp-fs");
        server.tool_timeout_secs = Some(0);
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![server];
        let err = validate_mcp_config(&cfg).expect_err("zero timeout should fail");
        assert!(err.to_string().contains("greater than 0"), "got: {err}");
    }

    #[test]
    async fn validate_mcp_config_rejects_timeout_exceeding_max() {
        let mut server = stdio_server("fs", "/usr/bin/mcp-fs");
        server.tool_timeout_secs = Some(MCP_MAX_TOOL_TIMEOUT_SECS + 1);
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![server];
        let err = validate_mcp_config(&cfg).expect_err("oversized timeout should fail");
        assert!(err.to_string().contains("exceeds max"), "got: {err}");
    }

    #[test]
    async fn validate_mcp_config_allows_max_timeout_exactly() {
        let mut server = stdio_server("fs", "/usr/bin/mcp-fs");
        server.tool_timeout_secs = Some(MCP_MAX_TOOL_TIMEOUT_SECS);
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![server];
        assert!(validate_mcp_config(&cfg).is_ok());
    }

    #[test]
    async fn validate_mcp_config_rejects_stdio_with_empty_command() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![stdio_server("fs", "")];
        let err = validate_mcp_config(&cfg).expect_err("empty command should fail");
        assert!(
            err.to_string().contains("requires non-empty command"),
            "got: {err}"
        );
    }

    #[test]
    async fn validate_mcp_config_rejects_http_without_url() {
        let mut server = McpServerConfig::default();
        server.name = "svc".to_string();
        server.transport = McpTransport::Http;
        server.url = None;
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![server];
        let err = validate_mcp_config(&cfg).expect_err("http without url should fail");
        assert!(err.to_string().contains("requires url"), "got: {err}");
    }

    #[test]
    async fn validate_mcp_config_rejects_sse_without_url() {
        let mut server = McpServerConfig::default();
        server.name = "svc".to_string();
        server.transport = McpTransport::Sse;
        server.url = None;
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![server];
        let err = validate_mcp_config(&cfg).expect_err("sse without url should fail");
        assert!(err.to_string().contains("requires url"), "got: {err}");
    }

    #[test]
    async fn validate_mcp_config_rejects_non_http_scheme() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![http_server("svc", "ftp://example.com/mcp")];
        let err = validate_mcp_config(&cfg).expect_err("non-http scheme should fail");
        assert!(err.to_string().contains("http/https"), "got: {err}");
    }

    #[test]
    async fn validate_mcp_config_rejects_invalid_url() {
        let mut cfg = McpConfig::default();
        cfg.enabled = true;
        cfg.servers = vec![http_server("svc", "not a url at all !!!")];
        let err = validate_mcp_config(&cfg).expect_err("invalid url should fail");
        assert!(err.to_string().contains("valid URL"), "got: {err}");
    }

    #[test]
    async fn mcp_config_default_disabled_with_empty_servers() {
        let cfg = McpConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.servers.is_empty());
    }

    #[test]
    async fn mcp_transport_serde_roundtrip_lowercase() {
        let cases = [
            (McpTransport::Stdio, "\"stdio\""),
            (McpTransport::Http, "\"http\""),
            (McpTransport::Sse, "\"sse\""),
        ];
        for (variant, expected_json) in &cases {
            let serialized = serde_json::to_string(variant).expect("serialize");
            assert_eq!(&serialized, expected_json, "variant: {variant:?}");
            let deserialized: McpTransport =
                serde_json::from_str(expected_json).expect("deserialize");
            assert_eq!(&deserialized, variant);
        }
    }

    #[test]
    async fn swarm_strategy_roundtrip() {
        let cases = vec![
            (SwarmStrategy::Sequential, "\"sequential\""),
            (SwarmStrategy::Parallel, "\"parallel\""),
            (SwarmStrategy::Router, "\"router\""),
        ];
        for (variant, expected_json) in &cases {
            let serialized = serde_json::to_string(variant).expect("serialize");
            assert_eq!(&serialized, expected_json, "variant: {variant:?}");
            let deserialized: SwarmStrategy =
                serde_json::from_str(expected_json).expect("deserialize");
            assert_eq!(&deserialized, variant);
        }
    }

    #[test]
    async fn swarm_config_deserializes_with_defaults() {
        let toml_str = r#"
            agents = ["researcher", "writer"]
            strategy = "sequential"
        "#;
        let config: SwarmConfig = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(config.agents, vec!["researcher", "writer"]);
        assert_eq!(config.strategy, SwarmStrategy::Sequential);
        assert!(config.router_prompt.is_none());
        assert!(config.description.is_none());
        assert_eq!(config.timeout_secs, 300);
    }

    #[test]
    async fn swarm_config_deserializes_full() {
        let toml_str = r#"
            agents = ["a", "b", "c"]
            strategy = "router"
            router_prompt = "Pick the best."
            description = "Multi-agent router"
            timeout_secs = 120
        "#;
        let config: SwarmConfig = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(config.agents.len(), 3);
        assert_eq!(config.strategy, SwarmStrategy::Router);
        assert_eq!(config.router_prompt.as_deref(), Some("Pick the best."));
        assert_eq!(config.description.as_deref(), Some("Multi-agent router"));
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    async fn config_with_swarms_section_deserializes() {
        let toml_str = r#"
            [agents.researcher]
            provider = "ollama"
            model = "llama3"

            [agents.writer]
            provider = "openrouter"
            model = "claude-sonnet"

            [swarms.pipeline]
            agents = ["researcher", "writer"]
            strategy = "sequential"
        "#;
        let config = parse_test_config(toml_str);
        assert_eq!(config.agents.len(), 2);
        assert_eq!(config.swarms.len(), 1);
        assert!(config.swarms.contains_key("pipeline"));
    }

    #[tokio::test]
    async fn nevis_client_secret_encrypt_decrypt_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "zeroclaw_test_nevis_secret_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).await.unwrap();

        let plaintext_secret = "nevis-test-client-secret-value";

        let mut config = Config::default();
        config.workspace_dir = dir.join("workspace");
        config.config_path = dir.join("config.toml");
        config.security.nevis.client_secret = Some(plaintext_secret.into());

        // Save (triggers encryption)
        config.save().await.unwrap();

        // Read raw TOML and verify plaintext secret is NOT present
        let raw_toml = tokio::fs::read_to_string(&config.config_path)
            .await
            .unwrap();
        assert!(
            !raw_toml.contains(plaintext_secret),
            "Saved TOML must not contain the plaintext client_secret"
        );

        // Parse stored TOML and verify the value is encrypted
        let stored: Config = toml::from_str(&raw_toml).unwrap();
        let stored_secret = stored.security.nevis.client_secret.as_ref().unwrap();
        assert!(
            crate::secrets::SecretStore::is_encrypted(stored_secret),
            "Stored client_secret must be marked as encrypted"
        );

        // Decrypt and verify it matches the original plaintext
        let store = crate::secrets::SecretStore::new(&dir, true);
        assert_eq!(store.decrypt(stored_secret).unwrap(), plaintext_secret);

        // Simulate a full load: deserialize then decrypt (mirrors load_or_init logic)
        let mut loaded: Config = toml::from_str(&raw_toml).unwrap();
        loaded.config_path = dir.join("config.toml");
        let load_store = crate::secrets::SecretStore::new(&dir, loaded.secrets.encrypt);
        loaded.decrypt_secrets(&load_store).unwrap();
        assert_eq!(
            loaded.security.nevis.client_secret.as_deref().unwrap(),
            plaintext_secret,
            "Loaded client_secret must match the original plaintext after decryption"
        );

        let _ = fs::remove_dir_all(&dir).await;
    }

    // ══════════════════════════════════════════════════════════
    // Nevis config validation tests
    // ══════════════════════════════════════════════════════════

    #[test]
    async fn nevis_config_validate_disabled_accepts_empty_fields() {
        let cfg = NevisConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    async fn nevis_config_validate_rejects_empty_instance_url() {
        let mut cfg = NevisConfig::default();
        cfg.enabled = true;
        cfg.instance_url = String::new();
        cfg.client_id = "test-client".into();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("instance_url"));
    }

    #[test]
    async fn nevis_config_validate_rejects_empty_client_id() {
        let mut cfg = NevisConfig::default();
        cfg.enabled = true;
        cfg.instance_url = "https://nevis.example.com".into();
        cfg.client_id = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("client_id"));
    }

    #[test]
    async fn nevis_config_validate_rejects_empty_realm() {
        let mut cfg = NevisConfig::default();
        cfg.enabled = true;
        cfg.instance_url = "https://nevis.example.com".into();
        cfg.client_id = "test-client".into();
        cfg.realm = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("realm"));
    }

    #[test]
    async fn nevis_config_validate_rejects_local_without_jwks() {
        let mut cfg = NevisConfig::default();
        cfg.enabled = true;
        cfg.instance_url = "https://nevis.example.com".into();
        cfg.client_id = "test-client".into();
        cfg.token_validation = "local".into();
        cfg.jwks_url = None;
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("jwks_url"));
    }

    #[test]
    async fn nevis_config_validate_rejects_zero_session_timeout() {
        let mut cfg = NevisConfig::default();
        cfg.enabled = true;
        cfg.instance_url = "https://nevis.example.com".into();
        cfg.client_id = "test-client".into();
        cfg.token_validation = "remote".into();
        cfg.session_timeout_secs = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("session_timeout_secs"));
    }

    #[test]
    async fn nevis_config_validate_accepts_valid_enabled_config() {
        let mut cfg = NevisConfig::default();
        cfg.enabled = true;
        cfg.instance_url = "https://nevis.example.com".into();
        cfg.realm = "master".into();
        cfg.client_id = "test-client".into();
        cfg.token_validation = "remote".into();
        cfg.session_timeout_secs = 3600;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    async fn nevis_config_validate_rejects_invalid_token_validation() {
        let mut cfg = NevisConfig::default();
        cfg.enabled = true;
        cfg.instance_url = "https://nevis.example.com".into();
        cfg.realm = "master".into();
        cfg.client_id = "test-client".into();
        cfg.token_validation = "invalid_mode".into();
        cfg.session_timeout_secs = 3600;
        let err = cfg.validate().unwrap_err();
        assert!(
            err.contains("invalid value 'invalid_mode'"),
            "Expected invalid token_validation error, got: {err}"
        );
    }

    #[test]
    async fn nevis_config_debug_redacts_client_secret() {
        let mut cfg = NevisConfig::default();
        cfg.client_secret = Some("super-secret".into());
        let debug_output = format!("{:?}", cfg);
        assert!(
            !debug_output.contains("super-secret"),
            "Debug output must not contain the raw client_secret"
        );
        assert!(
            debug_output.contains("[REDACTED]"),
            "Debug output must show [REDACTED] for client_secret"
        );
    }

    #[test]
    async fn google_workspace_allowed_operations_deserialize_from_toml() {
        let toml_str = r#"
            enabled = true

            [[allowed_operations]]
            service = "gmail"
            resource = "users"
            sub_resource = "drafts"
            methods = ["create", "update"]
        "#;

        let cfg: GoogleWorkspaceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.allowed_operations.len(), 1);
        assert_eq!(cfg.allowed_operations[0].service, "gmail");
        assert_eq!(cfg.allowed_operations[0].resource, "users");
        assert_eq!(
            cfg.allowed_operations[0].sub_resource.as_deref(),
            Some("drafts")
        );
        assert_eq!(
            cfg.allowed_operations[0].methods,
            vec!["create".to_string(), "update".to_string()]
        );
    }

    #[test]
    async fn google_workspace_allowed_operations_deserialize_without_sub_resource() {
        let toml_str = r#"
            enabled = true

            [[allowed_operations]]
            service = "drive"
            resource = "files"
            methods = ["list", "get"]
        "#;

        let cfg: GoogleWorkspaceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.allowed_operations[0].sub_resource, None);
    }

    #[test]
    async fn config_validate_accepts_google_workspace_allowed_operations() {
        let mut cfg = Config::default();
        cfg.google_workspace.enabled = true;
        cfg.google_workspace.allowed_services = vec!["gmail".into()];
        cfg.google_workspace.allowed_operations = vec![GoogleWorkspaceAllowedOperation {
            service: "gmail".into(),
            resource: "users".into(),
            sub_resource: Some("drafts".into()),
            methods: vec!["create".into(), "update".into()],
        }];

        cfg.validate().unwrap();
    }

    #[test]
    async fn config_validate_rejects_duplicate_google_workspace_allowed_operations() {
        let mut cfg = Config::default();
        cfg.google_workspace.enabled = true;
        cfg.google_workspace.allowed_services = vec!["gmail".into()];
        cfg.google_workspace.allowed_operations = vec![
            GoogleWorkspaceAllowedOperation {
                service: "gmail".into(),
                resource: "users".into(),
                sub_resource: Some("drafts".into()),
                methods: vec!["create".into()],
            },
            GoogleWorkspaceAllowedOperation {
                service: "gmail".into(),
                resource: "users".into(),
                sub_resource: Some("drafts".into()),
                methods: vec!["update".into()],
            },
        ];

        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("duplicate service/resource/sub_resource entry"));
    }

    #[test]
    async fn config_validate_rejects_operation_service_not_in_allowed_services() {
        let mut cfg = Config::default();
        cfg.google_workspace.enabled = true;
        cfg.google_workspace.allowed_services = vec!["gmail".into()];
        cfg.google_workspace.allowed_operations = vec![GoogleWorkspaceAllowedOperation {
            service: "drive".into(), // drive is not in allowed_services
            resource: "files".into(),
            sub_resource: None,
            methods: vec!["list".into()],
        }];

        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("not in the effective allowed_services"),
            "expected not-in-allowed_services error, got: {err}"
        );
    }

    #[test]
    async fn config_validate_accepts_default_service_when_allowed_services_empty() {
        // When allowed_services is empty the validator uses DEFAULT_GWS_SERVICES.
        // A known default service must pass.
        let mut cfg = Config::default();
        cfg.google_workspace.enabled = true;
        // allowed_services deliberately left empty (falls back to defaults)
        cfg.google_workspace.allowed_operations = vec![GoogleWorkspaceAllowedOperation {
            service: "drive".into(),
            resource: "files".into(),
            sub_resource: None,
            methods: vec!["list".into()],
        }];

        assert!(cfg.validate().is_ok());
    }

    #[test]
    async fn config_validate_rejects_unknown_service_when_allowed_services_empty() {
        // Even with allowed_services empty (using defaults), an operation whose
        // service is not in DEFAULT_GWS_SERVICES must fail validation — not silently
        // pass through to be rejected at runtime.
        let mut cfg = Config::default();
        cfg.google_workspace.enabled = true;
        // allowed_services deliberately left empty
        cfg.google_workspace.allowed_operations = vec![GoogleWorkspaceAllowedOperation {
            service: "not_a_real_service".into(),
            resource: "files".into(),
            sub_resource: None,
            methods: vec!["list".into()],
        }];

        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("not in the effective allowed_services"),
            "expected effective-allowed_services error, got: {err}"
        );
    }

    // ── Bootstrap files ─────────────────────────────────────

    #[tokio::test]
    async fn ensure_bootstrap_files_creates_missing_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ws = tmp.path().join("workspace");
        let _: () = tokio::fs::create_dir_all(&ws).await.unwrap();

        ensure_bootstrap_files(&ws).await.unwrap();

        let soul: String = tokio::fs::read_to_string(ws.join("SOUL.md")).await.unwrap();
        let identity: String = tokio::fs::read_to_string(ws.join("IDENTITY.md"))
            .await
            .unwrap();
        assert!(soul.contains("SOUL.md"));
        assert!(identity.contains("IDENTITY.md"));
    }

    #[tokio::test]
    async fn ensure_bootstrap_files_does_not_overwrite_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ws = tmp.path().join("workspace");
        let _: () = tokio::fs::create_dir_all(&ws).await.unwrap();

        let custom = "# My custom SOUL";
        let _: () = tokio::fs::write(ws.join("SOUL.md"), custom).await.unwrap();

        ensure_bootstrap_files(&ws).await.unwrap();

        let soul: String = tokio::fs::read_to_string(ws.join("SOUL.md")).await.unwrap();
        assert_eq!(
            soul, custom,
            "ensure_bootstrap_files must not overwrite existing files"
        );

        // IDENTITY.md should still be created since it was missing
        let identity: String = tokio::fs::read_to_string(ws.join("IDENTITY.md"))
            .await
            .unwrap();
        assert!(identity.contains("IDENTITY.md"));
    }

    // ── PacingConfig serde defaults ─────────────────────────────

    #[test]
    async fn pacing_config_serde_defaults_match_manual_default() {
        // Deserialise an empty TOML table and verify the loop-detection
        // fields receive the same defaults as `PacingConfig::default()`.
        let from_toml: PacingConfig = toml::from_str("").unwrap();
        let manual = PacingConfig::default();

        assert_eq!(
            from_toml.loop_detection_enabled,
            manual.loop_detection_enabled
        );
        assert_eq!(
            from_toml.loop_detection_window_size,
            manual.loop_detection_window_size
        );
        assert_eq!(
            from_toml.loop_detection_max_repeats,
            manual.loop_detection_max_repeats
        );

        // Verify concrete values so a silent change to the defaults is caught.
        assert!(from_toml.loop_detection_enabled, "default should be true");
        assert_eq!(from_toml.loop_detection_window_size, 20);
        assert_eq!(from_toml.loop_detection_max_repeats, 3);
    }

    // ── Docker baked config template ────────────────────────────

    /// The TOML template baked into Docker images.
    /// Kept here so Docker config changes can be validated by `cargo test`.
    const DOCKER_CONFIG_TEMPLATE: &str = r#"
workspace_dir = "/naraeclaw-data/workspace"
config_path = "/naraeclaw-data/.naraeclaw/config.toml"
api_key = ""
default_provider = "openrouter"
default_model = "anthropic/claude-sonnet-4-20250514"
default_temperature = 0.7

[gateway]
port = 42617
host = "[::]"
allow_public_bind = true

[autonomy]
level = "supervised"
auto_approve = ["file_read", "file_write", "file_edit", "memory_recall", "memory_store", "web_search_tool", "web_fetch", "calculator", "glob_search", "content_search", "image_info", "weather", "git_operations"]
"#;

    #[test]
    async fn docker_config_template_is_parseable() {
        let cfg: Config = toml::from_str(DOCKER_CONFIG_TEMPLATE)
            .expect("Docker baked config.toml must be valid TOML that deserialises into Config");

        // The [autonomy] section must be present and contain the expected tools.
        let auto = &cfg.autonomy.auto_approve;
        for tool in &[
            "file_read",
            "file_write",
            "file_edit",
            "memory_recall",
            "memory_store",
            "web_search_tool",
            "web_fetch",
            "calculator",
            "glob_search",
            "content_search",
            "image_info",
            "weather",
            "git_operations",
        ] {
            assert!(
                auto.iter().any(|t| t == tool),
                "Docker config auto_approve missing expected tool: {tool}"
            );
        }
    }

    #[test]
    async fn cost_enforcement_config_defaults() {
        let config = CostEnforcementConfig::default();
        assert_eq!(config.mode, "warn");
        assert_eq!(config.route_down_model, None);
        assert_eq!(config.reserve_percent, 10);
    }

    #[test]
    async fn cost_config_includes_enforcement() {
        let config = CostConfig::default();
        assert_eq!(config.enforcement.mode, "warn");
        assert_eq!(config.enforcement.reserve_percent, 10);
    }

    // ── Configurable macro tests ──

    #[test]
    async fn backfill_enabled_activates_channel_without_explicit_enabled() {
        let toml = r#"
[channels_config.slack]
app_token = "xapp-test"
bot_token = "xoxb-test"
"#;
        let mut config: Config = toml::from_str(toml).unwrap();
        assert!(!config.channels_config.slack.as_ref().unwrap().enabled);

        config.channels_config.backfill_enabled(toml);
        assert!(config.channels_config.slack.as_ref().unwrap().enabled);
    }

    #[test]
    async fn backfill_enabled_respects_explicit_false() {
        let toml = r#"
[channels_config.slack]
app_token = "xapp-test"
bot_token = "xoxb-test"
enabled = false
"#;
        let mut config: Config = toml::from_str(toml).unwrap();
        config.channels_config.backfill_enabled(toml);
        assert!(
            !config.channels_config.slack.as_ref().unwrap().enabled,
            "explicit enabled=false must not be overwritten"
        );
    }

    #[test]
    async fn backfill_enabled_no_op_when_section_absent() {
        let toml = r#"
api_key = "sk-test"
"#;
        let mut config: Config = toml::from_str(toml).unwrap();
        config.channels_config.backfill_enabled(toml);
        assert!(config.channels_config.slack.is_none());
    }

    #[test]
    async fn backfill_enabled_works_with_toml_comments() {
        let toml = r#"
# Slack Socket Mode 설정
[channels_config.slack]
app_token = "xapp-test"  # App-Level Token
bot_token = "xoxb-test"
# enabled intentionally omitted
"#;
        let mut config: Config = toml::from_str(toml).unwrap();
        assert!(!config.channels_config.slack.as_ref().unwrap().enabled);

        config.channels_config.backfill_enabled(toml);
        assert!(
            config.channels_config.slack.as_ref().unwrap().enabled,
            "backfill should activate channel even when config has comments"
        );
    }
}
