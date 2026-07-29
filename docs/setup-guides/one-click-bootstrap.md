# One-Click Bootstrap

This page defines the fastest supported path to install and initialize NaraeClaw.

Last verified: **July 29, 2026**.

## Option 0: Homebrew (macOS/Linuxbrew)

```bash
brew install naraeclaw
```

## Option A (Recommended): Clone + local script

```bash
git clone https://github.com/naraeclaw/naraeclaw.git
cd naraeclaw
./install.sh
```

What it does by default:

1. `cargo build --release --locked`
2. `cargo install --path . --force --locked`

The NaraeClaw installer does not install ByoriDB. Complete the default durable-knowledge
setup separately:

```bash
curl -fsSL https://github.com/byoridb/byori/releases/latest/download/install.sh | bash
naraeclaw knowledge status
```

See [ByoriDB Durable Knowledge](byoridb-knowledge.md) before migrating an existing
workspace.

### Resource preflight and pre-built flow

Source builds typically require at least:

- **2 GB RAM + swap**
- **6 GB free disk**

When resources are constrained, bootstrap now attempts a pre-built binary first.

```bash
./install.sh --prefer-prebuilt
```

To require binary-only installation and fail if no compatible release asset exists:

```bash
./install.sh --prebuilt-only
```

To bypass pre-built flow and force source compilation:

```bash
./install.sh --force-source-build
```

## Dual-mode bootstrap

Default behavior is **app-only** (build/install NaraeClaw) and expects existing Rust toolchain.

For fresh machines, enable environment bootstrap explicitly:

```bash
./install.sh --install-system-deps --install-rust
```

Notes:

- `--install-system-deps` installs compiler/build prerequisites (may require `sudo`).
- `--install-rust` installs Rust via `rustup` when missing.
- `--prefer-prebuilt` tries release binary download first, then falls back to source build.
- `--prebuilt-only` disables source fallback.
- `--force-source-build` disables pre-built flow entirely.

## Option B: Remote one-liner

```bash
curl -fsSL https://raw.githubusercontent.com/naraeclaw/naraeclaw/master/install.sh | bash
```

For high-security environments, prefer Option A so you can review the script before execution.

If you run Option B outside a repository checkout, the install script automatically clones a temporary workspace, builds, installs, and then cleans it up.

## Optional onboarding modes

### Containerized onboarding (Docker)

