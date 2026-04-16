//! 채널 설정 — Telegram, Discord, Slack 등 채널별 Config 타입.
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
/// Each channel sub-section (e.g. `telegram`, `discord`) is optional;
/// setting it to `Some(...)` enables that channel.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels"]
pub struct ChannelsConfig {
    /// Enable the CLI interactive channel. Default: `true`.
    #[serde(default = "default_true")]
    pub cli: bool,
    /// Telegram bot channel configuration.
    #[nested]
    pub telegram: Option<TelegramConfig>,
    /// Discord bot channel configuration.
    #[nested]
    pub discord: Option<DiscordConfig>,
    /// Discord history channel — logs ALL messages and forwards @mentions to agent.
    #[nested]
    pub discord_history: Option<DiscordHistoryConfig>,
    /// Slack bot channel configuration.
    #[nested]
    pub slack: Option<SlackConfig>,
    /// Mattermost bot channel configuration.
    #[nested]
    pub mattermost: Option<MattermostConfig>,
    /// Webhook channel configuration.
    #[nested]
    pub webhook: Option<WebhookConfig>,
    /// iMessage channel configuration (macOS only).
    #[nested]
    pub imessage: Option<IMessageConfig>,
    /// Matrix channel configuration.
    #[nested]
    pub matrix: Option<MatrixConfig>,
    /// Signal channel configuration.
    #[nested]
    pub signal: Option<SignalConfig>,
    /// WhatsApp channel configuration (Cloud API or Web mode).
    #[nested]
    pub whatsapp: Option<WhatsAppConfig>,
    /// Linq Partner API channel configuration.
    #[nested]
    pub linq: Option<LinqConfig>,
    /// WATI WhatsApp Business API channel configuration.
    #[nested]
    pub wati: Option<WatiConfig>,
    /// Nextcloud Talk bot channel configuration.
    #[nested]
    pub nextcloud_talk: Option<NextcloudTalkConfig>,
    /// Email channel configuration.
    #[nested]
    pub email: Option<crate::scattered_types::EmailConfig>,
    /// Gmail Pub/Sub push notification channel configuration.
    #[nested]
    pub gmail_push: Option<crate::scattered_types::GmailPushConfig>,
    /// IRC channel configuration.
    #[nested]
    pub irc: Option<IrcConfig>,
    /// LINE Messaging API channel configuration.
    #[nested]
    pub line: Option<LineConfig>,
    /// X/Twitter channel configuration.
    #[nested]
    pub twitter: Option<TwitterConfig>,
    #[cfg(feature = "channel-nostr")]
    #[nested]
    pub nostr: Option<NostrConfig>,
    /// ClawdTalk voice channel configuration.
    #[nested]
    pub clawdtalk: Option<crate::scattered_types::ClawdTalkConfig>,
    /// Reddit channel configuration (OAuth2 bot).
    #[nested]
    pub reddit: Option<RedditConfig>,
    /// Bluesky channel configuration (AT Protocol).
    #[nested]
    pub bluesky: Option<BlueskyConfig>,
    /// Voice call channel configuration (Twilio/Telnyx/Plivo).
    #[nested]
    pub voice_call: Option<crate::scattered_types::VoiceCallConfig>,
    /// Voice wake word detection channel configuration.
    #[cfg(feature = "voice-wake")]
    #[nested]
    pub voice_wake: Option<VoiceWakeConfig>,
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
    /// that don't have an explicit `enabled` key. This preserves backward
    /// compatibility: configs written before `enabled` was introduced continue
    /// to activate their channels.
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
                // Section exists without explicit `enabled` — backfill true
                let prop_path = format!("channels.{}.enabled", key.replace('_', "-"));
                if let Err(e) = self.set_prop(&prop_path, "true") {
                    tracing::warn!("backfill_enabled: failed to set {prop_path}: {e}");
                }
            }
        }
    }

    /// get channels' metadata and `.is_some()`, except webhook
    #[rustfmt::skip]
    pub fn channels_except_webhook(&self) -> Vec<(Box<dyn crate::traits::ConfigHandle>, bool)> {
        vec![
            (
                Box::new(ConfigWrapper::new(self.telegram.as_ref())),
                self.telegram.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.discord.as_ref())),
                self.discord.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.slack.as_ref())),
                self.slack.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.mattermost.as_ref())),
                self.mattermost.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.imessage.as_ref())),
                self.imessage.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.matrix.as_ref())),
                self.matrix.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.signal.as_ref())),
                self.signal.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.whatsapp.as_ref())),
                self.whatsapp.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.linq.as_ref())),
                self.linq.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.wati.as_ref())),
                self.wati.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.nextcloud_talk.as_ref())),
                self.nextcloud_talk.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.email.as_ref())),
                self.email.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.gmail_push.as_ref())),
                self.gmail_push.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.irc.as_ref())),
                self.irc.is_some()
            ),
            #[cfg(feature = "channel-nostr")]
            (
                Box::new(ConfigWrapper::new(self.nostr.as_ref())),
                self.nostr.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.clawdtalk.as_ref())),
                self.clawdtalk.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.reddit.as_ref())),
                self.reddit.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.bluesky.as_ref())),
                self.bluesky.is_some(),
            ),
            #[cfg(feature = "voice-wake")]
            (
                Box::new(ConfigWrapper::new(self.voice_wake.as_ref())),
                self.voice_wake.is_some(),
            ),
            (
                Box::new(ConfigWrapper::new(self.mqtt.as_ref())),
                self.mqtt.is_some(),
            ),
        ]
    }

    pub fn channels(&self) -> Vec<(Box<dyn crate::traits::ConfigHandle>, bool)> {
        let mut ret = self.channels_except_webhook();
        ret.push((
            Box::new(ConfigWrapper::new(self.webhook.as_ref())),
            self.webhook.is_some(),
        ));
        ret
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
            telegram: None,
            discord: None,
            discord_history: None,
            slack: None,
            mattermost: None,
            webhook: None,
            imessage: None,
            matrix: None,
            signal: None,
            whatsapp: None,
            linq: None,
            wati: None,
            nextcloud_talk: None,
            email: None,
            gmail_push: None,
            irc: None,
            line: None,
            twitter: None,
            #[cfg(feature = "channel-nostr")]
            nostr: None,
            clawdtalk: None,
            reddit: None,
            bluesky: None,
            voice_call: None,
            #[cfg(feature = "voice-wake")]
            voice_wake: None,
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

/// Streaming mode for channels that support progressive message updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum StreamMode {
    /// No streaming -- send the complete response as a single message (default).
    #[default]
    Off,
    /// Update a draft message with every flush interval.
    Partial,
    /// Send the response as multiple separate messages at paragraph boundaries.
    #[serde(rename = "multi_message")]
    MultiMessage,
}

