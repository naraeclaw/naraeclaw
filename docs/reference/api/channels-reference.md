# Channels Reference

This is the current public channel configuration contract.

Last verified: **July 29, 2026**.

## Supported Surfaces

| Surface | Receive mode | Public inbound port |
|---|---|---|
| CLI | local stdin/stdout | no |
| Slack | Socket Mode WebSocket | no |
| Gateway | HTTP/WebSocket API, separate from `channels_config` | yes, when exposed |

The current `ChannelsConfig` schema contains only CLI and Slack. Older documentation and
configuration examples for Telegram, Discord, Matrix, Mattermost, Signal, WhatsApp,
Nextcloud Talk, email, Nostr, and regional channels are not part of the current public
runtime contract.

The gateway still reserves several integration-specific paths as compatibility stubs.
`/whatsapp`, `/wati`, `/nextcloud-talk`, and `/webhook/gmail` return `404 Not Found`; their
presence in the router does not mean the integration is available.

## `[channels_config]`

```toml
[channels_config]
cli = true
message_timeout_secs = 300
ack_reactions = true
show_tool_calls = false
session_persistence = true
session_backend = "sqlite"
session_ttl_hours = 0
debounce_ms = 0
```

| Key | Default | Purpose |
|---|---|---|
| `cli` | `true` | enable the interactive CLI channel |
| `message_timeout_secs` | `300` | base per-message processing budget |
| `ack_reactions` | `true` | add receipt/completion reactions when supported |
| `show_tool_calls` | `false` | forward tool-call notices to channel users |
| `session_persistence` | `true` | persist channel conversation sessions |
| `session_backend` | `sqlite` | session store; `jsonl` remains a legacy option |
| `session_ttl_hours` | `0` | archive sessions older than N hours; `0` disables expiry |
| `debounce_ms` | `0` | combine rapid messages from one sender; `0` disables |

Channel session SQLite is operational conversation state. It is not a durable-knowledge
backend; ByoriDB owns cross-session knowledge.

## Slack Socket Mode

Create a Slack app, enable Socket Mode, and provide an App-Level Token (`xapp-`) plus a Bot
Token (`xoxb-`). A public webhook URL is not required.

```toml
[channels_config.slack]
enabled = true
app_token = "xapp-..."
bot_token = "xoxb-..."
# signing_secret = "..."
# default_channel = "C0123456789"
```

| Key | Default | Purpose |
|---|---|---|
| `enabled` | `false` | activate Slack |
| `app_token` | required | Socket Mode connection token |
| `bot_token` | required | Web API send token |
| `signing_secret` | unset | optional Slack signing secret |
| `default_channel` | unset | fallback channel when no incoming thread context exists |

Recommended Slack scopes include `chat:write`, `reactions:write`, `channels:history`, and
`im:history`; the App-Level Token needs `connections:write`.

## CLI Operations

```bash
naraeclaw channel list
naraeclaw channel doctor
naraeclaw channel start
naraeclaw channel send "message" --channel-id slack --recipient C0123456789
```

`channel add` and `channel remove` currently direct users back to managed setup or manual
configuration rather than acting as a complete declarative config editor.

## Troubleshooting

| Symptom | Check |
|---|---|
| Slack does not connect | token prefixes, Socket Mode, app installation, `enabled = true` |
| Messages arrive but no reply | bot scopes, channel membership, daemon logs |
| Tool activity is invisible | set `show_tool_calls = true` if disclosure is desired |
| Sessions disappear | `session_persistence`, writable workspace, session TTL |
| Old channel config has no effect | remove it; only CLI and Slack are in the current schema |

Use `naraeclaw channel doctor` after every channel config change. The config schema emitted
by `naraeclaw config schema` and `crates/naraeclaw-config/src/schema/channels.rs` are the
sources of truth.

## Related Docs

- [Gateway API](../../setup-guides/gateway-api.md)
- [Network Deployment](../../ops/network-deployment.md)
- [Config Reference](config-reference.md)
