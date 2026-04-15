//! 프로바이더·전송 설정 — ModelProvider, TTS/STT, Transcription, MCP, Pacing, Reliability.
#![allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use zeroclaw_macros::Configurable;

/// Named provider profile definition compatible with Codex app-server style config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ModelProviderConfig {
    /// Optional provider type/name override (e.g. "openai", "openai-codex", or custom profile id).
    #[serde(default)]
    pub name: Option<String>,
    /// Optional base URL for OpenAI-compatible endpoints.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Optional custom API path suffix (e.g. "/v2/generate" instead of the
    /// default "/v1/chat/completions"). Only used by OpenAI-compatible / custom providers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_path: Option<String>,
    /// Provider protocol variant ("responses" or "chat_completions").
    #[serde(default)]
    pub wire_api: Option<String>,
    /// If true, load OpenAI auth material (OPENAI_API_KEY or ~/.codex/auth.json).
    #[serde(default)]
    pub requires_openai_auth: bool,
    /// Azure OpenAI resource name (e.g. "my-resource" in https://my-resource.openai.azure.com).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure_openai_resource: Option<String>,
    /// Azure OpenAI deployment name (e.g. "gpt-4o").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure_openai_deployment: Option<String>,
    /// Azure OpenAI API version (defaults to "2024-08-01-preview").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure_openai_api_version: Option<String>,
    /// Optional maximum output tokens to send in API requests.
    /// When set, overrides the provider's default `max_tokens` value.
    /// Useful for providers like OpenRouter where the platform default (65536)
    /// may exceed a model's actual limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// When true, all system messages are collected and prepended to the first
    /// user message instead of being sent with `role: system`. Native tool
    /// calling is preserved. Useful for local model servers (e.g. llama.cpp)
    /// whose chat template rejects system messages at non-first positions.
    #[serde(default)]
    pub merge_system_into_user: bool,
}

// ── Transcription ────────────────────────────────────────────────

fn default_transcription_api_url() -> String {
    "https://api.groq.com/openai/v1/audio/transcriptions".into()
}

fn default_transcription_model() -> String {
    "whisper-large-v3-turbo".into()
}

fn default_transcription_max_duration_secs() -> u64 {
    120
}

fn default_transcription_provider() -> String {
    "groq".into()
}

fn default_openai_stt_model() -> String {
    "whisper-1".into()
}

fn default_deepgram_stt_model() -> String {
    "nova-2".into()
}

fn default_google_stt_language_code() -> String {
    "en-US".into()
}

/// Voice transcription configuration with multi-provider support.
///
/// The top-level `api_url`, `model`, and `api_key` fields remain for backward
/// compatibility with existing Groq-based configurations.
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "transcription"]
pub struct TranscriptionConfig {
    /// Enable voice transcription for channels that support it.
    #[serde(default)]
    pub enabled: bool,
    /// Default STT provider: "groq", "openai", "deepgram", "assemblyai", "google".
    #[serde(default = "default_transcription_provider")]
    pub default_provider: String,
    /// API key used for transcription requests (Groq provider).
    ///
    /// If unset, runtime falls back to `GROQ_API_KEY` for backward compatibility.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
    /// Whisper API endpoint URL (Groq provider).
    #[serde(default = "default_transcription_api_url")]
    pub api_url: String,
    /// Whisper model name (Groq provider).
    #[serde(default = "default_transcription_model")]
    pub model: String,
    /// Optional language hint (ISO-639-1, e.g. "en", "ru") for Groq provider.
    #[serde(default)]
    pub language: Option<String>,
    /// Optional initial prompt to bias transcription toward expected vocabulary
    /// (proper nouns, technical terms, etc.). Sent as the `prompt` field in the
    /// Whisper API request.
    #[serde(default)]
    pub initial_prompt: Option<String>,
    /// Maximum voice duration in seconds (messages longer than this are skipped).
    #[serde(default = "default_transcription_max_duration_secs")]
    pub max_duration_secs: u64,
    /// OpenAI Whisper STT provider configuration.
    #[serde(default)]
    #[nested]
    pub openai: Option<OpenAiSttConfig>,
    /// Deepgram STT provider configuration.
    #[serde(default)]
    #[nested]
    pub deepgram: Option<DeepgramSttConfig>,
    /// AssemblyAI STT provider configuration.
    #[serde(default)]
    #[nested]
    pub assemblyai: Option<AssemblyAiSttConfig>,
    /// Google Cloud Speech-to-Text provider configuration.
    #[serde(default)]
    #[nested]
    pub google: Option<GoogleSttConfig>,
    /// Local/self-hosted Whisper-compatible STT provider.
    #[serde(default)]
    #[nested]
    pub local_whisper: Option<LocalWhisperConfig>,
    /// Also transcribe non-PTT (forwarded/regular) audio messages on WhatsApp,
    /// not just voice notes.  Default: `false` (preserves legacy behavior).
    #[serde(default)]
    pub transcribe_non_ptt_audio: bool,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_provider: default_transcription_provider(),
            api_key: None,
            api_url: default_transcription_api_url(),
            model: default_transcription_model(),
            language: None,
            initial_prompt: None,
            max_duration_secs: default_transcription_max_duration_secs(),
            openai: None,
            deepgram: None,
            assemblyai: None,
            google: None,
            local_whisper: None,
            transcribe_non_ptt_audio: false,
        }
    }
}

