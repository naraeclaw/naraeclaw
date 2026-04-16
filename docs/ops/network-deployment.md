# Network Deployment — NaraeClaw on a Server or Local Network

This document covers deploying NaraeClaw on a Linux server or local network host, with Telegram and optional webhook channels.

---

## 1. Overview

| Mode | Inbound port needed? | Use case |
|------|----------------------|----------|
| **Telegram polling** | No | NaraeClaw polls Telegram API; works from anywhere |
| **Matrix sync (including E2EE)** | No | NaraeClaw syncs via Matrix client API; no inbound webhook required |
| **Discord/Slack** | No | Same — outbound only |
| **Nostr** | No | Connects to relays via WebSocket; outbound only |
| **Gateway webhook** | Yes | POST /webhook, /whatsapp, /linq, /nextcloud-talk need a public URL |
| **Gateway pairing** | Yes | If you pair clients via the gateway |
| **Alpine/OpenRC service** | No | System-wide background service on Alpine Linux |

**Key:** Telegram, Discord, Slack, and Nostr use **outbound connections** — NaraeClaw connects to external servers/relays. No port forwarding or public IP required.

---

## 2. NaraeClaw on a Server

### 2.1 Prerequisites

- Linux server or local machine
- Network access to your chosen messaging provider or webhook tunnel

### 2.2 Install

```bash
cargo build --release
# Or install via your preferred method
```

### 2.3 Config

Edit `~/.naraeclaw/config.toml`:

```toml
[channels_config.telegram]
bot_token = "YOUR_BOT_TOKEN"
allowed_users = []

[gateway]
host = "127.0.0.1"
port = 42617
allow_public_bind = false
```

### 2.4 Run Daemon (Local Only)

```bash
naraeclaw daemon --host 127.0.0.1 --port 42617
```

- Gateway binds to `127.0.0.1` — not reachable from other machines
- Telegram channel works: NaraeClaw polls Telegram API (outbound)
- No firewall or port forwarding needed

---

## 3. Binding to 0.0.0.0 (Local Network)

To allow other devices on your LAN to hit the gateway (e.g. for pairing or webhooks):

### 3.1 Option A: Explicit Opt-In

```toml
[gateway]
host = "0.0.0.0"
port = 42617
allow_public_bind = true
```

```bash
naraeclaw daemon --host 0.0.0.0 --port 42617
```

**Security:** `allow_public_bind = true` exposes the gateway to your local network. Only use on trusted LANs.

### 3.2 Option B: Tunnel (Recommended for Webhooks)

If you need a **public URL** (e.g. WhatsApp webhook, external clients):

1. Run gateway on localhost:
   ```bash
   naraeclaw daemon --host 127.0.0.1 --port 42617
   ```

2. Start a tunnel:
   ```toml
   [tunnel]
   provider = "tailscale"   # or "ngrok", "cloudflare"
   ```
   Or use `naraeclaw tunnel` (see tunnel docs).

3. NaraeClaw will refuse `0.0.0.0` unless `allow_public_bind = true` or a tunnel is active.

---

## 4. Telegram Polling (No Inbound Port)

Telegram uses **long-polling** by default:

- NaraeClaw calls `https://api.telegram.org/bot{token}/getUpdates`
- No inbound port or public IP needed
- Works behind NAT, on RPi, in a home lab

**Config:**

```toml
[channels_config.telegram]
bot_token = "YOUR_BOT_TOKEN"
allowed_users = []            # deny-by-default, bind identities explicitly
```

Run `naraeclaw daemon` — Telegram channel starts automatically.

To approve one Telegram account at runtime:

```bash
naraeclaw channel bind-telegram <IDENTITY>
```

`<IDENTITY>` can be a numeric Telegram user ID or a username (without `@`).

### 4.1 Single Poller Rule (Important)

Telegram Bot API `getUpdates` supports only one active poller per bot token.

- Keep one runtime instance for the same token (recommended: `naraeclaw daemon` service).
- Do not run `cargo run -- channel start` or another bot process at the same time.

If you hit this error:

`Conflict: terminated by other getUpdates request`

you have a polling conflict. Stop extra instances and restart only one daemon.

---

## 5. Webhook Channels (WhatsApp, Nextcloud Talk, Custom)

Webhook-based channels need a **public URL** so Meta (WhatsApp) or your client can POST events.

