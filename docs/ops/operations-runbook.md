# NaraeClaw Operations Runbook

This runbook is for operators who maintain availability, security posture, and incident response.

Last verified: **July 29, 2026**.

## Scope

Use this document for day-2 operations:

- starting and supervising runtime
- health checks and diagnostics
- safe rollout and rollback
- incident triage and recovery

For first-time installation, start from [one-click-bootstrap.md](../setup-guides/one-click-bootstrap.md).

## Runtime Modes

| Mode | Command | When to use |
|---|---|---|
| Foreground runtime | `naraeclaw daemon` | local debugging, short-lived sessions |
| Foreground gateway only | `naraeclaw gateway` | webhook endpoint testing |
| User service | `naraeclaw service install && naraeclaw service start` | persistent operator-managed runtime |
| Docker / Podman | `docker compose up -d` | containerized deployment |

## Docker / Podman Runtime

The stock container currently lacks the Byori MCP wrapper and Python runtime. Gateway and
agent operations can run, but durable knowledge is unavailable; mounting Byori data alone
does not fix the missing runtime. Use native deployment when ByoriDB knowledge is required,
and see [Container Limitation](../setup-guides/byoridb-knowledge.md#container-limitation).

If you installed via `./install.sh --docker`, the container exits after onboarding. To run
NaraeClaw as a long-lived container, use the repository `docker-compose.yml` or start a
container manually against the persisted data directory.

### Recommended: docker-compose

```bash
# Start (detached, auto-restarts on reboot)
docker compose up -d

# Stop
docker compose down

# Restart
docker compose up -d
```

Replace `docker` with `podman` if using Podman.

### Manual container lifecycle

```bash
# Start a new container from the bootstrap image
docker run -d --name naraeclaw \
  --restart unless-stopped \
  -v "$PWD/.naraeclaw-docker/.naraeclaw:/naraeclaw-data/.naraeclaw" \
  -v "$PWD/.naraeclaw-docker/workspace:/naraeclaw-data/workspace" \
  -e HOME=/naraeclaw-data \
  -e NARAECLAW_WORKSPACE=/naraeclaw-data/workspace \
  -p 42617:42617 \
  naraeclaw-bootstrap:local \
  gateway

# Stop (preserves config and workspace)
docker stop naraeclaw

# Restart a stopped container
docker start naraeclaw

# View logs
docker logs -f naraeclaw

# Health check
docker exec naraeclaw naraeclaw status
```

For Podman, add `--userns keep-id --user "$(id -u):$(id -g)"` and append `:Z` to volume mounts.

### Key detail: do not re-run install.sh to restart

Re-running `install.sh --docker` rebuilds the image and re-runs onboarding. To simply
restart, use `docker start`, `docker compose up -d`, or `podman start`.

For full setup instructions, see [one-click-bootstrap.md](../setup-guides/one-click-bootstrap.md#stopping-and-restarting-a-dockerpodman-container).

## Baseline Operator Checklist

1. Validate configuration:

```bash
naraeclaw status
```

1. Verify diagnostics:

```bash
naraeclaw doctor
naraeclaw knowledge status
naraeclaw channel doctor
```

1. Start runtime:

```bash
naraeclaw daemon
```

1. For persistent user session service:

```bash
naraeclaw service install
naraeclaw service start
naraeclaw service status
```

## Health and State Signals

| Signal | Command / File | Expected |
|---|---|---|
| Config validity | `naraeclaw doctor` | no critical errors |
| Channel connectivity | `naraeclaw channel doctor` | configured channels healthy |
| Runtime summary | `naraeclaw status` | expected provider/model/channels |
| Durable knowledge | `naraeclaw knowledge status` | managed wrapper connected, safe profile, expected space |
| Daemon heartbeat/state | `~/.naraeclaw/daemon_state.json` | file updates periodically |

## Logs and Diagnostics

### macOS / Windows (service wrapper logs)

- `~/.naraeclaw/logs/daemon.stdout.log`
- `~/.naraeclaw/logs/daemon.stderr.log`

### Linux (systemd user service)

```bash
journalctl --user -u naraeclaw.service -f
```

## Incident Triage Flow (Fast Path)

1. Snapshot system state:

```bash
naraeclaw status
naraeclaw doctor
naraeclaw knowledge status
naraeclaw channel doctor
```

1. Check service state:

```bash
naraeclaw service status
```

1. If service is unhealthy, restart cleanly:

```bash
naraeclaw service stop
naraeclaw service start
```

1. If channels still fail, verify allowlists and credentials in `~/.naraeclaw/config.toml`.

1. If gateway is involved, verify bind/auth settings (`[gateway]`) and local reachability.

## Safe Change Procedure

Before applying config changes:

1. back up `~/.naraeclaw/config.toml` and important ByoriDB data
2. apply one logical change at a time
3. run `naraeclaw doctor` and `naraeclaw knowledge status`
4. restart daemon/service
5. verify with `status` + `channel doctor`

## Rollback Procedure

If a rollout regresses behavior:

1. restore previous `config.toml`
2. restart runtime (`daemon` or `service`)
3. confirm recovery via `doctor` and channel health checks
4. document incident root cause and mitigation

## Related Docs

- [one-click-bootstrap.md](../setup-guides/one-click-bootstrap.md)
- [byoridb-knowledge.md](../setup-guides/byoridb-knowledge.md)
- [troubleshooting.md](./troubleshooting.md)
- [config-reference.md](../reference/api/config-reference.md)
- [commands-reference.md](../reference/cli/commands-reference.md)