pub fn default_draft_update_interval_ms() -> u64 {
    1000
}

pub fn default_multi_message_delay_ms() -> u64 {
    800
}

pub fn default_matrix_draft_update_interval_ms() -> u64 {
    1500
}

pub fn default_telegram_webhook_listen_addr() -> String {
    "0.0.0.0:8443".to_string()
}

pub fn default_telegram_webhook_path() -> String {
    "/telegram/webhook".to_string()
}

/// Telegram bot channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.telegram"]
pub struct TelegramConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Telegram Bot API token (from @BotFather).
    #[secret]
    pub bot_token: String,
    /// Allowed Telegram user IDs or usernames. Empty = deny all.
    pub allowed_users: Vec<String>,
    /// Streaming mode for progressive response delivery via message edits.
    #[serde(default)]
    pub stream_mode: StreamMode,
    /// Minimum interval (ms) between draft message edits to avoid rate limits.
    #[serde(default = "default_draft_update_interval_ms")]
    pub draft_update_interval_ms: u64,
    /// When true, a newer Telegram message from the same sender in the same chat
    /// cancels the in-flight request and starts a fresh response with preserved history.
    #[serde(default)]
    pub interrupt_on_new_message: bool,
    /// When true, only respond to messages that @-mention the bot in groups.
    /// Direct messages are always processed.
    #[serde(default)]
    pub mention_only: bool,
    /// Override for the top-level `ack_reactions` setting. When `None`, the
    /// channel falls back to `[channels_config].ack_reactions`. When set
    /// explicitly, it takes precedence.
    #[serde(default)]
    pub ack_reactions: Option<bool>,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// Public HTTPS URL registered with Telegram via `setWebhook`.
    /// When set, Telegram receives updates in webhook mode instead of polling.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// Local bind address for the embedded Telegram webhook server.
    #[serde(default = "default_telegram_webhook_listen_addr")]
    pub webhook_listen_addr: String,
    /// Local HTTP path for Telegram webhook POST requests.
    #[serde(default = "default_telegram_webhook_path")]
    pub webhook_path: String,
    /// Optional Telegram secret token verified from
    /// `X-Telegram-Bot-Api-Secret-Token`.
    #[serde(default)]
    #[secret]
    pub webhook_secret_token: Option<String>,
}