### 5.1 Tailscale Funnel

```toml
[tunnel]
provider = "tailscale"
```

Tailscale Funnel exposes your gateway via a `*.ts.net` URL. No port forwarding.

### 5.2 ngrok

```toml
[tunnel]
provider = "ngrok"
```

Or run ngrok manually:
```bash
ngrok http 42617
# Use the HTTPS URL for your webhook
```

### 5.3 Cloudflare Tunnel

Configure Cloudflare Tunnel to forward to `127.0.0.1:42617`, then set your webhook URL to the tunnel's public hostname.

---

## 6. Checklist: Server Deployment

- [ ] Build or install the `naraeclaw` binary
- [ ] Configure `[channels_config.telegram]` or your selected channel
- [ ] Run `naraeclaw daemon --host 127.0.0.1 --port 42617` (Telegram works without 0.0.0.0)
- [ ] For LAN access: `--host 0.0.0.0` + `allow_public_bind = true` in config
- [ ] For webhooks: use Tailscale, ngrok, or Cloudflare tunnel

---

## 7. OpenRC (Alpine Linux Service)

NaraeClaw supports OpenRC for Alpine Linux and other distributions using the OpenRC init system. OpenRC services run **system-wide** and require root/sudo.

### 7.1 Prerequisites

- Alpine Linux (or another OpenRC-based distro)
- Root or sudo access
- A dedicated `naraeclaw` system user (created during install)

### 7.2 Install Service

```bash
# Install service (OpenRC is auto-detected on Alpine)
sudo naraeclaw service install
```

This creates:
- Init script: `/etc/init.d/naraeclaw`
- Config directory: `/etc/naraeclaw/`
- Log directory: `/var/log/naraeclaw/`

### 7.3 Configuration

Manual config copy is usually not required.

`sudo naraeclaw service install` automatically prepares `/etc/naraeclaw`, migrates existing runtime state from your user setup when available, and sets ownership/permissions for the `naraeclaw` service user.

If no prior runtime state is available to migrate, create `/etc/naraeclaw/config.toml` before starting the service.

### 7.4 Enable and Start

```bash
# Add to default runlevel
sudo rc-update add naraeclaw default

# Start the service
sudo rc-service naraeclaw start

# Check status
sudo rc-service naraeclaw status
```

### 7.5 Manage Service

| Command | Description |
|---------|-------------|
| `sudo rc-service naraeclaw start` | Start the daemon |
| `sudo rc-service naraeclaw stop` | Stop the daemon |
| `sudo rc-service naraeclaw status` | Check service status |
| `sudo rc-service naraeclaw restart` | Restart the daemon |
| `sudo naraeclaw service status` | NaraeClaw status wrapper (uses `/etc/naraeclaw` config) |

### 7.6 Logs

OpenRC routes logs to:

| Log | Path |
|-----|------|
| Access/stdout | `/var/log/naraeclaw/access.log` |
| Errors/stderr | `/var/log/naraeclaw/error.log` |

View logs:

```bash
sudo tail -f /var/log/naraeclaw/error.log
```

### 7.7 Uninstall

```bash
# Stop and remove from runlevel
sudo rc-service naraeclaw stop
sudo rc-update del naraeclaw default

# Remove init script
sudo naraeclaw service uninstall
```

### 7.8 Notes

- OpenRC is **system-wide only** (no user-level services)
- Requires `sudo` or root for all service operations
- The service runs as the `naraeclaw:naraeclaw` user (least privilege)
- Config must be at `/etc/naraeclaw/config.toml` (explicit path in init script)
- If the `naraeclaw` user does not exist, install will fail with instructions to create it

### 7.9 Checklist: Alpine/OpenRC Deployment

- [ ] Install: `sudo naraeclaw service install`
- [ ] Enable: `sudo rc-update add naraeclaw default`
- [ ] Start: `sudo rc-service naraeclaw start`
- [ ] Verify: `sudo rc-service naraeclaw status`
- [ ] Check logs: `/var/log/naraeclaw/error.log`

---

## 8. References

- [channels-reference.md](../reference/api/channels-reference.md) — Channel configuration overview
- [matrix-e2ee-guide.md](../security/matrix-e2ee-guide.md) — Matrix setup and encrypted-room troubleshooting
