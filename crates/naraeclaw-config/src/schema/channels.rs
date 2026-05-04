//! 채널 설정 — CLI, Webhook, MQTT 채널 Config 타입.
#![allow(unused_imports)]
use super::*;
use crate::traits::ChannelConfig;
use naraeclaw_macros::Configurable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Channels ─────────────────────────────────────────────────────

struct ConfigWrapper<T: ChannelConfig>(std::marker::PhantomData<T>);

impl<T: ChannelConfig> ConfigWrapper<T> {
    fn new(_: Option<&T>) -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: ChannelConfig> crate::traits::ConfigHandle for ConfigWrapper<T> {
    fn name(&self) -> &'static str {
        T::name()
    }
    fn desc(&self) -> &'static str {
        T::desc()
    }
}

/// Top-level channel configurations (`[channels_config]` section).
///
/// NaraeClaw supports two active channel surfaces:
/// - `cli`     — interactive terminal session (always available)
/// - `webhook` — HTTP receiver for external integrations
/// - `mqtt`    — MQTT SOP listener
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels"]
pub struct ChannelsConfig {
    /// Enable the CLI interactive channel. Default: `true`.
    #[serde(default = "default_true")]
    pub cli: bool,
    /// Webhook channel configuration.
    #[nested]
    pub webhook: Option<WebhookConfig>,
    /// MQTT channel configuration (SOP listener).
    #[nested]
    pub mqtt: Option<MqttConfig>,
    /// Base timeout in seconds for processing a single channel message (LLM + tools).
    /// Runtime uses this as a per-turn budget that scales with tool-loop depth
    /// (up to 4x, capped) so one slow/retried model call does not consume the
    /// entire conversation budget.
    /// Default: 300s for on-device LLMs (Ollama) which are slower than cloud APIs.
    #[serde(default = "default_channel_message_timeout_secs")]
    pub message_timeout_secs: u64,
    /// Whether to add acknowledgement reactions (👀 on receipt, ✅/⚠️ on
    /// completion) to incoming channel messages. Default: `true`.
    #[serde(default = "default_true")]
    pub ack_reactions: bool,
    /// Whether to send tool-call notification messages (e.g. `🔧 web_search_tool: …`)
    /// to channel users. When `false`, tool calls are still logged server-side but
    /// not forwarded as individual channel messages. Default: `false`.
    #[serde(default = "default_false")]
    pub show_tool_calls: bool,
    /// Persist channel conversation history to JSONL files so sessions survive
    /// daemon restarts. Files are stored in `{workspace}/sessions/`. Default: `true`.
    #[serde(default = "default_true")]
    pub session_persistence: bool,
    /// Session persistence backend: `"jsonl"` (legacy) or `"sqlite"` (new default).
    /// SQLite provides FTS5 search, metadata tracking, and TTL cleanup.
    #[serde(default = "default_session_backend")]
    pub session_backend: String,
    /// Auto-archive stale sessions older than this many hours. `0` disables. Default: `0`.
    #[serde(default)]
    pub session_ttl_hours: u32,
    /// Inbound message debounce window in milliseconds. When a sender fires
    /// multiple messages within this window, they are accumulated and dispatched
    /// as a single concatenated message. `0` disables debouncing. Default: `0`.
    #[serde(default)]
    pub debounce_ms: u64,
}

impl ChannelsConfig {
    /// Backfill `enabled = true` for channel sections present in the raw TOML
    /// that don't have an explicit `enabled` key.
    pub fn backfill_enabled(&mut self, raw_toml: &str) {
        let table = match raw_toml.parse::<toml::Table>() {
            Ok(t) => t,
            Err(_) => return,
        };
        let channels = match table.get("channels_config").and_then(|v| v.as_table()) {
            Some(t) => t,
            None => return,
        };
        for (key, value) in channels {
            let is_section = value.as_table().is_some();
            let has_explicit_enabled = value.as_table().is_some_and(|t| t.contains_key("enabled"));
            if is_section && !has_explicit_enabled {
                let prop_path = format!("channels.{}.enabled", key.replace('_', "-"));
                if let Err(e) = self.set_prop(&prop_path, "true") {
                    tracing::warn!("backfill_enabled: failed to set {prop_path}: {e}");
                }
            }
        }
    }

    pub fn channels(&self) -> Vec<(Box<dyn crate::traits::ConfigHandle>, bool)> {
        vec![
            (
                Box::new(ConfigWrapper::new(self.webhook.as_ref())),
                self.webhook.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.mqtt.as_ref())),
                self.mqtt.is_some(),
            ),
        ]
    }
}