impl ChannelConfig for TelegramConfig {
    fn name() -> &'static str {
        "Telegram"
    }
    fn desc() -> &'static str {
        "connect your bot"
    }
}

/// Discord bot channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.discord"]
#[allow(clippy::struct_excessive_bools)]
pub struct DiscordConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Discord bot token (from Discord Developer Portal).
    #[secret]
    pub bot_token: String,
    /// Optional guild (server) ID to restrict the bot to a single guild.
    pub guild_id: Option<String>,
    /// Allowed Discord user IDs. Empty = deny all.
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// When true, process messages from other bots (not just humans).
    /// The bot still ignores its own messages to prevent feedback loops.
    #[serde(default)]
    pub listen_to_bots: bool,
    /// When true, a newer Discord message from the same sender in the same channel
    /// cancels the in-flight request and starts a fresh response with preserved history.
    #[serde(default)]
    pub interrupt_on_new_message: bool,
    /// When true, only respond to messages that @-mention the bot.
    /// Other messages in the guild are silently ignored.
    #[serde(default)]
    pub mention_only: bool,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// Streaming mode for progressive response delivery.
    /// `off` (default): single message. `partial`: editable draft updates.
    /// `multi_message`: split response into separate messages at paragraph boundaries.
    #[serde(default)]
    pub stream_mode: StreamMode,
    /// Minimum interval (ms) between draft message edits to avoid rate limits.
    /// Only used when `stream_mode = "partial"`.
    #[serde(default = "default_draft_update_interval_ms")]
    pub draft_update_interval_ms: u64,
    /// Delay (ms) between sending each message chunk in multi-message mode.
    /// Only used when `stream_mode = "multi_message"`.
    #[serde(default = "default_multi_message_delay_ms")]
    pub multi_message_delay_ms: u64,
    /// Stall-watchdog timeout in seconds. When non-zero, the bot will abort
    /// and retry if no progress is made within this duration. 0 = disabled.
    #[serde(default)]
    pub stall_timeout_secs: u64,
}

impl ChannelConfig for DiscordConfig {
    fn name() -> &'static str {
        "Discord"
    }
    fn desc() -> &'static str {
        "connect your bot"
    }
}

/// Discord history channel — logs ALL messages to discord.db and forwards @mentions to the agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.discord-history"]
pub struct DiscordHistoryConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Discord bot token (from Discord Developer Portal).
    #[secret]
    pub bot_token: String,
    /// Optional guild (server) ID to restrict logging to a single guild.
    pub guild_id: Option<String>,
    /// Allowed Discord user IDs. Empty = allow all (open logging).
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// Discord channel IDs to watch. Empty = watch all channels.
    #[serde(default)]
    pub channel_ids: Vec<String>,
    /// When true (default), store Direct Messages in discord.db.
    #[serde(default = "default_true")]
    pub store_dms: bool,
    /// When true (default), respond to @mentions in Direct Messages.
    #[serde(default = "default_true")]
    pub respond_to_dms: bool,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    #[serde(default)]
    pub proxy_url: Option<String>,
}

impl ChannelConfig for DiscordHistoryConfig {
    fn name() -> &'static str {
        "Discord History"
    }
    fn desc() -> &'static str {
        "log all messages and forward @mentions"
    }
}

