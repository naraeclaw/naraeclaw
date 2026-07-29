# Network Deployment

This document covers the current NaraeClaw network surfaces: the gateway API and Slack
Socket Mode. The independent Web UI and Desktop application are not supported product
surfaces.

Last verified: **July 29, 2026**.

## Current Inbound Surfaces

| Surface | Inbound port | Notes |
|---|---|---|
| CLI agent | none | local stdin/stdout |
| Slack Socket Mode | none | outbound WebSocket connection to Slack |
| Gateway API | `42617` by default | HTTP, webhooks, and WebSocket chat |

The public channel config currently supports CLI and Slack. Gateway routes for WhatsApp,
WATI, Nextcloud Talk, and Gmail exist only as `404 Not Found` compatibility stubs and must
not be configured as working integrations.

## Loopback Deployment

Keep the gateway on loopback unless another machine must connect:

```toml
[gateway]
host = "127.0.0.1"
port = 42617
require_pairing = true
allow_public_bind = false
```

```bash
naraeclaw daemon --host 127.0.0.1 --port 42617
```

Validate the local surface:

```bash
curl -fsS http://127.0.0.1:42617/health
naraeclaw self-test
naraeclaw knowledge status
```

## LAN or Public Deployment

The current implementation logs a warning for a non-loopback bind when
`allow_public_bind = false`, but it does not reject the bind. The requested `host` is the
actual exposure control. Do not rely on `allow_public_bind` as a security boundary.

For intentional LAN exposure:

```toml
[gateway]
host = "0.0.0.0"
port = 42617
allow_public_bind = true
require_pairing = true
```

```bash
naraeclaw gateway start --host 0.0.0.0 --port 42617
```

Use all of the following:

- host firewall rules that restrict source networks;
- pairing/bearer authentication;
- TLS or a trusted TLS-terminating reverse proxy;
- `trust_forwarded_headers = true` only behind a proxy that overwrites client-IP headers;
- a path prefix when the reverse proxy publishes NaraeClaw under a subpath.

For Internet-facing access, prefer a tunnel or reverse proxy that forwards to
`127.0.0.1:42617`. NaraeClaw supports Tailscale, ngrok, and Cloudflare tunnel providers in
its tunnel configuration; verify the active config schema before deployment.

## Slack Socket Mode

Slack does not require a public inbound gateway URL. Configure an App-Level Token and Bot
Token, then run the daemon:

```toml
[channels_config.slack]
enabled = true
app_token = "xapp-..."
bot_token = "xoxb-..."
```

```bash
naraeclaw daemon
naraeclaw channel doctor
```

## Deployment Checklist

- [ ] Keep `host = "127.0.0.1"` unless remote access is intentional.
- [ ] Verify `/health` locally before publishing a route.
- [ ] Keep pairing enabled or provide an equivalent authenticated proxy boundary.
- [ ] Use TLS for traffic that leaves the host.
- [ ] Restrict firewall and proxy source ranges.
- [ ] Run `naraeclaw knowledge status`; gateway health does not prove ByoriDB readiness.
- [ ] Confirm the current route in `crates/naraeclaw-gateway/src/lib.rs` before relying on
      an integration-specific webhook.

## References

- [Gateway API](../setup-guides/gateway-api.md)
- [Channels Reference](../reference/api/channels-reference.md)
- [Config Reference](../reference/api/config-reference.md#gateway)
- [ByoriDB Durable Knowledge](../setup-guides/byoridb-knowledge.md)