> **Current limitation:** the stock container image does not bundle Python or the Byori
> MCP wrapper. Containerized onboarding can configure NaraeClaw, but durable knowledge is
> unavailable until a Byori-capable image or supported sidecar contract exists. There is no
> automatic fallback to legacy memory. See
> [Container Limitation](byoridb-knowledge.md#container-limitation).

```bash
./install.sh --docker
```

This builds a local NaraeClaw image and launches onboarding inside a container while
persisting config/workspace to `./.naraeclaw-docker`.

Container CLI defaults to `docker`. If Docker CLI is unavailable and `podman` exists,
the installer auto-falls back to `podman`. You can also set `NARAECLAW_CONTAINER_CLI`
explicitly (for example: `NARAECLAW_CONTAINER_CLI=podman ./install.sh --docker`).

For Podman, the installer runs with `--userns keep-id` and `:Z` volume labels so
workspace/config mounts remain writable inside the container.

If you add `--skip-build`, the installer skips local image build. It first tries the local
Docker tag (`NARAECLAW_DOCKER_IMAGE`, default: `naraeclaw-bootstrap:local`); if missing,
it pulls `ghcr.io/naraeclaw/naraeclaw:latest` and tags it locally before running.

### Stopping and restarting a Docker/Podman container

After `./install.sh --docker` finishes, the container exits. Your config and workspace
are persisted in the data directory (default: `./.naraeclaw-docker`, or `~/.naraeclaw-docker`
when bootstrapping via `curl | bash`). You can override this path with `NARAECLAW_DOCKER_DATA_DIR`.

**Do not re-run `install.sh`** to restart -- it will rebuild the image and re-run onboarding.
Instead, start a new container from the existing image and mount the persisted data directory.

#### Using the repository docker-compose.yml

The simplest way to run NaraeClaw long-term in Docker/Podman is with the provided
`docker-compose.yml` at the repository root. It uses a named volume (`naraeclaw-data`)
and sets `restart: unless-stopped` so the container survives reboots.

```bash
# Start (detached)
docker compose up -d

# Stop
docker compose down

# Restart after stopping
docker compose up -d
```

Replace `docker` with `podman` if you use Podman.

#### Manual container run (using install.sh data directory)

If you installed via `./install.sh --docker` and want to reuse the `.naraeclaw-docker`
data directory without compose:

```bash
# Docker
docker run -d --name naraeclaw \
  --restart unless-stopped \
  -v "$PWD/.naraeclaw-docker/.naraeclaw:/naraeclaw-data/.naraeclaw" \
  -v "$PWD/.naraeclaw-docker/workspace:/naraeclaw-data/workspace" \
  -e HOME=/naraeclaw-data \
  -e NARAECLAW_WORKSPACE=/naraeclaw-data/workspace \
  -p 42617:42617 \
  naraeclaw-bootstrap:local \
  gateway

# Podman (add --userns keep-id and :Z volume labels)
podman run -d --name naraeclaw \
  --restart unless-stopped \
  --userns keep-id \
  --user "$(id -u):$(id -g)" \
  -v "$PWD/.naraeclaw-docker/.naraeclaw:/naraeclaw-data/.naraeclaw:Z" \
  -v "$PWD/.naraeclaw-docker/workspace:/naraeclaw-data/workspace:Z" \
  -e HOME=/naraeclaw-data \
  -e NARAECLAW_WORKSPACE=/naraeclaw-data/workspace \
  -p 42617:42617 \
  naraeclaw-bootstrap:local \
  gateway
```

#### Common lifecycle commands

```bash
# Stop the container (preserves data)
docker stop naraeclaw

# Start a stopped container (config and workspace are intact)
docker start naraeclaw

# View logs
docker logs -f naraeclaw

# Remove the container (data in volumes/.naraeclaw-docker is preserved)
docker rm naraeclaw

# Check health
docker exec naraeclaw naraeclaw status
```

#### Environment variables

When running manually, pass provider configuration as environment variables
or ensure they are already saved in the persisted `config.toml`:

```bash
docker run -d --name naraeclaw \
  -e API_KEY="sk-..." \
  -e PROVIDER="openrouter" \
  -v "$PWD/.naraeclaw-docker/.naraeclaw:/naraeclaw-data/.naraeclaw" \
  -v "$PWD/.naraeclaw-docker/workspace:/naraeclaw-data/workspace" \
  -p 42617:42617 \
  naraeclaw-bootstrap:local \
  gateway
```

If you already ran `onboard` during the initial install, your API key and provider are
saved in `.naraeclaw-docker/.naraeclaw/config.toml` and do not need to be passed again.

### Quick onboarding (non-interactive)

```bash
./install.sh --api-key "sk-..." --provider openrouter
```

Or with environment variables:

```bash
NARAECLAW_API_KEY="sk-..." NARAECLAW_PROVIDER="openrouter" ./install.sh
```

## Useful flags

- `--install-system-deps`
- `--install-rust`
- `--skip-build` (in `--docker` mode: use local image if present, otherwise pull `ghcr.io/naraeclaw/naraeclaw:latest`)
- `--skip-install`
- `--provider <id>`

See all options:

```bash
./install.sh --help
```

## Related docs

- [README.md](../README.md)
- [byoridb-knowledge.md](byoridb-knowledge.md)
- [commands-reference.md](../reference/cli/commands-reference.md)
- [providers-reference.md](../reference/api/providers-reference.md)
- [channels-reference.md](../reference/api/channels-reference.md)