/// Slack bot channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.slack"]
#[allow(clippy::struct_excessive_bools)]
pub struct SlackConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Slack bot OAuth token (xoxb-...).
    #[secret]
    pub bot_token: String,
    /// Slack app-level token for Socket Mode (xapp-...).
    #[secret]
    pub app_token: Option<String>,
    /// Optional channel ID to restrict the bot to a single channel.
    /// Omit (or set `"*"`) to listen across all accessible channels.
    pub channel_id: Option<String>,
    /// Optional explicit list of channel IDs to watch.
    /// When set, this takes precedence over `channel_id`.
    #[serde(default)]
    pub channel_ids: Vec<String>,
    /// Allowed Slack user IDs. Empty = deny all.
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// When true, a newer Slack message from the same sender in the same channel
    /// cancels the in-flight request and starts a fresh response with preserved history.
    #[serde(default)]
    pub interrupt_on_new_message: bool,
    /// When true (default), replies stay in the originating Slack thread.
    /// When false, replies go to the channel root instead.
    #[serde(default)]
    pub thread_replies: Option<bool>,
    /// When true, only respond to messages that @-mention the bot in groups.
    /// Direct messages remain allowed.
    #[serde(default)]
    pub mention_only: bool,
    /// Use the newer Slack `markdown` block type (12 000 char limit, richer formatting).
    /// Defaults to false (uses universally supported `section` blocks with `mrkdwn`).
    /// Enable this only if your Slack workspace supports the `markdown` block type.
    #[serde(default)]
    pub use_markdown_blocks: bool,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// Enable progressive draft message streaming via `chat.update`.
    #[serde(default)]
    pub stream_drafts: bool,
    /// Minimum interval (ms) between draft message edits to avoid Slack rate limits.
    #[serde(default = "default_slack_draft_update_interval_ms")]
    pub draft_update_interval_ms: u64,
    /// Emoji reaction name (without colons) that cancels an in-flight request.
    /// For example, `"x"` means reacting with `:x:` cancels the task.
    /// Leave unset to disable reaction-based cancellation.
    #[serde(default)]
    pub cancel_reaction: Option<String>,
}

pub fn default_slack_draft_update_interval_ms() -> u64 {
    1200
}

impl ChannelConfig for SlackConfig {
    fn name() -> &'static str {
        "Slack"
    }
    fn desc() -> &'static str {
        "connect your bot"
    }
}

/// Mattermost bot channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.mattermost"]
pub struct MattermostConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Mattermost server URL (e.g. `"https://mattermost.example.com"`).
    pub url: String,
    /// Mattermost bot access token.
    #[secret]
    pub bot_token: String,
    /// Optional channel ID to restrict the bot to a single channel.
    pub channel_id: Option<String>,
    /// Allowed Mattermost user IDs. Empty = deny all.
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// When true (default), replies thread on the original post.
    /// When false, replies go to the channel root.
    #[serde(default)]
    pub thread_replies: Option<bool>,
    /// When true, only respond to messages that @-mention the bot.
    /// Other messages in the channel are silently ignored.
    #[serde(default)]
    pub mention_only: Option<bool>,
    /// When true, a newer Mattermost message from the same sender in the same channel
    /// cancels the in-flight request and starts a fresh response with preserved history.
    #[serde(default)]
    pub interrupt_on_new_message: bool,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
}

impl ChannelConfig for MattermostConfig {
    fn name() -> &'static str {
        "Mattermost"
    }
    fn desc() -> &'static str {
        "connect to your bot"
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

/// iMessage channel configuration (macOS only).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.imessage"]
pub struct IMessageConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Allowed iMessage contacts (phone numbers or email addresses). Empty = deny all.
    pub allowed_contacts: Vec<String>,
}

impl ChannelConfig for IMessageConfig {
    fn name() -> &'static str {
        "iMessage"
    }
    fn desc() -> &'static str {
        "macOS only"
    }
}