// ── MCP ─────────────────────────────────────────────────────────

/// Transport type for MCP server connections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    /// Spawn a local process and communicate over stdin/stdout.
    #[default]
    Stdio,
    /// Connect via HTTP POST.
    Http,
    /// Connect via HTTP + Server-Sent Events.
    Sse,
}

/// Configuration for a single external MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct McpServerConfig {
    /// Display name used as a tool prefix (`<server>__<tool>`).
    pub name: String,
    /// Transport type (default: stdio).
    #[serde(default)]
    pub transport: McpTransport,
    /// URL for HTTP/SSE transports.
    #[serde(default)]
    pub url: Option<String>,
    /// Executable to spawn for stdio transport.
    #[serde(default)]
    pub command: String,
    /// Command arguments for stdio transport.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional environment variables for stdio transport.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Optional HTTP headers for HTTP/SSE transports.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Optional per-call timeout in seconds (hard capped in validation).
    #[serde(default)]
    pub tool_timeout_secs: Option<u64>,
}

/// External MCP client configuration (`[mcp]` section).
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "mcp"]
pub struct McpConfig {
    /// Enable MCP tool loading.
    #[serde(default)]
    pub enabled: bool,
    /// Load MCP tool schemas on-demand via `tool_search` instead of eagerly
    /// including them in the LLM context window. When `true` (the default),
    /// only tool names are listed in the system prompt; the LLM must call
    /// `tool_search` to fetch full schemas before invoking a deferred tool.
    #[serde(default = "default_deferred_loading")]
    pub deferred_loading: bool,
    /// Configured MCP servers.
    #[serde(default, alias = "mcpServers")]
    pub servers: Vec<McpServerConfig>,
}

fn default_deferred_loading() -> bool {
    true
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            deferred_loading: default_deferred_loading(),
            servers: Vec::new(),
        }
    }
}

/// Verifiable Intent (VI) credential verification and issuance (`[verifiable_intent]` section).
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "verifiable-intent"]
pub struct VerifiableIntentConfig {
    /// Enable VI credential verification on commerce tool calls (default: false).
    #[serde(default)]
    pub enabled: bool,

    /// Strictness mode for constraint evaluation: "strict" (fail-closed on unknown
    /// constraint types) or "permissive" (skip unknown types with a warning).
    /// Default: "strict".
    #[serde(default = "default_vi_strictness")]
    pub strictness: String,
}

fn default_vi_strictness() -> String {
    "strict".to_owned()
}

impl Default for VerifiableIntentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strictness: default_vi_strictness(),
        }
    }
}

// ── Nodes (Dynamic Node Discovery) ───────────────────────────────

