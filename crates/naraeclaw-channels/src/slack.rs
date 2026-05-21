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
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

const SLACK_API_BASE: &str = "https://slack.com/api";

pub struct SlackChannel {
    app_token: String,
    bot_token: String,
    default_channel: Option<String>,
    /// Lazily-resolved bot user ID (e.g. `Uxxxxx`) discovered via `auth.test`.
    /// Used so the reply-intent classifier can tell self-mentions apart from
    /// mentions of other users.
    bot_user_id: tokio::sync::OnceCell<Option<String>>,
}

impl SlackChannel {
    pub fn new(app_token: String, bot_token: String, default_channel: Option<String>) -> Self {
        Self {
            app_token,
            bot_token,
            default_channel,
            bot_user_id: tokio::sync::OnceCell::new(),
        }
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
            .await
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
            let mut ping_tick =
                tokio::time::interval(std::time::Duration::from_secs(30));
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
                                            && tx.send(channel_msg).await.is_err()
                                        {
                                            return Ok(());
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
        self.add_reaction_inner(channel_id, message_id, emoji).await
    }

    async fn remove_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        self.remove_reaction_inner(channel_id, message_id, emoji)
            .await
    }
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
        thread_ts: Some(f.ts),
        interruption_scope_id: f.thread_ts,
        attachments: vec![],
        // true  → bot was @mentioned or this is a DM; skip reply-intent classifier
        // false → general channel message; orchestrator LLM decides whether to reply
        is_mention: f.is_direct_mention,
    }
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
        assert_eq!(msg.thread_ts.as_deref(), Some("1700000000.123456"));
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
}