pub fn default_channel_message_timeout_secs() -> u64 {
    300
}

pub fn default_session_backend() -> String {
    "sqlite".into()
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            cli: true,
            webhook: None,
            mqtt: None,
            message_timeout_secs: default_channel_message_timeout_secs(),
            ack_reactions: true,
            show_tool_calls: false,
            session_persistence: true,
            session_backend: default_session_backend(),
            session_ttl_hours: 0,
            debounce_ms: 0,
        }
    }
}

/// Webhook channel configuration.
///
/// Receives messages via HTTP POST and sends replies to a configurable outbound URL.
/// This is the "universal adapter" for any system that supports webhooks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.webhook"]
pub struct WebhookConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Port to listen on for incoming webhooks.
    pub port: u16,
    /// URL path to listen on (default: `/webhook`).
    #[serde(default)]
    pub listen_path: Option<String>,
    /// URL to POST/PUT outbound messages to.
    #[serde(default)]
    pub send_url: Option<String>,
    /// HTTP method for outbound messages (`POST` or `PUT`). Default: `POST`.
    #[serde(default)]
    pub send_method: Option<String>,
    /// Optional `Authorization` header value for outbound requests.
    #[serde(default)]
    pub auth_header: Option<String>,
    /// Optional shared secret for webhook signature verification (HMAC-SHA256).
    #[secret]
    pub secret: Option<String>,
}

impl ChannelConfig for WebhookConfig {
    fn name() -> &'static str {
        "Webhook"
    }
    fn desc() -> &'static str {
        "HTTP endpoint"
    }
}

/// MQTT channel configuration (SOP listener).
///
/// Subscribes to MQTT topics and dispatches incoming messages
/// to the SOP engine for processing.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.mqtt"]
pub struct MqttConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// MQTT broker URL (e.g., `mqtt://localhost:1883` or `mqtts://broker.example.com:8883`).
    /// Use `mqtt://` for plain connections or `mqtts://` for TLS.
    pub broker_url: String,
    /// MQTT client ID (must be unique per broker).
    pub client_id: String,
    /// Topics to subscribe to (e.g., `sensors/#`, `alerts/+/critical`).
    /// At least one topic is required.
    #[serde(default)]
    pub topics: Vec<String>,
    /// MQTT QoS level (0 = at-most-once, 1 = at-least-once, 2 = exactly-once). Default: 1.
    #[serde(default = "default_mqtt_qos")]
    pub qos: u8,
    /// Username for authentication (optional).
    pub username: Option<String>,
    /// Password for authentication (optional).
    #[secret]
    pub password: Option<String>,
    /// Enable TLS encryption. Must match the broker_url scheme:
    /// - `mqtt://` → `use_tls: false`
    /// - `mqtts://` → `use_tls: true`
    #[serde(default)]
    pub use_tls: bool,
    /// Keep-alive interval in seconds (default: 30). Prevents broker disconnect on idle.
    #[serde(default = "default_mqtt_keep_alive_secs")]
    pub keep_alive_secs: u64,
}

impl MqttConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.qos > 2 {
            anyhow::bail!("qos must be 0, 1, or 2, got {}", self.qos);
        }

        let is_tls_scheme = self.broker_url.starts_with("mqtts://");
        let is_mqtt_scheme = self.broker_url.starts_with("mqtt://");

        if !is_tls_scheme && !is_mqtt_scheme {
            anyhow::bail!(
                "broker_url must start with 'mqtt://' or 'mqtts://', got: {}",
                self.broker_url
            );
        }

        if is_mqtt_scheme && self.use_tls {
            anyhow::bail!("use_tls is true but broker_url uses 'mqtt://' (not 'mqtts://')");
        }

        if is_tls_scheme && !self.use_tls {
            anyhow::bail!(
                "use_tls is false but broker_url uses 'mqtts://' (requires use_tls: true)"
            );
        }

        if self.topics.is_empty() {
            anyhow::bail!("at least one topic must be configured");
        }

        if self.client_id.is_empty() {
            anyhow::bail!("client_id must not be empty");
        }

        Ok(())
    }
}

impl ChannelConfig for MqttConfig {
    fn name() -> &'static str {
        "MQTT"
    }
    fn desc() -> &'static str {
        "MQTT SOP Listener"
    }
}

pub fn default_mqtt_qos() -> u8 {
    1
}

pub fn default_mqtt_keep_alive_secs() -> u64 {
    30
}