/// Configuration for the dynamic node discovery system (`[nodes]`).
///
/// When enabled, external processes/devices can connect via WebSocket
/// at `/ws/nodes` and advertise their capabilities at runtime.
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "nodes"]
pub struct NodesConfig {
    /// Enable dynamic node discovery endpoint.
    #[serde(default)]
    pub enabled: bool,
    /// Maximum number of concurrent node connections.
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
    /// Optional bearer token for node authentication.
    #[serde(default)]
    #[secret]
    pub auth_token: Option<String>,
}

fn default_max_nodes() -> usize {
    16
}

impl Default for NodesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_nodes: default_max_nodes(),
            auth_token: None,
        }
    }
}

// ── TTS (Text-to-Speech) ─────────────────────────────────────────

fn default_tts_provider() -> String {
    "openai".into()
}

fn default_tts_voice() -> String {
    "alloy".into()
}

fn default_tts_format() -> String {
    "mp3".into()
}

fn default_tts_max_text_length() -> usize {
    4096
}

fn default_openai_tts_model() -> String {
    "tts-1".into()
}

fn default_openai_tts_speed() -> f64 {
    1.0
}

fn default_elevenlabs_model_id() -> String {
    "eleven_monolingual_v1".into()
}

fn default_elevenlabs_stability() -> f64 {
    0.5
}

fn default_elevenlabs_similarity_boost() -> f64 {
    0.5
}

fn default_google_tts_language_code() -> String {
    "en-US".into()
}

fn default_edge_tts_binary_path() -> String {
    "edge-tts".into()
}

fn default_piper_tts_api_url() -> String {
    "http://127.0.0.1:5000/v1/audio/speech".into()
}

/// Text-to-Speech configuration (`[tts]`).
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "tts"]
pub struct TtsConfig {
    /// Enable TTS synthesis.
    #[serde(default)]
    pub enabled: bool,
    /// Default TTS provider (`"openai"`, `"elevenlabs"`, `"google"`, `"edge"`).
    #[serde(default = "default_tts_provider")]
    pub default_provider: String,
    /// Default voice ID passed to the selected provider.
    #[serde(default = "default_tts_voice")]
    pub default_voice: String,
    /// Default audio output format (`"mp3"`, `"opus"`, `"wav"`).
    #[serde(default = "default_tts_format")]
    pub default_format: String,
    /// Maximum input text length in characters (default 4096).
    #[serde(default = "default_tts_max_text_length")]
    pub max_text_length: usize,
    /// OpenAI TTS provider configuration (`[tts.openai]`).
    #[serde(default)]
    #[nested]
    pub openai: Option<OpenAiTtsConfig>,
    /// ElevenLabs TTS provider configuration (`[tts.elevenlabs]`).
    #[serde(default)]
    #[nested]
    pub elevenlabs: Option<ElevenLabsTtsConfig>,
    /// Google Cloud TTS provider configuration (`[tts.google]`).
    #[serde(default)]
    #[nested]
    pub google: Option<GoogleTtsConfig>,
    /// Edge TTS provider configuration (`[tts.edge]`).
    #[serde(default)]
    #[nested]
    pub edge: Option<EdgeTtsConfig>,
    /// Piper TTS provider configuration (`[tts.piper]`).
    #[serde(default)]
    #[nested]
    pub piper: Option<PiperTtsConfig>,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_provider: default_tts_provider(),
            default_voice: default_tts_voice(),
            default_format: default_tts_format(),
            max_text_length: default_tts_max_text_length(),
            openai: None,
            elevenlabs: None,
            google: None,
            edge: None,
            piper: None,
        }
    }
}

/// OpenAI TTS provider configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "tts.openai"]
pub struct OpenAiTtsConfig {
    /// API key for OpenAI TTS.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
    /// Model name (default `"tts-1"`).
    #[serde(default = "default_openai_tts_model")]
    pub model: String,
    /// Playback speed multiplier (default `1.0`).
    #[serde(default = "default_openai_tts_speed")]
    pub speed: f64,
}

/// ElevenLabs TTS provider configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "tts.elevenlabs"]
pub struct ElevenLabsTtsConfig {
    /// API key for ElevenLabs.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
    /// Model ID (default `"eleven_monolingual_v1"`).
    #[serde(default = "default_elevenlabs_model_id")]
    pub model_id: String,
    /// Voice stability (0.0-1.0, default `0.5`).
    #[serde(default = "default_elevenlabs_stability")]
    pub stability: f64,
    /// Similarity boost (0.0-1.0, default `0.5`).
    #[serde(default = "default_elevenlabs_similarity_boost")]
    pub similarity_boost: f64,
}

