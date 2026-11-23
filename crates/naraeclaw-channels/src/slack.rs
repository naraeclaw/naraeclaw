//! Slack Socket Mode channel implementation.
//!
//! Uses Slack's Socket Mode API over WebSocket — no public URL required.
//! Protocol:
//! 1. Call `apps.connections.open` with the App-Level Token to obtain a WSS URL.
//! 2. Connect to the WSS URL and process event envelopes.
//! 3. ACK each envelope immediately with `{"envelope_id": "..."}`.
//! 4. Reply via `chat.postMessage` using the Bot Token.

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use naraeclaw_api::channel::{Channel, ChannelMessage, SendMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

const SLACK_API_BASE: &str = "https://slack.com/api";

/// How long a thread stays "joined" after the bot was last addressed in it.
const ACTIVE_THREAD_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Upper bound on remembered threads so the map cannot grow without limit.
const ACTIVE_THREAD_MAX: usize = 4096;

pub struct SlackChannel {
    app_token: String,
    bot_token: String,
    default_channel: Option<String>,
    /// Lazily-resolved bot user ID (e.g. `Uxxxxx`) discovered via `auth.test`.
    /// Used so the reply-intent classifier can tell self-mentions apart from
    /// mentions of other users.
    bot_user_id: tokio::sync::OnceCell<Option<String>>,
    /// When true, plain channel messages are dropped unless they are follow-ups
    /// in a thread the bot has already joined (mentioned in or replied to).
    mention_only: bool,
    /// Threads the bot has joined, keyed by `(channel, thread root ts)`.
    active_threads: Mutex<HashMap<(String, String), Instant>>,
}

impl SlackChannel {
    pub fn new(app_token: String, bot_token: String, default_channel: Option<String>) -> Self {
        Self {
            app_token,
            bot_token,
            default_channel,
            bot_user_id: tokio::sync::OnceCell::new(),
            mention_only: false,
            active_threads: Mutex::new(HashMap::new()),
        }
    }

    /// Only handle @mentions, DMs, and follow-ups in threads the bot joined.
    #[must_use]
    pub fn with_mention_only(mut self, mention_only: bool) -> Self {
        self.mention_only = mention_only;
        self
    }

    fn remember_thread(&self, channel: &str, root_ts: &str) {
        let mut threads = self
            .active_threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        threads.insert((channel.to_string(), root_ts.to_string()), now);
        if threads.len() > ACTIVE_THREAD_MAX {
            threads.retain(|_, seen| now.duration_since(*seen) < ACTIVE_THREAD_TTL);
        }
        while threads.len() > ACTIVE_THREAD_MAX {
            let Some(oldest) = threads
                .iter()
                .min_by_key(|(_, seen)| **seen)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            threads.remove(&oldest);
        }
    }

    fn is_remembered_thread(&self, channel: &str, root_ts: &str) -> bool {
        let threads = self
            .active_threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        threads
            .get(&(channel.to_string(), root_ts.to_string()))
            .is_some_and(|seen| seen.elapsed() < ACTIVE_THREAD_TTL)
    }

    /// Ask Slack whether the bot posted in, or was mentioned in, a thread.
    /// Covers threads joined before a restart. Fails closed (returns false).
    async fn bot_joined_thread(&self, channel: &str, root_ts: &str) -> bool {
        let Some(bot_id) = self.bot_self_id().await else {
            return false;
        };
        self.fetch_thread_messages(channel, root_ts, 200)
            .await
            .is_some_and(|messages| thread_involves_bot(&messages, &bot_id))
    }

    /// Fetch up to `limit` messages of a thread (root first) via
    /// `conversations.replies`. Returns `None` on any failure.
    async fn fetch_thread_messages(
        &self,
        channel: &str,
        root_ts: &str,
        limit: u32,
    ) -> Option<Vec<serde_json::Value>> {
        let limit = limit.to_string();
        let resp = match self
            .http_client()
            .get(format!("{SLACK_API_BASE}/conversations.replies"))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .query(&[("channel", channel), ("ts", root_ts), ("limit", &limit)])
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                warn!("Slack conversations.replies request failed: {e}");
                return None;
            }
        };
        let body: serde_json::Value = resp.json().await.ok()?;
        if !body["ok"].as_bool().unwrap_or(false) {
            warn!(
                "Slack conversations.replies failed: {}",
                body["error"].as_str().unwrap_or("unknown")
            );
            return None;
        }
        body["messages"].as_array().cloned()
    }

    /// When the bot first sees a thread reply, prepend the thread's root
    /// message so the model knows what "this" refers to. Messages that start
    /// a thread themselves are left unchanged.
    async fn attach_thread_parent(&self, msg: &mut ChannelMessage) {
        let Some(root) = msg.interruption_scope_id.clone() else {
            return;
        };
        let Some(messages) = self
            .fetch_thread_messages(&msg.reply_target, &root, 1)
            .await
        else {
            return;
        };
        let Some(parent) = messages.first() else {
            return;
        };
        if let Some(content) = with_thread_parent(parent, &msg.content) {
            info!("Slack: attached thread root {root} to message {}", msg.id);
            msg.content = content;
        }
    }

    /// Filter an incoming message and enrich it with thread context.
    /// Returns `None` when the message should be dropped.
    async fn prepare_incoming(&self, mut msg: ChannelMessage) -> Option<ChannelMessage> {
        let root = thread_root(&msg).map(str::to_string);
        let first_seen = root
            .as_deref()
            .is_none_or(|r| !self.is_remembered_thread(&msg.reply_target, r));
        if !self.admit(&msg).await {
            return None;
        }
        if let Some(root) = root.as_deref() {
            self.remember_thread(&msg.reply_target, root);
        }
        if first_seen {
            self.attach_thread_parent(&mut msg).await;
        }
        Some(msg)
    }

    /// Decide whether an incoming message should reach the orchestrator.
    async fn admit(&self, msg: &ChannelMessage) -> bool {
        if !self.mention_only {
            return true;
        }
        let root = thread_root(msg);
        if msg.is_mention {
            if let Some(root) = root {
                self.remember_thread(&msg.reply_target, root);
            }
            return true;
        }
        // Plain channel message: only thread follow-ups can pass.
        let Some(root) = msg.interruption_scope_id.as_deref() else {
            return false;
        };
        if self.is_remembered_thread(&msg.reply_target, root) {
            return true;
        }
        if self.bot_joined_thread(&msg.reply_target, root).await {
            self.remember_thread(&msg.reply_target, root);
            return true;
        }
        false
    }

    /// Resolve the bot's own user ID via `auth.test` and cache it for the
    /// lifetime of the channel. Returns `None` if the call fails so the caller
    /// can fall back gracefully — the classifier still works, it just won't
    /// have the extra grounding signal.
    async fn resolve_bot_user_id(&self) -> Option<String> {
        let resp = self
            .http_client()
            .post(format!("{SLACK_API_BASE}/auth.test"))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .send()
            .await
            .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        if !body["ok"].as_bool().unwrap_or(false) {
            warn!(
                "Slack auth.test failed: {}",
                body["error"].as_str().unwrap_or("unknown")
            );
            return None;
        }
        body["user_id"].as_str().map(|s| s.to_string())
    }

    fn http_client(&self) -> reqwest::Client {
        naraeclaw_config::schema::build_runtime_proxy_client("channel.slack")
    }

    /// Call `apps.connections.open` to get a Socket Mode WSS URL.
    async fn open_wss_url(&self) -> Result<String> {
        let resp = self
            .http_client()
            .post(format!("{SLACK_API_BASE}/apps.connections.open"))
            .header("Authorization", format!("Bearer {}", self.app_token))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send()
            .await
            .context("apps.connections.open request failed")?;

        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse connections.open response")?;

        if !body["ok"].as_bool().unwrap_or(false) {
            bail!(
                "apps.connections.open failed: {}",
                body["error"].as_str().unwrap_or("unknown error")
            );
        }

        body["url"]
            .as_str()
            .map(|s| s.to_string())
            .context("No 'url' field in connections.open response")
    }

    /// Send a message via `chat.postMessage`.
    async fn post_message(&self, channel: &str, text: &str, thread_ts: Option<&str>) -> Result<()> {
        #[derive(Serialize)]
        struct PostMessage<'a> {
            channel: &'a str,
            text: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            thread_ts: Option<&'a str>,
        }

        let payload = PostMessage {
            channel,
            text,
            thread_ts,
        };

        let resp = self
            .http_client()
            .post(format!("{SLACK_API_BASE}/chat.postMessage"))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&payload)
            .send()
            .await
            .context("chat.postMessage request failed")?;

        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse postMessage response")?;

        if !body["ok"].as_bool().unwrap_or(false) {
            bail!(
                "chat.postMessage failed: {}",
                body["error"].as_str().unwrap_or("unknown error")
            );
        }

        Ok(())
    }

    /// Add a reaction emoji to a message.
    async fn add_reaction_inner(&self, channel: &str, ts: &str, emoji: &str) -> Result<()> {
        #[derive(Serialize)]
        struct AddReaction<'a> {
            channel: &'a str,
            timestamp: &'a str,
            name: &'a str,
        }

        let payload = AddReaction {
            channel,
            timestamp: ts,
            name: emoji,
        };

        let resp = self
            .http_client()
            .post(format!("{SLACK_API_BASE}/reactions.add"))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&payload)
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;
        if !body["ok"].as_bool().unwrap_or(false) {
            // "already_reacted" is benign
            let err = body["error"].as_str().unwrap_or("");
            if err != "already_reacted" {
                warn!("reactions.add failed: {err}");
            }
        }

        Ok(())
    }

    /// Remove a reaction emoji from a message.
    async fn remove_reaction_inner(&self, channel: &str, ts: &str, emoji: &str) -> Result<()> {
        #[derive(Serialize)]
        struct RemoveReaction<'a> {
            channel: &'a str,
            timestamp: &'a str,
            name: &'a str,
        }

        let payload = RemoveReaction {
            channel,
            timestamp: ts,
            name: emoji,
        };

        let resp = self
            .http_client()
            .post(format!("{SLACK_API_BASE}/reactions.remove"))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&payload)
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;
        if !body["ok"].as_bool().unwrap_or(false) {
            let err = body["error"].as_str().unwrap_or("");
            if err != "no_reaction" {
                warn!("reactions.remove failed: {err}");
            }
        }

        Ok(())
    }
}