/// Matrix channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.matrix"]
pub struct MatrixConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Matrix homeserver URL (e.g. `"https://matrix.org"`).
    pub homeserver: String,
    /// Matrix access token for the bot account.
    #[secret]
    pub access_token: String,
    /// Optional Matrix user ID (e.g. `"@bot:matrix.org"`).
    #[serde(default)]
    pub user_id: Option<String>,
    /// Optional Matrix device ID.
    #[serde(default)]
    pub device_id: Option<String>,
    /// Matrix room ID to listen in (e.g. `"!abc123:matrix.org"`).
    pub room_id: String,
    /// Allowed Matrix user IDs. Empty = deny all.
    pub allowed_users: Vec<String>,
    /// Allowed Matrix room IDs or aliases. Empty = allow all rooms.
    /// Supports canonical room IDs (`!abc:server`) and aliases (`#room:server`).
    #[serde(default)]
    pub allowed_rooms: Vec<String>,
    /// Whether to interrupt an in-flight agent response when a new message arrives.
    #[serde(default)]
    pub interrupt_on_new_message: bool,
    /// Streaming mode for progressive response delivery.
    /// `"off"` (default): single message. `"partial"`: edit-in-place draft.
    /// `"multi_message"`: paragraph-split delivery.
    #[serde(default)]
    pub stream_mode: StreamMode,
    /// Minimum interval (ms) between draft message edits in Partial mode.
    #[serde(default = "default_matrix_draft_update_interval_ms")]
    pub draft_update_interval_ms: u64,
    /// Delay (ms) between sending each paragraph in MultiMessage mode.
    #[serde(default = "default_multi_message_delay_ms")]
    pub multi_message_delay_ms: u64,
    /// Optional Matrix recovery key for automatic E2EE key backup restore.
    /// When set, NaraeClaw recovers room keys and cross-signing secrets on startup.
    #[secret]
    #[serde(default)]
    pub recovery_key: Option<String>,
}

impl ChannelConfig for MatrixConfig {
    fn name() -> &'static str {
        "Matrix"
    }
    fn desc() -> &'static str {
        "self-hosted chat"
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.signal"]
pub struct SignalConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Base URL for the signal-cli HTTP daemon (e.g. "http://127.0.0.1:8686").
    pub http_url: String,
    /// E.164 phone number of the signal-cli account (e.g. "+1234567890").
    pub account: String,
    /// Optional group ID to filter messages.
    /// - `None` or omitted: accept all messages (DMs and groups)
    /// - `"dm"`: only accept direct messages
    /// - Specific group ID: only accept messages from that group
    #[serde(default)]
    pub group_id: Option<String>,
    /// Allowed sender phone numbers (E.164) or "*" for all.
    #[serde(default)]
    pub allowed_from: Vec<String>,
    /// Skip messages that are attachment-only (no text body).
    #[serde(default)]
    pub ignore_attachments: bool,
    /// Skip incoming story messages.
    #[serde(default)]
    pub ignore_stories: bool,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
}

impl ChannelConfig for SignalConfig {
    fn name() -> &'static str {
        "Signal"
    }
    fn desc() -> &'static str {
        "An open-source, encrypted messaging service"
    }
}

/// WhatsApp Web usage mode.
///
/// `Personal` treats the account as a personal phone — the bot only responds to
/// incoming messages that pass the DM/group/self-chat policy filters.
/// `Business` (default) responds to all incoming messages, subject only to the
/// `allowed_numbers` allowlist.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WhatsAppWebMode {
    /// Respond to all messages passing the allowlist (default).
    #[default]
    Business,
    /// Apply per-chat-type policies (dm_policy, group_policy, self_chat_mode).
    Personal,
}

/// Policy for a particular WhatsApp chat type (DMs or groups) when
/// `mode = "personal"`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WhatsAppChatPolicy {
    /// Only respond to senders on the `allowed_numbers` list (default).
    #[default]
    Allowlist,
    /// Ignore all messages in this chat type.
    Ignore,
    /// Respond to every message regardless of allowlist.
    All,
}