/// Google Cloud TTS provider configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "tts.google"]
pub struct GoogleTtsConfig {
    /// API key for Google Cloud TTS.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
    /// Language code (default `"en-US"`).
    #[serde(default = "default_google_tts_language_code")]
    pub language_code: String,
}

/// Edge TTS provider configuration (free, subprocess-based).
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "tts.edge"]
pub struct EdgeTtsConfig {
    /// Path to the `edge-tts` binary (default `"edge-tts"`).
    #[serde(default = "default_edge_tts_binary_path")]
    pub binary_path: String,
}

impl Default for EdgeTtsConfig {
    fn default() -> Self {
        Self {
            binary_path: default_edge_tts_binary_path(),
        }
    }
}

/// Piper TTS provider configuration (local GPU-accelerated, OpenAI-compatible endpoint).
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "tts.piper"]
pub struct PiperTtsConfig {
    /// Base URL for the Piper TTS HTTP server (e.g. `"http://127.0.0.1:5000/v1/audio/speech"`).
    #[serde(default = "default_piper_tts_api_url")]
    pub api_url: String,
}

impl Default for PiperTtsConfig {
    fn default() -> Self {
        Self {
            api_url: default_piper_tts_api_url(),
        }
    }
}

/// Determines when a `ToolFilterGroup` is active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ToolFilterGroupMode {
    /// Tools in this group are always included in every turn.
    Always,
    /// Tools in this group are included only when the user message contains
    /// at least one of the configured `keywords` (case-insensitive substring match).
    #[default]
    Dynamic,
}

/// A named group of MCP tool patterns with an activation mode.
///
/// Each group lists glob patterns for MCP tool names (prefix `mcp_`) and an
/// optional set of keywords that trigger inclusion in `dynamic` mode.
/// Built-in (non-MCP) tools always pass through and are never affected by
/// `tool_filter_groups`.
///
/// # Example
/// ```toml
/// [[agent.tool_filter_groups]]
/// mode = "always"
/// tools = ["mcp_filesystem_*"]
/// keywords = []
///
/// [[agent.tool_filter_groups]]
/// mode = "dynamic"
/// tools = ["mcp_browser_*"]
/// keywords = ["browse", "website", "url", "search"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct ToolFilterGroup {
    /// Activation mode: `"always"` or `"dynamic"`.
    #[serde(default)]
    pub mode: ToolFilterGroupMode,
    /// Glob patterns matching MCP tool names (single `*` wildcard supported).
    #[serde(default)]
    pub tools: Vec<String>,
    /// Keywords that activate this group in `dynamic` mode (case-insensitive substring).
    /// Ignored when `mode = "always"`.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// When true, also filter built-in tools (not just MCP tools).
    #[serde(default)]
    pub filter_builtins: bool,
}

/// OpenAI Whisper STT provider configuration (`[transcription.openai]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "transcription.openai"]
pub struct OpenAiSttConfig {
    /// OpenAI API key for Whisper transcription.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
    /// Whisper model name (default: "whisper-1").
    #[serde(default = "default_openai_stt_model")]
    pub model: String,
}

/// Deepgram STT provider configuration (`[transcription.deepgram]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "transcription.deepgram"]
pub struct DeepgramSttConfig {
    /// Deepgram API key.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
    /// Deepgram model name (default: "nova-2").
    #[serde(default = "default_deepgram_stt_model")]
    pub model: String,
}

/// AssemblyAI STT provider configuration (`[transcription.assemblyai]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "transcription.assemblyai"]
pub struct AssemblyAiSttConfig {
    /// AssemblyAI API key.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
}

/// Google Cloud Speech-to-Text provider configuration (`[transcription.google]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "transcription.google"]
pub struct GoogleSttConfig {
    /// Google Cloud API key.
    #[serde(default)]
    #[secret]
    pub api_key: Option<String>,
    /// BCP-47 language code (default: "en-US").
    #[serde(default = "default_google_stt_language_code")]
    pub language_code: String,
}

