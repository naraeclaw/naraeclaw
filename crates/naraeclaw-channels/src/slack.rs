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
}

impl SlackChannel {
    pub fn new(app_token: String, bot_token: String, default_channel: Option<String>) -> Self {
        Self {
            app_token,
            bot_token,
            default_channel,
        }
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

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Slack WS recv: {text}");

                        let envelope: SocketEnvelope = match serde_json::from_str(&text) {
                            Ok(e) => e,
                            Err(e) => {
                                warn!("Slack Socket Mode: failed to parse envelope: {e}");
                                continue;
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
                                break;
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
                    Ok(Message::Close(_)) => {
                        info!("Slack Socket Mode: WebSocket closed, reconnecting...");
                        break;
                    }
                    Ok(Message::Ping(data)) => {
                        if let Err(e) = write.send(Message::Pong(data)).await {
                            warn!("Slack Socket Mode: pong failed: {e}");
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        naraeclaw_runtime::health::mark_component_error("slack", e.to_string());
                        warn!("Slack Socket Mode: WebSocket error: {e}");
                        break;
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

/// Parse a Slack `events_api` payload into a `ChannelMessage`.
/// Only handles `message` events from real users (ignores bots and subtypes).
fn parse_events_api(payload: &serde_json::Value) -> Option<ChannelMessage> {
    let event = payload.get("event")?;
    let event_type = event["type"].as_str()?;

    if event_type != "message" {
        return None;
    }

    // Ignore bot messages and edited/deleted subtypes
    if event.get("subtype").is_some() {
        return None;
    }
    if event.get("bot_id").is_some() {
        return None;
    }

    let user = event["user"].as_str()?.to_string();
    let channel = event["channel"].as_str()?.to_string();
    let text = event["text"].as_str().unwrap_or("").to_string();
    let ts = event["ts"].as_str()?.to_string();

    if text.is_empty() {
        return None;
    }

    // thread_ts is set when the message is inside a thread
    let thread_ts = event["thread_ts"].as_str().map(|s| s.to_string());
    // Use thread_ts as the interruption scope when in a thread
    let interruption_scope_id = thread_ts.clone();

    let reply_target = channel.clone();

    let timestamp = ts
        .split('.')
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_default();

    Some(ChannelMessage {
        id: format!("slack_{channel}_{ts}"),
        sender: user,
        reply_target,
        content: text,
        channel: "slack".to_string(),
        timestamp,
        thread_ts: Some(ts),
        interruption_scope_id,
        attachments: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event_payload(user: &str, channel: &str, text: &str, ts: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "event_callback",
            "event": {
                "type": "message",
                "user": user,
                "channel": channel,
                "text": text,
                "ts": ts
            }
        })
    }

    #[test]
    fn parse_basic_message() {
        let payload = make_event_payload("U123", "C456", "Hello!", "1234567890.123456");
        let msg = parse_events_api(&payload).unwrap();
        assert_eq!(msg.sender, "U123");
        assert_eq!(msg.reply_target, "C456");
        assert_eq!(msg.content, "Hello!");
        assert_eq!(msg.channel, "slack");
        assert_eq!(msg.id, "slack_C456_1234567890.123456");
        assert_eq!(msg.thread_ts.as_deref(), Some("1234567890.123456"));
    }

    #[test]
    fn ignores_bot_messages() {
        let payload = serde_json::json!({
            "event": {
                "type": "message",
                "user": "U123",
                "bot_id": "B999",
                "channel": "C456",
                "text": "I am a bot",
                "ts": "1234567890.000001"
            }
        });
        assert!(parse_events_api(&payload).is_none());
    }

    #[test]
    fn ignores_message_subtypes() {
        let payload = serde_json::json!({
            "event": {
                "type": "message",
                "subtype": "message_changed",
                "user": "U123",
                "channel": "C456",
                "text": "edited",
                "ts": "1234567890.000002"
            }
        });
        assert!(parse_events_api(&payload).is_none());
    }

    #[test]
    fn ignores_empty_text() {
        let payload = make_event_payload("U123", "C456", "", "1234567890.000003");
        assert!(parse_events_api(&payload).is_none());
    }

    #[test]
    fn threaded_message_sets_interruption_scope() {
        let payload = serde_json::json!({
            "event": {
                "type": "message",
                "user": "U123",
                "channel": "C456",
                "text": "reply in thread",
                "ts": "1234567891.000001",
                "thread_ts": "1234567890.000000"
            }
        });
        let msg = parse_events_api(&payload).unwrap();
        assert_eq!(
            msg.interruption_scope_id.as_deref(),
            Some("1234567890.000000")
        );
    }

    #[test]
    fn timestamp_parsed_from_ts() {
        let payload = make_event_payload("U123", "C456", "hi", "1700000000.123456");
        let msg = parse_events_api(&payload).unwrap();
        assert_eq!(msg.timestamp, 1700000000);
    }
}