/// WhatsApp channel configuration (Cloud API or Web mode).
///
/// Set `phone_number_id` for Cloud API mode, or `session_path` for Web mode.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.whatsapp"]
pub struct WhatsAppConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Access token from Meta Business Suite (Cloud API mode)
    #[serde(default)]
    #[secret]
    pub access_token: Option<String>,
    /// Phone number ID from Meta Business API (Cloud API mode)
    #[serde(default)]
    pub phone_number_id: Option<String>,
    /// Webhook verify token (you define this, Meta sends it back for verification)
    /// Only used in Cloud API mode
    #[serde(default)]
    #[secret]
    pub verify_token: Option<String>,
    /// App secret from Meta Business Suite (for webhook signature verification)
    /// Can also be set via `ZEROCLAW_WHATSAPP_APP_SECRET` environment variable
    /// Only used in Cloud API mode
    #[serde(default)]
    #[secret]
    pub app_secret: Option<String>,
    /// Session database path for WhatsApp Web client (Web mode)
    /// When set, enables native WhatsApp Web mode with wa-rs
    #[serde(default)]
    pub session_path: Option<String>,
    /// Phone number for pair code linking (Web mode, optional)
    /// Format: country code + number (e.g., "15551234567")
    /// If not set, QR code pairing will be used
    #[serde(default)]
    pub pair_phone: Option<String>,
    /// Custom pair code for linking (Web mode, optional)
    /// Leave empty to let WhatsApp generate one
    #[serde(default)]
    pub pair_code: Option<String>,
    /// Allowed phone numbers (E.164 format: +1234567890) or "*" for all
    #[serde(default)]
    pub allowed_numbers: Vec<String>,
    /// When true, only respond to messages that @-mention the bot in groups (Web mode only).
    /// Direct messages are always processed.
    /// Bot identity is resolved from the wa-rs device at runtime; `pair_phone` seeds it on first connect.
    #[serde(default)]
    pub mention_only: bool,
    /// Usage mode for WhatsApp Web: "business" (default) or "personal".
    /// In personal mode the bot applies dm_policy, group_policy, and
    /// self_chat_mode to decide which chats to respond in.
    #[serde(default)]
    pub mode: WhatsAppWebMode,
    /// Policy for direct messages when mode = "personal".
    /// "allowlist" (default) | "ignore" | "all".
    #[serde(default)]
    pub dm_policy: WhatsAppChatPolicy,
    /// Policy for group chats when mode = "personal".
    /// "allowlist" (default) | "ignore" | "all".
    #[serde(default)]
    pub group_policy: WhatsAppChatPolicy,
    /// When true and mode = "personal", always respond to messages in the
    /// user's own self-chat (Notes to Self). Defaults to false.
    #[serde(default)]
    pub self_chat_mode: bool,
    /// Regex patterns for DM mention gating (case-insensitive).
    /// When non-empty, only direct messages matching at least one pattern are
    /// processed; matched fragments are stripped from the forwarded content.
    /// Example: `["@?NaraeClaw", "\\+?15555550123"]`
    #[serde(default)]
    pub dm_mention_patterns: Vec<String>,
    /// Regex patterns for group-chat mention gating (case-insensitive).
    /// When non-empty, only group messages matching at least one pattern are
    /// processed; matched fragments are stripped from the forwarded content.
    /// Example: `["@?NaraeClaw", "\\+?15555550123"]`
    #[serde(default)]
    pub group_mention_patterns: Vec<String>,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
}

impl ChannelConfig for WhatsAppConfig {
    fn name() -> &'static str {
        "WhatsApp"
    }
    fn desc() -> &'static str {
        "Business Cloud API"
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.linq"]
pub struct LinqConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Linq Partner API token (Bearer auth)
    #[secret]
    pub api_token: String,
    /// Phone number to send from (E.164 format)
    pub from_phone: String,
    /// Webhook signing secret for signature verification
    #[serde(default)]
    #[secret]
    pub signing_secret: Option<String>,
    /// Allowed sender handles (phone numbers) or "*" for all
    #[serde(default)]
    pub allowed_senders: Vec<String>,
}

impl ChannelConfig for LinqConfig {
    fn name() -> &'static str {
        "Linq"
    }
    fn desc() -> &'static str {
        "iMessage/RCS/SMS via Linq API"
    }
}