/// Local/self-hosted Whisper-compatible STT endpoint (`[transcription.local_whisper]`).
///
/// Configures a self-hosted STT endpoint. Can be on localhost, a private network host, or any reachable URL.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "transcription.local-whisper"]
pub struct LocalWhisperConfig {
    /// HTTP or HTTPS endpoint URL, e.g. `"http://10.10.0.1:8001/v1/transcribe"`.
    pub url: String,
    /// Bearer token for endpoint authentication.
    /// Omit for unauthenticated local endpoints.
    #[serde(default)]
    #[secret]
    pub bearer_token: Option<String>,
    /// Maximum audio file size in bytes accepted by this endpoint.
    /// Defaults to 25 MB — matching the cloud API cap for a safe out-of-the-box
    /// experience. Self-hosted endpoints can accept much larger files; raise this
    /// as needed, but note that each transcription call clones the audio buffer
    /// into a multipart payload, so peak memory per request is ~2× this value.
    #[serde(default = "default_local_whisper_max_audio_bytes")]
    pub max_audio_bytes: usize,
    /// Request timeout in seconds. Defaults to 300 (large files on local GPU).
    #[serde(default = "default_local_whisper_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_local_whisper_max_audio_bytes() -> usize {
    25 * 1024 * 1024
}

fn default_local_whisper_timeout_secs() -> u64 {
    300
}

/// Agent orchestration configuration (`[agent]` section).
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "agent"]
pub struct AgentConfig {
    /// When true: bootstrap_max_chars=6000, rag_chunk_limit=2. Use for 13B or smaller models.
    #[serde(default)]
    pub compact_context: bool,
    /// Maximum tool-call loop turns per user message. Default: `10`.
    /// Setting to `0` falls back to the safe default of `10`.
    #[serde(default = "default_agent_max_tool_iterations")]
    pub max_tool_iterations: usize,
    /// Maximum conversation history messages retained per session. Default: `50`.
    #[serde(default = "default_agent_max_history_messages")]
    pub max_history_messages: usize,
    /// Maximum estimated tokens for conversation history before compaction triggers.
    /// Uses ~4 chars/token heuristic. When this threshold is exceeded, older messages
    /// are summarized to preserve context while staying within budget. Default: `32000`.
    #[serde(default = "default_agent_max_context_tokens")]
    pub max_context_tokens: usize,
    /// Enable parallel tool execution within a single iteration. Default: `false`.
    #[serde(default)]
    pub parallel_tools: bool,
    /// Tool dispatch strategy (e.g. `"auto"`). Default: `"auto"`.
    #[serde(default = "default_agent_tool_dispatcher")]
    pub tool_dispatcher: String,
    /// Tools exempt from the within-turn duplicate-call dedup check. Default: `[]`.
    #[serde(default)]
    pub tool_call_dedup_exempt: Vec<String>,
    /// Per-turn MCP tool schema filtering groups.
    ///
    /// When non-empty, only MCP tools matched by an active group are included in the
    /// tool schema sent to the LLM for that turn. Built-in tools always pass through.
    /// Default: `[]` (no filtering — all tools included).
    #[serde(default)]
    pub tool_filter_groups: Vec<ToolFilterGroup>,
    /// Maximum characters for the assembled system prompt. When `> 0`, the prompt
    /// is truncated to this limit after assembly (keeping the top portion which
    /// contains identity and safety instructions). `0` means unlimited.
    /// Useful for small-context models (e.g. glm-4.5-air ~8K tokens → set to 8000).
    #[serde(default = "default_max_system_prompt_chars")]
    pub max_system_prompt_chars: usize,
    /// Thinking/reasoning level control. Configures how deeply the model reasons
    /// per message. Users can override per-message with `/think:<level>` directives.
    #[nested]
    #[serde(default)]
    pub thinking: crate::scattered_types::ThinkingConfig,

    /// History pruning configuration for token efficiency.
    #[nested]
    #[serde(default)]
    pub history_pruning: crate::scattered_types::HistoryPrunerConfig,