// ── Slack Socket Mode envelope types ─────────────────────────────

#[derive(Debug, Deserialize)]
struct SocketEnvelope {
    #[serde(rename = "type")]
    event_type: String,
    envelope_id: Option<String>,
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct AckPayload<'a> {
    envelope_id: &'a str,
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }

    async fn bot_self_id(&self) -> Option<String> {
        self.bot_user_id
            .get_or_init(|| async { self.resolve_bot_user_id().await })
            .await
            .clone()
    }

    async fn send(&self, message: &SendMessage) -> Result<()> {
        let channel = if message.recipient.is_empty() {
            self.default_channel
                .as_deref()
                .context("No recipient and no default_channel configured")?
        } else {
            &message.recipient
        };

        self.post_message(channel, &message.content, message.thread_ts.as_deref())
            .await?;
        if let Some(ts) = message.thread_ts.as_deref() {
            self.remember_thread(channel, ts);
        }
        Ok(())
    }

    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> Result<()> {
        loop {
            let wss_url = match self.open_wss_url().await {
                Ok(u) => u,
                Err(e) => {
                    warn!("Slack Socket Mode: failed to get WSS URL: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            info!("Slack Socket Mode: connecting to WSS...");

            let (ws_stream, _) = match connect_async(&wss_url).await {
                Ok(s) => s,
                Err(e) => {
                    warn!("Slack Socket Mode: WebSocket connection failed: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            info!("Slack Socket Mode: connected");
            naraeclaw_runtime::health::mark_component_ok("slack");

            let (mut write, mut read) = ws_stream.split();
            // Send a WebSocket ping every 30 s so we detect silent TCP death
            // (e.g. after macOS sleep) without relying on Slack's server-sent
            // disconnect events, which stop arriving when the connection is dead.
            let mut ping_tick = tokio::time::interval(std::time::Duration::from_secs(30));
            ping_tick.tick().await; // consume the immediate first tick
            let mut awaiting_pong = false;

            'inner: loop {
                tokio::select! {
                    maybe_msg = read.next() => {
                        match maybe_msg {
                            Some(Ok(Message::Text(text))) => {
                                debug!("Slack WS recv: {text}");

                                let envelope: SocketEnvelope = match serde_json::from_str(&text) {
                                    Ok(e) => e,
                                    Err(e) => {
                                        warn!("Slack Socket Mode: failed to parse envelope: {e}");
                                        continue 'inner;
                                    }
                                };

                                // ACK immediately
                                if let Some(ref id) = envelope.envelope_id {
                                    let ack = serde_json::to_string(&AckPayload { envelope_id: id })
                                        .unwrap_or_default();
                                    if let Err(e) = write.send(Message::Text(ack.into())).await {
                                        warn!("Slack Socket Mode: failed to send ACK: {e}");
                                    }
                                }

                                match envelope.event_type.as_str() {
                                    "hello" => {
                                        info!("Slack Socket Mode: handshake complete");
                                    }
                                    "disconnect" => {
                                        info!(
                                            "Slack Socket Mode: server requested disconnect, reconnecting..."
                                        );
                                        break 'inner;
                                    }
                                    "events_api" => {
                                        if let Some(payload) = envelope.payload
                                            && let Some(channel_msg) = parse_events_api(&payload)
                                        {
                                            let id = channel_msg.id.clone();
                                            let Some(channel_msg) =
                                                self.prepare_incoming(channel_msg).await
                                            else {
                                                debug!(
                                                    "Slack: dropping non-mention message {id} (mention_only)"
                                                );
                                                continue 'inner;
                                            };
                                            if tx.send(channel_msg).await.is_err() {
                                                return Ok(());
                                            }
                                        }
                                    }
                                    other => {
                                        debug!("Slack Socket Mode: unhandled envelope type '{other}'");
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("Slack Socket Mode: WebSocket closed, reconnecting...");
                                break 'inner;
                            }
                            Some(Ok(Message::Ping(data))) => {
                                if let Err(e) = write.send(Message::Pong(data)).await {
                                    warn!("Slack Socket Mode: pong failed: {e}");
                                }
                            }
                            Some(Ok(Message::Pong(_))) => {
                                awaiting_pong = false;
                            }
                            Some(Ok(_)) => {}
                            Some(Err(e)) => {
                                naraeclaw_runtime::health::mark_component_error("slack", e.to_string());
                                warn!("Slack Socket Mode: WebSocket error: {e}");
                                break 'inner;
                            }
                            None => break 'inner,
                        }
                    }
                    _ = ping_tick.tick() => {
                        if awaiting_pong {
                            warn!("Slack Socket Mode: pong timeout, reconnecting...");
                            break 'inner;
                        }
                        awaiting_pong = true;
                        if let Err(e) = write.send(Message::Ping(vec![].into())).await {
                            warn!("Slack Socket Mode: heartbeat ping failed: {e}");
                            break 'inner;
                        }
                    }
                }
            }

            // Wait before reconnecting
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    async fn health_check(&self) -> bool {
        // Try to verify Bot Token is valid via auth.test
        let Ok(resp) = self
            .http_client()
            .post(format!("{SLACK_API_BASE}/auth.test"))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .send()
            .await
        else {
            return false;
        };

        let Ok(body) = resp.json::<serde_json::Value>().await else {
            return false;
        };

        body["ok"].as_bool().unwrap_or(false)
    }

    async fn add_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        let ts = message_ts(channel_id, message_id);
        self.add_reaction_inner(channel_id, ts, emoji).await
    }

    async fn remove_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        let ts = message_ts(channel_id, message_id);
        self.remove_reaction_inner(channel_id, ts, emoji).await
    }
}

/// The orchestrator passes `ChannelMessage::id` (`slack_<channel>_<ts>`) as the
/// reaction target; Slack's reactions API needs the bare message `ts`.
fn message_ts<'a>(channel_id: &str, message_id: &'a str) -> &'a str {
    message_id
        .strip_prefix("slack_")
        .and_then(|rest| rest.strip_prefix(channel_id))
        .and_then(|rest| rest.strip_prefix('_'))
        .unwrap_or(message_id)
}

/// Maximum characters of a thread root message injected as context.
const THREAD_PARENT_MAX_CHARS: usize = 4000;

/// Prefix `current` with the thread's root message. Returns `None` when the
/// root has no text worth adding.
fn with_thread_parent(parent: &serde_json::Value, current: &str) -> Option<String> {
    let text = parent["text"].as_str().map(str::trim).unwrap_or("");
    if text.is_empty() {
        return None;
    }
    let text: String = text.chars().take(THREAD_PARENT_MAX_CHARS).collect();
    let author = parent["user"]
        .as_str()
        .map(|u| format!("<@{u}>"))
        .unwrap_or_else(|| "unknown".to_string());
    Some(format!(
        "[Slack thread root message by {author} — context for the reply below]\n{text}\n\n[Reply in thread]\n{current}"
    ))
}

// ── parse_events_api helpers (CC ≤ 4 each) ───────────────────────

struct SlackEventFields {
    user: String,
    channel: String,
    text: String,
    ts: String,
    thread_ts: Option<String>,
    /// True when the bot was directly @mentioned (`app_mention`) or the channel is a DM.
    /// False for plain channel messages — reply intent is decided by the orchestrator.
    is_direct_mention: bool,
}

/// Extract the `event` object and its `type` string from an `events_api` payload.
/// CC=3 (base + 2×?)
fn get_event_type(payload: &serde_json::Value) -> Option<(&serde_json::Value, &str)> {
    let event = payload.get("event")?;
    let event_type = event["type"].as_str()?;
    Some((event, event_type))
}

/// Returns true for event types this bot handles.
/// CC=2 (base + 1×||)
fn is_supported_event(event_type: &str) -> bool {
    event_type == "message" || event_type == "app_mention"
}

/// Returns true for bot-generated or system messages (subtypes) to ignore.
/// CC=2 (base + 1×||)
fn is_passive_message(event: &serde_json::Value) -> bool {
    event.get("subtype").is_some() || event.get("bot_id").is_some()
}

/// Returns true when this message should be passed to the agent.
///
/// Slack fires TWO events for a channel @mention: `app_mention` + `message`.
/// To avoid double-processing, `message` events in public channels that contain
/// a `<@...>` mention pattern are dropped here — `app_mention` handles those.
/// Plain channel messages (no mention) are routed so the reply-intent classifier
/// can decide based on conversation context.
/// CC=3 (base + 2 branches)
fn is_routable(event_type: &str, channel: &str, text: &str) -> bool {
    match event_type {
        "app_mention" => true,
        "message" if channel.starts_with('D') => true,
        // Public channel: only route if there is no @mention in the text.
        // Messages with <@...> will also arrive as app_mention events.
        "message" => !text.contains("<@"),
        _ => false,
    }
}

/// Returns true when the bot was directly addressed (@mention or DM).
/// CC=2 (base + 1×||)
fn is_direct_mention(event_type: &str, channel: &str) -> bool {
    event_type == "app_mention" || channel.starts_with('D')
}

/// Returns Some(()) when all routing guards pass, None otherwise.
/// CC=4 (base + 3×&&)
fn guard_event(event_type: &str, event: &serde_json::Value, f: &SlackEventFields) -> Option<()> {
    (is_supported_event(event_type)
        && !is_passive_message(event)
        && !f.text.is_empty()
        && is_routable(event_type, &f.channel, &f.text))
    .then_some(())
}

/// Extract required message fields from an event object.
/// CC=4 (base + 3×?)
fn extract_event_fields(event: &serde_json::Value, event_type: &str) -> Option<SlackEventFields> {
    let channel = event["channel"].as_str()?.to_string();
    let direct = is_direct_mention(event_type, &channel);
    Some(SlackEventFields {
        user: event["user"].as_str()?.to_string(),
        ts: event["ts"].as_str()?.to_string(),
        text: event["text"].as_str().unwrap_or("").to_string(),
        thread_ts: event["thread_ts"].as_str().map(str::to_string),
        channel,
        is_direct_mention: direct,
    })
}

/// Build a ChannelMessage from validated event fields.
/// CC=1 (no branches; all guards have already passed)
fn build_channel_message(f: SlackEventFields) -> ChannelMessage {
    let timestamp =
        f.ts.split('.')
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_default();
    ChannelMessage {
        id: format!("slack_{}_{}", f.channel, f.ts),
        sender: f.user,
        reply_target: f.channel.clone(),
        content: f.text,
        channel: "slack".to_string(),
        timestamp,
        // Thread root: replies and conversation history are keyed per thread.
        thread_ts: Some(f.thread_ts.clone().unwrap_or(f.ts)),
        interruption_scope_id: f.thread_ts,
        attachments: vec![],
        // true  → bot was @mentioned or this is a DM; skip reply-intent classifier
        // false → general channel message; orchestrator LLM decides whether to reply
        is_mention: f.is_direct_mention,
    }
}

/// Thread root of a message: its parent thread, or the message itself when it
/// starts a new thread (the bot replies under `thread_ts`).
fn thread_root(msg: &ChannelMessage) -> Option<&str> {
    msg.interruption_scope_id
        .as_deref()
        .or(msg.thread_ts.as_deref())
}

/// True when any message in a `conversations.replies` result was posted by the
/// bot or @mentions it.
fn thread_involves_bot(messages: &[serde_json::Value], bot_id: &str) -> bool {
    let mention = format!("<@{bot_id}>");
    messages.iter().any(|m| {
        m["user"].as_str() == Some(bot_id)
            || m["text"].as_str().is_some_and(|t| t.contains(&mention))
    })
}

/// Parse a Slack `events_api` payload into a `ChannelMessage`.
/// CC=4 (base + 3×?) — delegates all decisions to focused helpers.
fn parse_events_api(payload: &serde_json::Value) -> Option<ChannelMessage> {
    let (event, event_type) = get_event_type(payload)?;
    let f = extract_event_fields(event, event_type)?;
    guard_event(event_type, event, &f)?;
    Some(build_channel_message(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 테스트 픽스처 ──────────────────────────────────────────────

    fn dm_event(user: &str, text: &str, ts: &str) -> serde_json::Value {
        serde_json::json!({ "event": { "type": "message", "user": user, "channel": "D456", "text": text, "ts": ts } })
    }

    fn mention_event(user: &str, channel: &str, text: &str, ts: &str) -> serde_json::Value {
        serde_json::json!({ "event": { "type": "app_mention", "user": user, "channel": channel, "text": text, "ts": ts } })
    }

    // ── get_event_type (CC=3) ──────────────────────────────────────

    #[test]
    fn get_event_type_extracts_event_and_type() {
        let payload = dm_event("U1", "hi", "1.0");
        let (_, event_type) = get_event_type(&payload).unwrap();
        assert_eq!(event_type, "message");
    }

    #[test]
    fn get_event_type_missing_event_returns_none() {
        assert!(get_event_type(&serde_json::json!({})).is_none());
    }

    #[test]
    fn get_event_type_missing_type_returns_none() {
        assert!(get_event_type(&serde_json::json!({ "event": {} })).is_none());
    }

    // ── is_supported_event (CC=2) ──────────────────────────────────

    #[test]
    fn is_supported_event_accepts_message_and_app_mention() {
        assert!(is_supported_event("message"));
        assert!(is_supported_event("app_mention"));
    }

    #[test]
    fn is_supported_event_rejects_other_types() {
        assert!(!is_supported_event("reaction_added"));
        assert!(!is_supported_event(""));
    }

    // ── is_passive_message (CC=2) ──────────────────────────────────

    #[test]
    fn is_passive_message_detects_bot_id() {
        let event = serde_json::json!({ "bot_id": "B1" });
        assert!(is_passive_message(&event));
    }

    #[test]
    fn is_passive_message_detects_subtype() {
        let event = serde_json::json!({ "subtype": "message_changed" });
        assert!(is_passive_message(&event));
    }

    #[test]
    fn is_passive_message_passes_normal_user_message() {
        let event = serde_json::json!({ "user": "U1" });
        assert!(!is_passive_message(&event));
    }

    // ── is_routable (CC=3) ────────────────────────────────────────

    #[test]
    fn is_routable_app_mention_in_public_channel() {
        assert!(is_routable("app_mention", "C123", "hey"));
    }

    #[test]
    fn is_routable_dm_message() {
        assert!(is_routable("message", "D123", "hello"));
    }

    #[test]
    fn is_routable_channel_message_without_mention() {
        // Plain channel chat (no @) is routed; reply-intent classifier decides.
        assert!(is_routable("message", "C123", "hey team"));
    }

    #[test]
    fn is_routable_channel_message_with_mention_dropped() {
        // Slack fires app_mention + message for @mentions — drop the message duplicate.
        assert!(!is_routable("message", "C123", "<@U0B17RD9MN3> 살아나라"));
    }

    // ── guard_event (CC=4) ────────────────────────────────────────

    fn valid_fields() -> SlackEventFields {
        SlackEventFields {
            user: "U1".into(),
            channel: "D1".into(),
            text: "hello".into(),
            ts: "1.0".into(),
            thread_ts: None,
            is_direct_mention: true,
        }
    }

    #[test]
    fn guard_event_passes_valid_dm_message() {
        let event = serde_json::json!({ "type": "message" });
        assert!(guard_event("message", &event, &valid_fields()).is_some());
    }

    #[test]
    fn guard_event_rejects_unsupported_type() {
        let event = serde_json::json!({});
        assert!(guard_event("reaction_added", &event, &valid_fields()).is_none());
    }

    #[test]
    fn guard_event_rejects_passive_message() {
        let event = serde_json::json!({ "bot_id": "B1" });
        assert!(guard_event("message", &event, &valid_fields()).is_none());
    }

    #[test]
    fn guard_event_rejects_empty_text() {
        let event = serde_json::json!({});
        let mut f = valid_fields();
        f.text = String::new();
        assert!(guard_event("message", &event, &f).is_none());
    }

    // ── extract_event_fields (CC=4) ───────────────────────────────

    #[test]
    fn extract_event_fields_returns_all_fields() {
        let event = serde_json::json!({
            "user": "U1", "channel": "D1", "ts": "1.0", "text": "hi", "thread_ts": "0.0"
        });
        let f = extract_event_fields(&event, "message").unwrap();
        assert_eq!(f.user, "U1");
        assert_eq!(f.channel, "D1");
        assert_eq!(f.ts, "1.0");
        assert_eq!(f.text, "hi");
        assert_eq!(f.thread_ts.as_deref(), Some("0.0"));
        assert!(f.is_direct_mention); // D1 is a DM channel
    }

    #[test]
    fn extract_event_fields_missing_user_returns_none() {
        let event = serde_json::json!({ "channel": "D1", "ts": "1.0", "text": "hi" });
        assert!(extract_event_fields(&event, "message").is_none());
    }

    #[test]
    fn extract_event_fields_missing_channel_returns_none() {
        let event = serde_json::json!({ "user": "U1", "ts": "1.0", "text": "hi" });
        assert!(extract_event_fields(&event, "message").is_none());
    }

    #[test]
    fn extract_event_fields_missing_ts_returns_none() {
        let event = serde_json::json!({ "user": "U1", "channel": "D1", "text": "hi" });
        assert!(extract_event_fields(&event, "message").is_none());
    }

    #[test]
    fn extract_event_fields_app_mention_sets_direct_mention() {
        let event = serde_json::json!({
            "user": "U1", "channel": "C123", "ts": "1.0", "text": "@bot hi"
        });
        let f = extract_event_fields(&event, "app_mention").unwrap();
        assert!(f.is_direct_mention);
    }

    #[test]
    fn extract_event_fields_channel_message_not_direct_mention() {
        let event = serde_json::json!({
            "user": "U1", "channel": "C123", "ts": "1.0", "text": "hey everyone"
        });
        let f = extract_event_fields(&event, "message").unwrap();
        assert!(!f.is_direct_mention);
    }

    // ── build_channel_message (CC=1) ─────────────────────────────

    #[test]
    fn build_channel_message_maps_fields_correctly() {
        let f = SlackEventFields {
            user: "U1".into(),
            channel: "D9".into(),
            text: "hello".into(),
            ts: "1700000000.123456".into(),
            thread_ts: Some("1700000000.000000".into()),
            is_direct_mention: true,
        };
        let msg = build_channel_message(f);
        assert_eq!(msg.sender, "U1");
        assert_eq!(msg.reply_target, "D9");
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.id, "slack_D9_1700000000.123456");
        assert_eq!(msg.timestamp, 1700000000);
        assert_eq!(msg.thread_ts.as_deref(), Some("1700000000.000000"));
        assert_eq!(
            msg.interruption_scope_id.as_deref(),
            Some("1700000000.000000")
        );
        assert!(msg.is_mention);
    }

    // ── parse_events_api integration (CC=4) ──────────────────────

    #[test]
    fn dm_message_parsed() {
        let payload = dm_event("U123", "Hello!", "1234567890.123456");
        let msg = parse_events_api(&payload).unwrap();
        assert_eq!(msg.sender, "U123");
        assert_eq!(msg.reply_target, "D456");
        assert_eq!(msg.content, "Hello!");
        assert_eq!(msg.channel, "slack");
        assert_eq!(msg.id, "slack_D456_1234567890.123456");
        assert_eq!(msg.thread_ts.as_deref(), Some("1234567890.123456"));
        assert!(msg.is_mention);
    }

    #[test]
    fn app_mention_in_channel_parsed() {
        let payload = mention_event("U123", "C456", "@bot help me", "1234567890.123456");
        let msg = parse_events_api(&payload).unwrap();
        assert_eq!(msg.sender, "U123");
        assert_eq!(msg.reply_target, "C456");
        assert_eq!(msg.content, "@bot help me");
        assert!(msg.is_mention);
    }

    #[test]
    fn channel_message_without_mention_is_routed_with_is_mention_false() {
        // Non-mention channel messages are now routed; reply-intent classifier decides.
        let payload = serde_json::json!({
            "event": { "type": "message", "user": "U123", "channel": "C456", "text": "hi", "ts": "1.0" }
        });
        let msg = parse_events_api(&payload).unwrap();
        assert!(!msg.is_mention);
    }

    #[test]
    fn ignores_bot_messages() {
        let payload = serde_json::json!({
            "event": { "type": "message", "user": "U123", "bot_id": "B999", "channel": "D456", "text": "I am a bot", "ts": "1.0" }
        });
        assert!(parse_events_api(&payload).is_none());
    }

    #[test]
    fn ignores_message_subtypes() {
        let payload = serde_json::json!({
            "event": { "type": "message", "subtype": "message_changed", "user": "U123", "channel": "D456", "text": "edited", "ts": "1.0" }
        });
        assert!(parse_events_api(&payload).is_none());
    }

    #[test]
    fn ignores_empty_text() {
        let payload = dm_event("U123", "", "1.0");
        assert!(parse_events_api(&payload).is_none());
    }

    #[test]
    fn threaded_dm_sets_interruption_scope() {
        let payload = serde_json::json!({
            "event": { "type": "message", "user": "U123", "channel": "D456",
                       "text": "reply", "ts": "1234567891.0", "thread_ts": "1234567890.0" }
        });
        let msg = parse_events_api(&payload).unwrap();
        assert_eq!(msg.interruption_scope_id.as_deref(), Some("1234567890.0"));
    }

    #[test]
    fn timestamp_parsed_from_ts() {
        let payload = dm_event("U123", "hi", "1700000000.123456");
        let msg = parse_events_api(&payload).unwrap();
        assert_eq!(msg.timestamp, 1700000000);
    }

    // ── mention_only admission ─────────────────────────────────────

    fn channel_message_event(text: &str, ts: &str, thread_ts: Option<&str>) -> serde_json::Value {
        let mut event = serde_json::json!({ "type": "message", "user": "U1", "channel": "C1", "text": text, "ts": ts });
        if let Some(t) = thread_ts {
            event["thread_ts"] = serde_json::json!(t);
        }
        serde_json::json!({ "event": event })
    }

    fn mention_only_channel() -> SlackChannel {
        SlackChannel::new("xapp-test".into(), "xoxb-test".into(), None).with_mention_only(true)
    }

    #[tokio::test]
    async fn admit_allows_everything_when_mention_only_disabled() {
        let ch = SlackChannel::new("xapp-test".into(), "xoxb-test".into(), None);
        let msg = parse_events_api(&channel_message_event("chatter", "1.0", None)).unwrap();
        assert!(ch.admit(&msg).await);
    }

    #[tokio::test]
    async fn admit_drops_top_level_chatter_when_mention_only() {
        let ch = mention_only_channel();
        let msg = parse_events_api(&channel_message_event("chatter", "1.0", None)).unwrap();
        assert!(!ch.admit(&msg).await);
    }

    #[tokio::test]
    async fn admit_allows_mentions_and_dms_when_mention_only() {
        let ch = mention_only_channel();
        let mention = parse_events_api(&mention_event("U1", "C1", "<@UBOT> hi", "1.0")).unwrap();
        assert!(ch.admit(&mention).await);
        let dm = parse_events_api(&dm_event("U1", "hi", "2.0")).unwrap();
        assert!(ch.admit(&dm).await);
    }

    #[tokio::test]
    async fn admit_allows_follow_up_in_thread_started_by_mention() {
        let ch = mention_only_channel();
        let mention = parse_events_api(&mention_event("U1", "C1", "<@UBOT> hi", "1.0")).unwrap();
        assert!(ch.admit(&mention).await);
        let follow_up =
            parse_events_api(&channel_message_event("and also", "1.5", Some("1.0"))).unwrap();
        assert!(ch.admit(&follow_up).await);
    }

    #[tokio::test]
    async fn admit_allows_follow_up_in_thread_bot_replied_to() {
        let ch = mention_only_channel();
        ch.remember_thread("C1", "5.0");
        let follow_up =
            parse_events_api(&channel_message_event("thanks", "5.1", Some("5.0"))).unwrap();
        assert!(ch.admit(&follow_up).await);
    }

    #[test]
    fn remembered_threads_are_scoped_by_channel() {
        let ch = mention_only_channel();
        ch.remember_thread("C1", "1.0");
        assert!(ch.is_remembered_thread("C1", "1.0"));
        assert!(!ch.is_remembered_thread("C2", "1.0"));
        assert!(!ch.is_remembered_thread("C1", "2.0"));
    }

    #[test]
    fn remembered_threads_are_bounded() {
        let ch = mention_only_channel();
        for i in 0..(ACTIVE_THREAD_MAX + 10) {
            ch.remember_thread("C1", &format!("{i}.0"));
        }
        let len = ch.active_threads.lock().unwrap().len();
        assert_eq!(len, ACTIVE_THREAD_MAX);
        assert!(ch.is_remembered_thread("C1", &format!("{}.0", ACTIVE_THREAD_MAX + 9)));
    }

    #[test]
    fn thread_involves_bot_detects_bot_posts_and_mentions() {
        let posted = vec![
            serde_json::json!({ "user": "U1", "text": "q" }),
            serde_json::json!({ "user": "UBOT", "text": "a" }),
        ];
        assert!(thread_involves_bot(&posted, "UBOT"));
        let mentioned = vec![serde_json::json!({ "user": "U1", "text": "<@UBOT> look" })];
        assert!(thread_involves_bot(&mentioned, "UBOT"));
        let unrelated = vec![serde_json::json!({ "user": "U1", "text": "<@U2> look" })];
        assert!(!thread_involves_bot(&unrelated, "UBOT"));
    }

    // ── thread context ─────────────────────────────────────────────

    #[test]
    fn thread_replies_share_the_root_as_thread_ts() {
        let first = parse_events_api(&mention_event("U1", "C1", "<@UBOT> q", "1.0")).unwrap();
        let reply = parse_events_api(&channel_message_event("more", "1.5", Some("1.0"))).unwrap();
        assert_eq!(first.thread_ts.as_deref(), Some("1.0"));
        assert_eq!(reply.thread_ts.as_deref(), Some("1.0"));
        assert_eq!(reply.interruption_scope_id.as_deref(), Some("1.0"));
    }

    #[test]
    fn message_ts_strips_internal_id_prefix() {
        assert_eq!(
            message_ts("C1", "slack_C1_1700000000.123456"),
            "1700000000.123456"
        );
        assert_eq!(message_ts("C1", "1700000000.123456"), "1700000000.123456");
        assert_eq!(message_ts("C2", "slack_C1_1.0"), "slack_C1_1.0");
    }

    #[test]
    fn with_thread_parent_prefixes_root_text() {
        let parent =
            serde_json::json!({ "user": "U1", "text": "<https://news.hada.io/topic?id=37555>" });
        let out = with_thread_parent(&parent, "<@UBOT> 이거 읽을 수 있니?").unwrap();
        assert!(out.contains("<@U1>"));
        assert!(out.contains("https://news.hada.io/topic?id=37555"));
        assert!(out.ends_with("<@UBOT> 이거 읽을 수 있니?"));
    }

    #[test]
    fn with_thread_parent_skips_empty_and_truncates() {
        assert!(
            with_thread_parent(&serde_json::json!({ "user": "U1", "text": "  " }), "q").is_none()
        );
        let long = "a".repeat(THREAD_PARENT_MAX_CHARS + 100);
        let out = with_thread_parent(&serde_json::json!({ "text": long }), "q").unwrap();
        assert!(out.contains(&"a".repeat(THREAD_PARENT_MAX_CHARS)));
        assert!(!out.contains(&"a".repeat(THREAD_PARENT_MAX_CHARS + 1)));
    }

    #[tokio::test]
    async fn prepare_incoming_leaves_thread_starters_untouched() {
        let ch = mention_only_channel();
        let msg = parse_events_api(&mention_event("U1", "C1", "<@UBOT> hi", "1.0")).unwrap();
        let out = ch.prepare_incoming(msg).await.unwrap();
        assert_eq!(out.content, "<@UBOT> hi");
        assert!(ch.is_remembered_thread("C1", "1.0"));
    }

    #[tokio::test]
    async fn prepare_incoming_skips_parent_fetch_for_known_threads() {
        let ch = mention_only_channel();
        ch.remember_thread("C1", "1.0");
        let reply = parse_events_api(&channel_message_event("more", "1.5", Some("1.0"))).unwrap();
        let out = ch.prepare_incoming(reply).await.unwrap();
        assert_eq!(out.content, "more");
    }
}