/// WATI WhatsApp Business API channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.wati"]
pub struct WatiConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// WATI API token (Bearer auth).
    #[secret]
    pub api_token: String,
    /// WATI API base URL (default: https://live-mt-server.wati.io).
    #[serde(default = "default_wati_api_url")]
    pub api_url: String,
    /// Tenant ID for multi-channel setups (optional).
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Allowed phone numbers (E.164 format) or "*" for all.
    #[serde(default)]
    pub allowed_numbers: Vec<String>,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
}

pub fn default_wati_api_url() -> String {
    "https://live-mt-server.wati.io".to_string()
}

impl ChannelConfig for WatiConfig {
    fn name() -> &'static str {
        "WATI"
    }
    fn desc() -> &'static str {
        "WhatsApp via WATI Business API"
    }
}

/// Nextcloud Talk bot configuration (webhook receive + OCS send API).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.nextcloud-talk"]
pub struct NextcloudTalkConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Nextcloud base URL (e.g. "https://cloud.example.com").
    pub base_url: String,
    /// Bot app token used for OCS API bearer auth.
    #[secret]
    pub app_token: String,
    /// Shared secret for webhook signature verification.
    ///
    /// Can also be set via `ZEROCLAW_NEXTCLOUD_TALK_WEBHOOK_SECRET`.
    #[serde(default)]
    #[secret]
    pub webhook_secret: Option<String>,
    /// Allowed Nextcloud actor IDs (`[]` = deny all, `"*"` = allow all).
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// Display name of the bot in Nextcloud Talk (e.g. "zeroclaw").
    /// Used to filter out the bot's own messages and prevent feedback loops.
    /// If not set, defaults to an empty string (no self-message filtering by name).
    #[serde(default)]
    pub bot_name: Option<String>,
}

impl ChannelConfig for NextcloudTalkConfig {
    fn name() -> &'static str {
        "NextCloud Talk"
    }
    fn desc() -> &'static str {
        "NextCloud Talk platform"
    }
}

impl WhatsAppConfig {
    /// Detect which backend to use based on config fields.
    /// Returns "cloud" if phone_number_id is set, "web" if session_path is set.
    pub fn backend_type(&self) -> &'static str {
        if self.phone_number_id.is_some() {
            "cloud"
        } else if self.session_path.is_some() {
            "web"
        } else {
            // Default to Cloud API for backward compatibility
            "cloud"
        }
    }

    /// Check if this is a valid Cloud API config
    pub fn is_cloud_config(&self) -> bool {
        self.phone_number_id.is_some() && self.access_token.is_some() && self.verify_token.is_some()
    }

    /// Check if this is a valid Web config
    pub fn is_web_config(&self) -> bool {
        self.session_path.is_some()
    }

    /// Returns true when both Cloud and Web selectors are present.
    ///
    /// Runtime currently prefers Cloud mode in this case for backward compatibility.
    pub fn is_ambiguous_config(&self) -> bool {
        self.phone_number_id.is_some() && self.session_path.is_some()
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
    /// Validate the MQTT configuration.
    ///
    /// Checks:
    /// - QoS is 0, 1, or 2
    /// - broker_url uses valid scheme (`mqtt://` or `mqtts://`)
    /// - `use_tls` flag matches broker_url scheme
    /// - At least one topic is configured
    /// - client_id is non-empty
    pub fn validate(&self) -> anyhow::Result<()> {
        // QoS validation
        if self.qos > 2 {
            anyhow::bail!("qos must be 0, 1, or 2, got {}", self.qos);
        }

        // Broker URL validation
        let is_tls_scheme = self.broker_url.starts_with("mqtts://");
        let is_mqtt_scheme = self.broker_url.starts_with("mqtt://");

        if !is_tls_scheme && !is_mqtt_scheme {
            anyhow::bail!(
                "broker_url must start with 'mqtt://' or 'mqtts://', got: {}",
                self.broker_url
            );
        }

        // TLS flag validation
        if is_mqtt_scheme && self.use_tls {
            anyhow::bail!("use_tls is true but broker_url uses 'mqtt://' (not 'mqtts://')");
        }

        if is_tls_scheme && !self.use_tls {
            anyhow::bail!(
                "use_tls is false but broker_url uses 'mqtts://' (requires use_tls: true)"
            );
        }

        // Topics validation
        if self.topics.is_empty() {
            anyhow::bail!("at least one topic must be configured");
        }

        // Client ID validation
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

/// IRC channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.irc"]
pub struct IrcConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// IRC server hostname
    pub server: String,
    /// IRC server port (default: 6697 for TLS)
    #[serde(default = "default_irc_port")]
    pub port: u16,
    /// Bot nickname
    pub nickname: String,
    /// Username (defaults to nickname if not set)
    pub username: Option<String>,
    /// Channels to join on connect
    #[serde(default)]
    pub channels: Vec<String>,
    /// Allowed nicknames (case-insensitive) or "*" for all
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// Server password (for bouncers like ZNC)
    #[secret]
    pub server_password: Option<String>,
    /// NickServ IDENTIFY password
    #[secret]
    pub nickserv_password: Option<String>,
    /// SASL PLAIN password (IRCv3)
    #[secret]
    pub sasl_password: Option<String>,
    /// Verify TLS certificate (default: true)
    pub verify_tls: Option<bool>,
}

impl ChannelConfig for IrcConfig {
    fn name() -> &'static str {
        "IRC"
    }
    fn desc() -> &'static str {
        "IRC over TLS"
    }
}