    /// Enable context-aware tool filtering (only surface relevant tools per iteration).
    #[serde(default)]
    pub context_aware_tools: bool,

    /// Post-response quality evaluator configuration.
    #[nested]
    #[serde(default)]
    pub eval: crate::scattered_types::EvalConfig,

    /// Automatic complexity-based classification fallback.
    #[nested]
    #[serde(default)]
    pub auto_classify: Option<crate::scattered_types::AutoClassifyConfig>,

    /// Context compression configuration for automatic conversation compaction.
    #[nested]
    #[serde(default)]
    pub context_compression: crate::scattered_types::ContextCompressionConfig,

    /// Maximum characters for a single tool result before truncation.
    /// Head (2/3) and tail (1/3) are preserved with a truncation marker in the
    /// middle. Set to `0` to disable truncation. Default: `50000`.
    #[serde(default = "default_max_tool_result_chars")]
    pub max_tool_result_chars: usize,

    /// Number of most recent conversation turns whose full tool-call/result
    /// messages are preserved in channel conversation history. Older turns
    /// keep only the final assistant text. Set to `0` to disable (previous
    /// behavior). Default: `2`.
    #[serde(default = "default_keep_tool_context_turns")]
    pub keep_tool_context_turns: usize,
}

fn default_max_tool_result_chars() -> usize {
    50_000
}

fn default_keep_tool_context_turns() -> usize {
    2
}

fn default_agent_max_tool_iterations() -> usize {
    10
}

fn default_agent_max_history_messages() -> usize {
    50
}

fn default_agent_max_context_tokens() -> usize {
    32_000
}

fn default_agent_tool_dispatcher() -> String {
    "auto".into()
}

fn default_max_system_prompt_chars() -> usize {
    0
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            compact_context: true,
            max_tool_iterations: default_agent_max_tool_iterations(),
            max_history_messages: default_agent_max_history_messages(),
            max_context_tokens: default_agent_max_context_tokens(),
            parallel_tools: false,
            tool_dispatcher: default_agent_tool_dispatcher(),
            tool_call_dedup_exempt: Vec::new(),
            tool_filter_groups: Vec::new(),
            max_system_prompt_chars: default_max_system_prompt_chars(),
            thinking: crate::scattered_types::ThinkingConfig::default(),
            history_pruning: crate::scattered_types::HistoryPrunerConfig::default(),
            context_aware_tools: false,
            eval: crate::scattered_types::EvalConfig::default(),
            auto_classify: None,
            context_compression: crate::scattered_types::ContextCompressionConfig::default(),
            max_tool_result_chars: default_max_tool_result_chars(),
            keep_tool_context_turns: default_keep_tool_context_turns(),
        }
    }
}

// ── Pacing ────────────────────────────────────────────────────────

/// Pacing controls for slow/local LLM workloads (`[pacing]` section).
///
/// All fields are optional and default to values that preserve existing
/// behavior. When set, they extend — not replace — the existing timeout
/// and loop-detection subsystems.
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "pacing"]
pub struct PacingConfig {
    /// Per-step timeout in seconds: the maximum time allowed for a single
    /// LLM inference turn, independent of the total message budget.
    /// `None` means no per-step timeout (existing behavior).
    #[serde(default)]
    pub step_timeout_secs: Option<u64>,

    /// Minimum elapsed seconds before loop detection activates.
    /// Tasks completing under this threshold get aggressive loop protection;
    /// longer-running tasks receive a grace period before the detector starts
    /// counting. `None` means loop detection is always active (existing behavior).
    #[serde(default)]
    pub loop_detection_min_elapsed_secs: Option<u64>,

    /// Tool names excluded from identical-output / alternating-pattern loop
    /// detection. Useful for browser workflows where `browser_screenshot`
    /// structurally resembles a loop even when making progress.
    #[serde(default)]
    pub loop_ignore_tools: Vec<String>,

    /// Override for the hardcoded timeout scaling cap (default: 4).
    /// The channel message timeout budget is computed as:
    ///   `message_timeout_secs * min(max_tool_iterations, message_timeout_scale_max)`
    /// Raising this value lets long multi-step tasks with slow local models
    /// receive a proportionally larger budget without inflating the base timeout.
    #[serde(default)]
    pub message_timeout_scale_max: Option<u64>,

