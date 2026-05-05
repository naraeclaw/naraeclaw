//! 채널 설정 — CLI, Slack 채널 Config 타입.
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
/// NaraeClaw supports the following channel surfaces:
/// - `cli`   — interactive terminal session (always available)
/// - `slack` — Slack Socket Mode bot (no public URL required)
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels"]
pub struct ChannelsConfig {
    /// Enable the CLI interactive channel. Default: `true`.
    #[serde(default = "default_true")]
    pub cli: bool,
    /// Slack Socket Mode channel configuration.
    #[nested]
    pub slack: Option<SlackConfig>,
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
        vec![(
            Box::new(ConfigWrapper::new(self.slack.as_ref())),
            self.slack.is_some(),
        )]
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
            slack: None,
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

/// Slack Socket Mode channel configuration.
///
/// Uses Slack's Socket Mode API — no public URL required.
/// Requires an App-Level Token (`xapp-`) and a Bot Token (`xoxb-`).
///
/// Setup:
/// 1. Create a Slack app at <https://api.slack.com/apps>
/// 2. Enable Socket Mode and create an App-Level Token with `connections:write` scope
/// 3. Add Bot Token scopes: `chat:write`, `reactions:write`, `channels:history`, `im:history`
/// 4. Install the app to your workspace
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.slack"]
pub struct SlackConfig {
    /// Whether this channel is active. Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// App-Level Token (`xapp-…`). Required for Socket Mode WebSocket connection.
    #[secret]
    pub app_token: String,
    /// Bot Token (`xoxb-…`). Required for sending messages via Web API.
    #[secret]
    pub bot_token: String,
    /// Slack signing secret for request verification (optional).
    #[secret]
    pub signing_secret: Option<String>,
    /// Default channel ID to post to when no thread context is available.
    /// If unset, replies are sent to the channel where the message arrived.
    pub default_channel: Option<String>,
}

impl ChannelConfig for SlackConfig {
    fn name() -> &'static str {
        "Slack"
    }
    fn desc() -> &'static str {
        "Slack Socket Mode"
    }
}