pub fn default_irc_port() -> u16 {
    6697
}

/// DM (1:1 chat) access policy for the LINE channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum LineDmPolicy {
    /// Respond to every DM regardless of who sent it.
    Open,
    /// Require a one-time `/bind <code>` handshake before responding (default).
    /// NaraeClaw prints the bind code on startup; send it once to unlock access.
    #[default]
    Pairing,
    /// Respond only to LINE user IDs listed in `allowed_users`.
    Allowlist,
}

/// Group / multi-person chat policy for the LINE channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum LineGroupPolicy {
    /// Respond to every message in group/room chats.
    Open,
    /// Respond only when the bot is @mentioned (default).
    #[default]
    Mention,
    /// Ignore all messages in group/room chats.
    Disabled,
}

/// LINE Messaging API channel configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "channels.line"]
pub struct LineConfig {
    /// Whether this channel is active (must be explicitly enabled). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Long-lived channel access token (from LINE Developers Console).
    /// Used for both the Reply API and the Push API fallback.
    /// Falls back to the `LINE_CHANNEL_ACCESS_TOKEN` environment variable if empty.
    #[serde(default)]
    #[secret]
    pub channel_access_token: String,
    /// Channel secret (from LINE Developers Console).
    /// Used to verify the `X-Line-Signature` header on incoming webhooks.
    /// Falls back to the `LINE_CHANNEL_SECRET` environment variable if empty.
    #[serde(default)]
    #[secret]
    pub channel_secret: String,
    /// DM (1:1 chat) access policy. Default: `pairing`.
    ///
    /// - `open`      — respond to everyone
    /// - `pairing`   — require one-time `/bind <code>` handshake on first contact
    /// - `allowlist` — respond only to user IDs listed in `allowed_users`
    #[serde(default)]
    pub dm_policy: LineDmPolicy,
    /// Group / multi-person chat policy. Default: `mention`.
    ///
    /// - `open`     — respond to every message
    /// - `mention`  — respond only when @mentioned
    /// - `disabled` — ignore all group messages
    #[serde(default)]
    pub group_policy: LineGroupPolicy,
    /// LINE user IDs that are allowed to interact with the bot.
    /// Used when `dm_policy = allowlist`. `["*"]` accepts everyone.
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// TCP port the embedded webhook server listens on. Default: `8443`.
    #[serde(default = "default_line_webhook_port")]
    pub webhook_port: u16,
    /// Per-channel proxy URL (http, https, socks5, socks5h).
    /// Overrides the global `[proxy]` setting for this channel only.
    #[serde(default)]
    pub proxy_url: Option<String>,
}

pub fn default_line_webhook_port() -> u16 {
    8443
}

impl ChannelConfig for LineConfig {
    fn name() -> &'static str {
        "LINE"
    }
    fn desc() -> &'static str {
        "connect your LINE bot"
    }
}