    /// Enable pattern-based loop detection (exact repeat, ping-pong,
    /// no-progress). Defaults to `true`.
    #[serde(default = "default_loop_detection_enabled")]
    pub loop_detection_enabled: bool,

    /// Sliding window size for the pattern-based loop detector.
    /// Defaults to 20.
    #[serde(default = "default_loop_detection_window_size")]
    pub loop_detection_window_size: usize,

    /// Number of consecutive identical tool+args calls before the first
    /// escalation (Warning). Defaults to 3.
    #[serde(default = "default_loop_detection_max_repeats")]
    pub loop_detection_max_repeats: usize,
}

fn default_loop_detection_enabled() -> bool {
    true
}

fn default_loop_detection_window_size() -> usize {
    20
}

fn default_loop_detection_max_repeats() -> usize {
    3
}

impl Default for PacingConfig {
    fn default() -> Self {
        Self {
            step_timeout_secs: None,
            loop_detection_min_elapsed_secs: None,
            loop_ignore_tools: Vec::new(),
            message_timeout_scale_max: None,
            loop_detection_enabled: default_loop_detection_enabled(),
            loop_detection_window_size: default_loop_detection_window_size(),
            loop_detection_max_repeats: default_loop_detection_max_repeats(),
        }
    }
}

// ── Reliability / supervision ────────────────────────────────────

/// Reliability and supervision configuration (`[reliability]` section).
///
/// Controls provider retries, fallback chains, API key rotation, and channel restart backoff.
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "reliability"]
pub struct ReliabilityConfig {
    /// Retries per provider before failing over.
    #[serde(default = "default_provider_retries")]
    pub provider_retries: u32,
    /// Base backoff (ms) for provider retry delay.
    #[serde(default = "default_provider_backoff_ms")]
    pub provider_backoff_ms: u64,
    /// Fallback provider chain (e.g. `["anthropic", "openai"]`).
    #[serde(default)]
    pub fallback_providers: Vec<String>,
    /// Additional API keys for round-robin rotation on rate-limit (429) errors.
    /// The primary `api_key` is always tried first; these are extras.
    #[serde(default)]
    pub api_keys: Vec<String>,
    /// Per-model fallback chains. When a model fails, try these alternatives in order.
    /// Example: `{ "claude-opus-4-20250514" = ["claude-sonnet-4-20250514", "gpt-4o"] }`
    #[serde(default)]
    pub model_fallbacks: std::collections::HashMap<String, Vec<String>>,
    /// Initial backoff for channel/daemon restarts.
    #[serde(default = "default_channel_backoff_secs")]
    pub channel_initial_backoff_secs: u64,
    /// Max backoff for channel/daemon restarts.
    #[serde(default = "default_channel_backoff_max_secs")]
    pub channel_max_backoff_secs: u64,
    /// Scheduler polling cadence in seconds.
    #[serde(default = "default_scheduler_poll_secs")]
    pub scheduler_poll_secs: u64,
    /// Max retries for cron job execution attempts.
    #[serde(default = "default_scheduler_retries")]
    pub scheduler_retries: u32,
}

fn default_provider_retries() -> u32 {
    2
}

fn default_provider_backoff_ms() -> u64 {
    500
}

fn default_channel_backoff_secs() -> u64 {
    2
}

fn default_channel_backoff_max_secs() -> u64 {
    60
}

fn default_scheduler_poll_secs() -> u64 {
    15
}

fn default_scheduler_retries() -> u32 {
    2
}

impl Default for ReliabilityConfig {
    fn default() -> Self {
        Self {
            provider_retries: default_provider_retries(),
            provider_backoff_ms: default_provider_backoff_ms(),
            fallback_providers: Vec::new(),
            api_keys: Vec::new(),
            model_fallbacks: std::collections::HashMap::new(),
            channel_initial_backoff_secs: default_channel_backoff_secs(),
            channel_max_backoff_secs: default_channel_backoff_max_secs(),
            scheduler_poll_secs: default_scheduler_poll_secs(),
            scheduler_retries: default_scheduler_retries(),
        }
    }
}
