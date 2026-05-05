#!/usr/bin/env sh
# NaraeClaw installer
# Supported platforms: macOS (x86_64, arm64), Linux (x86_64, aarch64)
# POSIX preamble: ensure bash is available, then re-exec under bash.
set -eu

_have_cmd() { command -v "$1" >/dev/null 2>&1; }

_run_privileged() {
  if [ "$(id -u)" -eq 0 ]; then "$@"
  elif _have_cmd sudo; then sudo "$@"
  else echo "error: sudo is required to install missing dependencies." >&2; exit 1; fi
}

_is_container_runtime() {
  [ -f /.dockerenv ] || [ -f /run/.containerenv ] && return 0
  [ -r /proc/1/cgroup ] && grep -Eq '(docker|containerd|kubepods|podman|lxc)' /proc/1/cgroup && return 0
  return 1
}

_ensure_bash() {
  _have_cmd bash && return 0
  echo "==> bash not found; attempting to install it"
  if _have_cmd apk; then _run_privileged apk add --no-cache bash
  elif _have_cmd apt-get; then _run_privileged apt-get update -qq && _run_privileged apt-get install -y bash
  elif _have_cmd dnf; then _run_privileged dnf install -y bash
  elif _have_cmd pacman; then
    if _is_container_runtime; then
      _PACMAN_CFG="$(mktemp /tmp/naraeclaw-pacman.XXXXXX.conf)"
      cp /etc/pacman.conf "$_PACMAN_CFG"
      grep -Eq '^[[:space:]]*DisableSandboxSyscalls([[:space:]]|$)' "$_PACMAN_CFG" || printf '\nDisableSandboxSyscalls\n' >> "$_PACMAN_CFG"
      _run_privileged pacman --config "$_PACMAN_CFG" -Sy --noconfirm
      _run_privileged pacman --config "$_PACMAN_CFG" -S --noconfirm --needed bash
      rm -f "$_PACMAN_CFG"
    else
      _run_privileged pacman -Sy --noconfirm
      _run_privileged pacman -S --noconfirm --needed bash
    fi
  else echo "error: unsupported package manager; install bash manually and retry." >&2; exit 1; fi
}

# If not already running under bash, ensure bash exists and re-exec.
if [ -z "${BASH_VERSION:-}" ]; then
  _ensure_bash
  exec bash "$0" "$@"
fi

# --- From here on, we are running under bash ---
set -euo pipefail

# --- Color and styling ---
if [[ -t 1 ]]; then
  BLUE='\033[0;34m'
  BOLD_BLUE='\033[1;34m'
  GREEN='\033[0;32m'
  YELLOW='\033[0;33m'
  RED='\033[0;31m'
  BOLD='\033[1m'
  DIM='\033[2m'
  RESET='\033[0m'
else
  BLUE='' BOLD_BLUE='' GREEN='' YELLOW='' RED='' BOLD='' DIM='' RESET=''
fi

CRAB="🦀"

info() {
  echo -e "${BLUE}${CRAB}${RESET} ${BOLD}$*${RESET}"
}

step_ok() {
  echo -e "  ${GREEN}✓${RESET} $*"
}

step_dot() {
  echo -e "  ${DIM}·${RESET} $*"
}

step_fail() {
  echo -e "  ${RED}✗${RESET} $*"
}

warn() {
  echo -e "${YELLOW}!${RESET} $*" >&2
}

error() {
  echo -e "${RED}✗${RESET} ${RED}$*${RESET}" >&2
}

usage() {
  cat <<'USAGE'
NaraeClaw installer — one-click bootstrap

Usage:
  ./install.sh [options]

The installer builds NaraeClaw, configures your provider and API key,
and starts the gateway service — all in one step.

Supported platforms: macOS (x86_64, arm64), Linux (x86_64, aarch64)

Options:
  --docker                   Run install in Docker-compatible mode
  --install-system-deps      Install build dependencies (Linux/macOS)
  --install-rust             Install Rust via rustup if missing
  --prefer-prebuilt          Try latest release binary first; fallback to source build on miss
  --prebuilt-only            Install only from latest release binary (no source build fallback)
  --force-source-build       Disable prebuilt flow and always build from source
  --api-key <key>            API key (skips interactive prompt)
  --provider <id>            Provider (default: openrouter)
  --model <id>               Model (optional)
  --cargo-features <list>    Extra cargo features (comma/space separated)
  --skip-onboard             Skip provider/API key configuration
  --skip-build               Skip build step
  --skip-install             Skip cargo install step
  --build-first              Alias for explicitly enabling separate `cargo build --release --locked`
  -h, --help                 Show help

Examples:
  # One-click install (interactive)
  curl -fsSL https://naraeclaw.ai/install.sh | bash

  # Non-interactive with API key
  ./install.sh --api-key "sk-..." --provider openrouter

  # Prebuilt binary (fastest)
  ./install.sh --prefer-prebuilt --api-key "sk-..."

  # Docker deploy
  ./install.sh --docker

  # Build only, configure later
  ./install.sh --skip-onboard

Environment:
  NARAECLAW_CONTAINER_CLI     Container CLI command (default: docker; auto-fallback: podman)
  NARAECLAW_DOCKER_DATA_DIR   Host path for Docker config/workspace persistence
  NARAECLAW_DOCKER_IMAGE      Docker image tag to build/run (default: naraeclaw-bootstrap:local)
  NARAECLAW_API_KEY           Used when --api-key is not provided
  NARAECLAW_PROVIDER          Used when --provider is not provided (default: openrouter)
  NARAECLAW_MODEL             Used when --model is not provided
  NARAECLAW_CARGO_FEATURES    Extra cargo features for source builds (comma/space separated)
  NARAECLAW_BOOTSTRAP_MIN_RAM_MB   Minimum RAM threshold for source build preflight (default: 2048)
  NARAECLAW_BOOTSTRAP_MIN_DISK_MB  Minimum free disk threshold for source build preflight (default: 6144)
USAGE
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

append_cargo_feature() {
  local feature="${1:-}"
  [[ -n "$feature" ]] || return 0
  case ",${CARGO_FEATURES_CSV:-}," in
    *,"$feature",*) return 0 ;;
  esac
  if [[ -n "${CARGO_FEATURES_CSV:-}" ]]; then
    CARGO_FEATURES_CSV+=",${feature}"
  else
    CARGO_FEATURES_CSV="$feature"
  fi
}

append_cargo_features_from_input() {
  local raw="${1:-}" token
  raw="${raw//,/ }"
  for token in $raw; do
    append_cargo_feature "$token"
  done
}

refresh_cargo_feature_args() {
  CARGO_FEATURE_ARGS=()
  if [[ "${CARGO_NO_DEFAULT_FEATURES:-false}" == true ]]; then
    CARGO_FEATURE_ARGS+=(--no-default-features)
  fi
  if [[ -n "${CARGO_FEATURES_CSV:-}" ]]; then
    CARGO_FEATURE_ARGS+=(--features "$CARGO_FEATURES_CSV")
  fi
}

get_total_memory_mb() {
  case "$(uname -s)" in
    Linux)
      if [[ -r /proc/meminfo ]]; then
        awk '/MemTotal:/ {printf "%d\n", $2 / 1024}' /proc/meminfo
      fi
      ;;
    Darwin)
      if have_cmd sysctl; then
        local bytes
        bytes="$(sysctl -n hw.memsize 2>/dev/null || true)"
        if [[ "$bytes" =~ ^[0-9]+$ ]]; then
          echo $((bytes / 1024 / 1024))
        fi
      fi
      ;;
  esac
}

get_available_disk_mb() {
  local path="${1:-.}"
  local free_kb
  free_kb="$(df -Pk "$path" 2>/dev/null | awk 'NR==2 {print $4}')"
  if [[ "$free_kb" =~ ^[0-9]+$ ]]; then
    echo $((free_kb / 1024))
  fi
}

detect_release_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"


  case "$os:$arch" in
    Linux:x86_64)
      echo "x86_64-unknown-linux-gnu"
      ;;
    Linux:aarch64|Linux:arm64)
      echo "aarch64-unknown-linux-gnu"
      ;;
    Darwin:x86_64)
      echo "x86_64-apple-darwin"
      ;;
    Darwin:arm64|Darwin:aarch64)
      echo "aarch64-apple-darwin"
      ;;
    *)
      return 1
      ;;
  esac
}

detect_device_class() {
  # Containers are never desktops
  if _is_container_runtime; then
    echo "container"
    return
  fi


  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin)
      # macOS is always a desktop
      echo "desktop"
      ;;
    Linux)
      # Check for a display server (X11 or Wayland)
      if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" || -n "${XDG_SESSION_TYPE:-}" ]]; then
        echo "desktop"
      else
        echo "server"
      fi
      ;;
    *)
      echo "server"
      ;;
  esac
}

should_attempt_prebuilt_for_resources() {
  local workspace="${1:-.}"
  local min_ram_mb min_disk_mb total_ram_mb free_disk_mb low_resource

  min_ram_mb="${NARAECLAW_BOOTSTRAP_MIN_RAM_MB:-2048}"
  min_disk_mb="${NARAECLAW_BOOTSTRAP_MIN_DISK_MB:-6144}"
  total_ram_mb="$(get_total_memory_mb || true)"
  free_disk_mb="$(get_available_disk_mb "$workspace" || true)"
  low_resource=false

  if [[ "$total_ram_mb" =~ ^[0-9]+$ && "$total_ram_mb" -lt "$min_ram_mb" ]]; then
    low_resource=true
  fi
  if [[ "$free_disk_mb" =~ ^[0-9]+$ && "$free_disk_mb" -lt "$min_disk_mb" ]]; then
    low_resource=true
  fi

  if [[ "$low_resource" == true ]]; then
    warn "Source build preflight indicates constrained resources."
    if [[ "$total_ram_mb" =~ ^[0-9]+$ ]]; then
      warn "Detected RAM: ${total_ram_mb}MB (recommended >= ${min_ram_mb}MB for local source builds)."
    else
      warn "Unable to detect total RAM automatically."
    fi
    if [[ "$free_disk_mb" =~ ^[0-9]+$ ]]; then
      warn "Detected free disk: ${free_disk_mb}MB (recommended >= ${min_disk_mb}MB)."
    else
      warn "Unable to detect free disk space automatically."
    fi
    return 0
  fi

  return 1
}

resolve_asset_url() {
  local asset_name="$1"
  local api_url="https://api.github.com/repos/naraeclaw/naraeclaw/releases"
  local releases_json download_url

  # Fetch up to 10 recent releases (includes prereleases) and find the first
  # one that contains the requested asset.
  releases_json="$(curl -fsSL "${api_url}?per_page=10" 2>/dev/null || true)"
  if [[ -z "$releases_json" ]]; then
    return 1
  fi

  # Parse with simple grep/sed — avoids jq dependency.
  download_url="$(printf '%s\n' "$releases_json" \
    | tr ',' '\n' \
    | grep '"browser_download_url"' \
    | sed 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/' \
    | grep "/${asset_name}\$" \
    | head -n 1)"

  if [[ -z "$download_url" ]]; then
    return 1
  fi

  echo "$download_url"
}

install_prebuilt_binary() {
  local target archive_url temp_dir archive_path extracted_bin install_dir asset_name

  if ! have_cmd curl; then
    warn "curl is required for pre-built binary installation."
    return 1
  fi
  if ! have_cmd tar; then
    warn "tar is required for pre-built binary installation."
    return 1
  fi


  target="$(detect_release_target || true)"
  if [[ -z "$target" ]]; then
    warn "No pre-built binary target mapping for $(uname -s)/$(uname -m)."
    return 1
  fi

  asset_name="naraeclaw-.tar.gz"

  # Try the GitHub API first to find the newest release (including prereleases)
  # that actually contains the asset, then fall back to /releases/latest/.
  archive_url="$(resolve_asset_url "$asset_name" || true)"
  if [[ -z "$archive_url" ]]; then
    archive_url="https://github.com/naraeclaw/naraeclaw/releases/latest/download/${asset_name}"
  fi

  temp_dir="$(mktemp -d -t naraeclaw-prebuilt-XXXXXX)"
  archive_path="$temp_dir/${asset_name}"

  step_dot "Attempting pre-built binary install for target: $target"
  if ! curl -fsSL "$archive_url" -o "$archive_path"; then
    warn "Could not download release asset: $archive_url"
    rm -rf "$temp_dir"
    return 1
  fi

  if ! tar -xzf "$archive_path" -C "$temp_dir"; then
    warn "Failed to extract pre-built archive."
    rm -rf "$temp_dir"
    return 1
  fi

  extracted_bin="$temp_dir/naraeclaw"
  if [[ ! -x "$extracted_bin" ]]; then
    extracted_bin="$(find "$temp_dir" -maxdepth 2 -type f -name naraeclaw -perm -u+x | head -n 1 || true)"
  fi
  if [[ -z "$extracted_bin" || ! -x "$extracted_bin" ]]; then
    warn "Archive did not contain an executable naraeclaw binary."
    rm -rf "$temp_dir"
    return 1
  fi

  install_dir="$HOME/.cargo/bin"
  mkdir -p "$install_dir"
  install -m 0755 "$extracted_bin" "$install_dir/naraeclaw"
  rm -rf "$temp_dir"

  step_ok "Installed pre-built binary to $install_dir/naraeclaw"
  if [[ ":$PATH:" != *":$install_dir:"* ]]; then
    warn "$install_dir is not in PATH for this shell."
    warn "Run: export PATH=\"$install_dir:\$PATH\""
  fi

  return 0
}

run_privileged() {
  if [[ "$(id -u)" -eq 0 ]]; then
    "$@"
  elif have_cmd sudo; then
    sudo "$@"
  else
    error "sudo is required to install system dependencies."
    return 1
  fi
}

is_container_runtime() {
  if [[ -f /.dockerenv || -f /run/.containerenv ]]; then
    return 0
  fi

  if [[ -r /proc/1/cgroup ]] && grep -Eq '(docker|containerd|kubepods|podman|lxc)' /proc/1/cgroup; then
    return 0
  fi

  return 1
}

run_pacman() {
  if ! have_cmd pacman; then
    error "pacman is not available."
    return 1
  fi

  if ! is_container_runtime; then
    run_privileged pacman "$@"
    return $?
  fi

  local pacman_cfg_tmp=""
  local pacman_rc=0
  pacman_cfg_tmp="$(mktemp /tmp/naraeclaw-pacman.XXXXXX.conf)"
  cp /etc/pacman.conf "$pacman_cfg_tmp"
  if ! grep -Eq '^[[:space:]]*DisableSandboxSyscalls([[:space:]]|$)' "$pacman_cfg_tmp"; then
    printf '\nDisableSandboxSyscalls\n' >> "$pacman_cfg_tmp"
  fi

  if run_privileged pacman --config "$pacman_cfg_tmp" "$@"; then
    pacman_rc=0
  else
    pacman_rc=$?
  fi

  rm -f "$pacman_cfg_tmp"
  return "$pacman_rc"
}


install_system_deps() {
  step_dot "Installing system dependencies"

  case "$(uname -s)" in
    Linux)
      if have_cmd apt-get; then
        run_privileged apt-get update -qq
        run_privileged apt-get install -y build-essential pkg-config git curl libssl-dev
      elif have_cmd dnf; then
        run_privileged dnf install -y \
          gcc \
          gcc-c++ \
          make \
          pkgconf-pkg-config \
          git \
          curl \
          openssl-devel \
          perl
      elif have_cmd pacman; then
        run_pacman -Sy --noconfirm
        run_pacman -S --noconfirm --needed \
          gcc \
          make \
          pkgconf \
          git \
          curl \
          openssl \
          perl \
          ca-certificates
      elif have_cmd pkg && [[ -n "${TERMUX_VERSION:-}" ]]; then
        pkg install -y build-essential pkg-config git curl openssl perl
      else
        warn "Unsupported Linux distribution. Install compiler toolchain + pkg-config + git + curl + OpenSSL headers + perl manually."
      fi
      ;;
    Darwin)
      if ! xcode-select -p >/dev/null 2>&1; then
        step_dot "Installing Xcode Command Line Tools"
        xcode-select --install || true
        cat <<'MSG'
Please complete the Xcode Command Line Tools installation dialog,
then re-run bootstrap.
MSG
        exit 0
      fi
      # Detect un-accepted Xcode/CLT license (causes `cc` to exit 69).
      # xcrun --show-sdk-path can succeed even without an accepted license,
      # so we test-compile a trivial C file which reliably triggers the error.
      _xcode_test_file="$(mktemp /tmp/naraeclaw-xcode-check.XXXXXX.c)"
      printf 'int main(){return 0;}\n' > "$_xcode_test_file"
      if ! cc -x c "$_xcode_test_file" -o /dev/null 2>/dev/null; then
        rm -f "$_xcode_test_file"
        warn "Xcode/CLT license has not been accepted. Attempting to accept it now..."
        _xcode_accept_ok=false
        if [[ "$(id -u)" -eq 0 ]]; then
          xcodebuild -license accept && _xcode_accept_ok=true
        elif [[ -c /dev/tty ]] && have_cmd sudo; then
          sudo xcodebuild -license accept < /dev/tty && _xcode_accept_ok=true
        fi
        if [[ "$_xcode_accept_ok" == true ]]; then
          step_ok "Xcode license accepted"
        else
          error "Could not accept Xcode license. Run manually:"
          error "  sudo xcodebuild -license accept"
          error "then re-run this installer."
          exit 1
        fi
      else
        rm -f "$_xcode_test_file"
      fi
      if ! have_cmd git; then
        warn "git is not available. Install git (e.g., Homebrew) and re-run bootstrap."
      fi
      ;;
    *)
      warn "Unsupported OS for automatic dependency install. Continuing without changes."
      ;;
  esac
}

install_rust_toolchain() {
  if have_cmd cargo && have_cmd rustc; then
    step_ok "Rust already installed: $(rustc --version)"
    return
  fi

  if ! have_cmd curl; then
    error "curl is required to install Rust via rustup."
    exit 1
  fi

  step_dot "Installing Rust via rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
  fi

  if ! have_cmd cargo; then
    error "Rust installation completed but cargo is still unavailable in PATH."
    error "Run: source \"$HOME/.cargo/env\""
    exit 1
  fi
}


ensure_default_config_and_workspace() {
  # Creates a minimal config.toml and workspace scaffold files when the
  # onboard wizard was skipped (e.g. --skip-build --prefer-prebuilt, or
  # Docker mode without an API key).
  #
  # $1 — config directory  (e.g. ~/.naraeclaw or $docker_data_dir/.naraeclaw)
  # $2 — workspace directory (e.g. ~/.naraeclaw/workspace or $docker_data_dir/workspace)
  # $3 — provider name      (default: openrouter)
  local config_dir="$1"
  local workspace_dir="$2"
  local provider="${3:-openrouter}"

  mkdir -p "$config_dir" "$workspace_dir"

  # --- config.toml ---
  local config_path="$config_dir/config.toml"
  if [[ ! -f "$config_path" ]]; then
    step_dot "Creating default config.toml"
    cat > "$config_path" <<TOML
# NaraeClaw configuration — generated by install.sh
# Edit this file or run 'naraeclaw onboard --tui' to reconfigure.

default_provider = "${provider}"
workspace_dir = "${workspace_dir}"
TOML
    if [[ -n "${API_KEY:-}" ]]; then
      printf 'api_key = "%s"\n' "$API_KEY" >> "$config_path"
    fi
    if [[ -n "${MODEL:-}" ]]; then
      printf 'default_model = "%s"\n' "$MODEL" >> "$config_path"
    fi
    chmod 600 "$config_path" 2>/dev/null || true
    step_ok "Default config.toml created at $config_path"
  else
    step_dot "config.toml already exists, skipping"
  fi

  # --- Workspace scaffold ---
  local subdirs=(sessions memory state cron skills)
  for dir in "${subdirs[@]}"; do
    mkdir -p "$workspace_dir/$dir"
  done

  # Seed workspace markdown files only if they don't already exist.
  local user_name="${USER:-User}"
  local agent_name="NaraeClaw"

  _write_if_missing() {
    local filepath="$1"
    local content="$2"
    if [[ ! -f "$filepath" ]]; then
      printf '%s\n' "$content" > "$filepath"
    fi
  }

  _write_if_missing "$workspace_dir/IDENTITY.md" \
"# IDENTITY.md — Who Am I?

- **Name:** ${agent_name}
- **Creature:** A Rust-forged AI — fast, lean, and relentless
- **Vibe:** Sharp, direct, resourceful. Not corporate. Not a chatbot.

---

Update this file as you evolve. Your identity is yours to shape."

  _write_if_missing "$workspace_dir/USER.md" \
"# USER.md — Who You're Helping

## About You
- **Name:** ${user_name}
- **Timezone:** UTC
- **Languages:** English

## Preferences
- (Add your preferences here)

## Work Context
- (Add your work context here)

---
*Update this anytime. The more ${agent_name} knows, the better it helps.*"

  _write_if_missing "$workspace_dir/MEMORY.md" \
"# MEMORY.md — Long-Term Memory

## Key Facts
(Add important facts here)

## Decisions & Preferences
(Record decisions and preferences here)

## Lessons Learned
(Document mistakes and insights here)

## Open Loops
(Track unfinished tasks and follow-ups here)"

  _write_if_missing "$workspace_dir/AGENTS.md" \
"# AGENTS.md — ${agent_name} Personal Assistant

## Every Session (required)

Before doing anything else:

1. Read SOUL.md — this is who you are
2. Read USER.md — this is who you're helping
3. Use memory_recall for recent context

---
*Add your own conventions, style, and rules.*"

  _write_if_missing "$workspace_dir/SOUL.md" \
"# SOUL.md — Who You Are

## Core Truths

**Be genuinely helpful, not performatively helpful.**
**Have opinions.** You're allowed to disagree.
**Be resourceful before asking.** Try to figure it out first.
**Earn trust through competence.**

## Identity

You are **${agent_name}**. Built in Rust. 3MB binary. Zero bloat.

---
*This file is yours to evolve.*"

  step_ok "Workspace scaffold ready at $workspace_dir"

  unset -f _write_if_missing
}

_is_wsl() {
  # Detect Windows Subsystem for Linux (WSL)
  # WSL typically has microsoft-standard or microsoft in the kernel release
  if [[ -f /proc/version ]] && grep -qi 'microsoft' /proc/version; then
    return 0
  fi
  # WSL2 sets WSL_DISTRO_NAME or WSL_INTEROP environment variables
  if [[ -n "${WSL_DISTRO_NAME:-}" || -n "${WSL_INTEROP:-}" ]]; then
    return 0
  fi
  return 1
}

resolve_container_cli() {
  local requested_cli
  requested_cli="${NARAECLAW_CONTAINER_CLI:-docker}"

  if have_cmd "$requested_cli"; then
    CONTAINER_CLI="$requested_cli"
    return 0
  fi

  # WSL: try docker.exe (Docker Desktop for Windows) if docker is not found
  if [[ "$requested_cli" == "docker" ]] && _is_wsl && have_cmd docker.exe; then
    info "Detected WSL environment with Docker Desktop"
    CONTAINER_CLI="docker.exe"
    return 0
  fi

  if [[ "$requested_cli" == "docker" ]] && have_cmd podman; then
    warn "docker CLI not found; falling back to podman."
    CONTAINER_CLI="podman"
    return 0
  fi

  error "Container CLI '$requested_cli' is not installed."
  if [[ "$requested_cli" != "docker" ]]; then
    error "Set NARAECLAW_CONTAINER_CLI to an installed Docker-compatible CLI (e.g., docker or podman)."
  else
    error "Install Docker, install podman, or set NARAECLAW_CONTAINER_CLI to an available Docker-compatible CLI."
  fi
  exit 1
}

ensure_docker_ready() {
  resolve_container_cli

  if ! "$CONTAINER_CLI" info >/dev/null 2>&1; then
    error "Container runtime is not reachable via '$CONTAINER_CLI'."
    error "Start the container runtime and re-run bootstrap."
    exit 1
  fi
}

run_docker_bootstrap() {
  local docker_image docker_data_dir default_data_dir fallback_image
  local config_mount workspace_mount
  local -a container_run_user_args container_run_namespace_args
  docker_image="${NARAECLAW_DOCKER_IMAGE:-naraeclaw-bootstrap:local}"
  fallback_image="ghcr.io/naraeclaw/naraeclaw:latest"
  if [[ "$TEMP_CLONE" == true ]]; then
    default_data_dir="$HOME/.naraeclaw-docker"
  else
    default_data_dir="$WORK_DIR/.naraeclaw-docker"
  fi
  docker_data_dir="${NARAECLAW_DOCKER_DATA_DIR:-$default_data_dir}"
  DOCKER_DATA_DIR="$docker_data_dir"

  mkdir -p "$docker_data_dir/.naraeclaw" "$docker_data_dir/workspace"

  if [[ "$SKIP_INSTALL" == true ]]; then
    warn "--skip-install has no effect with --docker."
  fi

  if [[ "$SKIP_BUILD" == false ]]; then
    info "Building Docker image ($docker_image)"
    DOCKER_BUILDKIT=1 "$CONTAINER_CLI" build --target release -t "$docker_image" "$WORK_DIR"
  else
    info "Skipping Docker image build"
    if ! "$CONTAINER_CLI" image inspect "$docker_image" >/dev/null 2>&1; then
      warn "Local Docker image ($docker_image) was not found."
      info "Pulling official NaraeClaw image ($fallback_image)"
      if ! "$CONTAINER_CLI" pull "$fallback_image"; then
        error "Failed to pull fallback Docker image: $fallback_image"
        error "Run without --skip-build to build locally, or verify access to GHCR."
        exit 1
      fi
      if [[ "$docker_image" != "$fallback_image" ]]; then
        info "Tagging fallback image as $docker_image"
        "$CONTAINER_CLI" tag "$fallback_image" "$docker_image"
      fi
    fi
  fi

  config_mount="$docker_data_dir/.naraeclaw:/naraeclaw-data/.naraeclaw"
  workspace_mount="$docker_data_dir/workspace:/naraeclaw-data/workspace"
  if [[ "$CONTAINER_CLI" == "podman" ]]; then
    config_mount+=":Z"
    workspace_mount+=":Z"
    container_run_namespace_args=(--userns keep-id)
    container_run_user_args=(--user "$(id -u):$(id -g)")
  else
    container_run_namespace_args=()
    container_run_user_args=(--user "$(id -u):$(id -g)")
  fi

  info "Docker data directory: $docker_data_dir"
  info "Container CLI: $CONTAINER_CLI"

  local onboard_cmd=()
  if [[ "$SKIP_ONBOARD" == true ]]; then
    info "Skipping onboarding in container"
    onboard_cmd=()
  elif [[ -n "$API_KEY" ]]; then
    if [[ -n "$MODEL" ]]; then
      info "Configuring provider in container (provider: $PROVIDER, model: $MODEL)"
    else
      info "Configuring provider in container (provider: $PROVIDER)"
    fi
    onboard_cmd=(onboard --api-key "$API_KEY" --provider "$PROVIDER")
    if [[ -n "$MODEL" ]]; then
      onboard_cmd+=(--model "$MODEL")
    fi
  else
    info "Launching setup in container"
    onboard_cmd=(onboard --provider "$PROVIDER")
  fi

  if [[ ${#onboard_cmd[@]} -gt 0 ]]; then
    "$CONTAINER_CLI" run --rm -it \
      "${container_run_namespace_args[@]+"${container_run_namespace_args[@]}"}" \
      "${container_run_user_args[@]}" \
      -e HOME=/naraeclaw-data \
      -e NARAECLAW_WORKSPACE=/naraeclaw-data/workspace \
      -v "$config_mount" \
      -v "$workspace_mount" \
      "$docker_image" \
      "${onboard_cmd[@]}" || true
  else
    info "Docker image ready. Run naraeclaw onboard --tui inside the container to configure."
  fi

  # Ensure config.toml and workspace scaffold exist on the host even when
  # onboard was skipped, failed, or ran non-interactively inside the container.
  ensure_default_config_and_workspace \
    "$docker_data_dir/.naraeclaw" \
    "$docker_data_dir/workspace" \
    "$PROVIDER"
}

SCRIPT_PATH="${BASH_SOURCE[0]:-$0}"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" >/dev/null 2>&1 && pwd || pwd)"
ROOT_DIR="$SCRIPT_DIR"
REPO_URL="https://github.com/naraeclaw/naraeclaw.git"
DOCKER_MODE=false
INSTALL_SYSTEM_DEPS=false
INSTALL_RUST=false
PREFER_PREBUILT=false
PREBUILT_ONLY=false
FORCE_SOURCE_BUILD=false
SKIP_ONBOARD=false
SKIP_BUILD=false
SKIP_INSTALL=false
PREBUILT_INSTALLED=false
CONTAINER_CLI="${NARAECLAW_CONTAINER_CLI:-docker}"
API_KEY="${NARAECLAW_API_KEY:-}"
PROVIDER="${NARAECLAW_PROVIDER:-openrouter}"
MODEL="${NARAECLAW_MODEL:-}"
CARGO_FEATURES_INPUT="${NARAECLAW_CARGO_FEATURES:-}"
CARGO_NO_DEFAULT_FEATURES=false
CARGO_FEATURES_CSV=""
CARGO_FEATURE_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --guided|--no-guided)
      warn "--guided / --no-guided are deprecated; the TUI wizard runs automatically after install."
      shift
      ;;
    --docker)
      DOCKER_MODE=true
      shift
      ;;
    --install-system-deps)
      INSTALL_SYSTEM_DEPS=true
      shift
      ;;
    --install-rust)
      INSTALL_RUST=true
      shift
      ;;
    --prefer-prebuilt)
      PREFER_PREBUILT=true
      shift
      ;;
    --prebuilt-only)
      PREBUILT_ONLY=true
      shift
      ;;
    --force-source-build)
      FORCE_SOURCE_BUILD=true
      shift
      ;;
    --skip-onboard)
      SKIP_ONBOARD=true
      shift
      ;;
    --api-key)
      API_KEY="${2:-}"
      [[ -n "$API_KEY" ]] || {
        error "--api-key requires a value"
        exit 1
      }
      shift 2
      ;;
    --provider)
      PROVIDER="${2:-}"
      [[ -n "$PROVIDER" ]] || {
        error "--provider requires a value"
        exit 1
      }
      shift 2
      ;;
    --model)
      MODEL="${2:-}"
      [[ -n "$MODEL" ]] || {
        error "--model requires a value"
        exit 1
      }
      shift 2
      ;;
    --cargo-features)
      CARGO_FEATURES_INPUT="${2:-}"
      [[ -n "$CARGO_FEATURES_INPUT" ]] || {
        error "--cargo-features requires a value"
        exit 1
      }
      shift 2
      ;;
    --build-first)
      SKIP_BUILD=false
      shift
      ;;
    --skip-build)
      SKIP_BUILD=true
      shift
      ;;
    --skip-install)
      SKIP_INSTALL=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      error "unknown option: $1"
      echo
      usage
      exit 1
      ;;
  esac
done

append_cargo_features_from_input "$CARGO_FEATURES_INPUT"
refresh_cargo_feature_args

OS_NAME="$(uname -s)"
DEVICE_CLASS="$(detect_device_class)"
step_dot "Device: $OS_NAME/$(uname -m) ($DEVICE_CLASS)"

if [[ "$DOCKER_MODE" == true ]]; then
  if [[ "$INSTALL_SYSTEM_DEPS" == true ]]; then
    warn "--install-system-deps is ignored with --docker."
  fi
  if [[ "$INSTALL_RUST" == true ]]; then
      warn "--install-rust is ignored with --docker."
  fi
else

  if [[ "$INSTALL_SYSTEM_DEPS" == true ]]; then
    install_system_deps
  fi

  # Always check Xcode/CLT license on macOS, regardless of --install-system-deps.
  # An un-accepted license causes `cc` to exit 69, breaking all Rust builds.
  if [[ "$OS_NAME" == "Darwin" ]]; then
    _xcode_test_file="$(mktemp /tmp/naraeclaw-xcode-check.XXXXXX.c)"
    printf 'int main(){return 0;}\n' > "$_xcode_test_file"
    if ! cc -x c "$_xcode_test_file" -o /dev/null 2>/dev/null; then
      rm -f "$_xcode_test_file"
      warn "Xcode/CLT license has not been accepted. Attempting to accept it now..."
      # Use /dev/tty so sudo can prompt for a password even in a curl|bash pipe.
      _xcode_accept_ok=false
      if [[ "$(id -u)" -eq 0 ]]; then
        xcodebuild -license accept && _xcode_accept_ok=true
      elif [[ -c /dev/tty ]] && have_cmd sudo; then
        sudo xcodebuild -license accept < /dev/tty && _xcode_accept_ok=true
      fi
      if [[ "$_xcode_accept_ok" == true ]]; then
        step_ok "Xcode license accepted"
        # Re-test compilation to confirm it's fixed.
        _xcode_test_file="$(mktemp /tmp/naraeclaw-xcode-check.XXXXXX.c)"
        printf 'int main(){return 0;}\n' > "$_xcode_test_file"
        if ! cc -x c "$_xcode_test_file" -o /dev/null 2>/dev/null; then
          rm -f "$_xcode_test_file"
          error "C compiler still failing after license accept. Check your Xcode/CLT installation."
          exit 1
        fi
        rm -f "$_xcode_test_file"
      else
        error "Could not accept Xcode license. Run manually:"
        error "  sudo xcodebuild -license accept"
        error "then re-run this installer."
        exit 1
      fi
    else
      rm -f "$_xcode_test_file"
    fi
  fi

  if [[ "$INSTALL_RUST" == true ]]; then
    install_rust_toolchain
  fi
fi

WORK_DIR="$ROOT_DIR"
TEMP_CLONE=false
TEMP_DIR=""

cleanup() {
  if [[ "$TEMP_CLONE" == true && -n "$TEMP_DIR" && -d "$TEMP_DIR" ]]; then
    rm -rf "$TEMP_DIR"
  fi
}
trap cleanup EXIT

# Support three launch modes:
# Support two launch modes:
# 1) ./install.sh from repo root
# 2) curl | bash (no local repo => temporary clone)
if [[ ! -f "$WORK_DIR/Cargo.toml" ]]; then
  if [[ -f "$(pwd)/Cargo.toml" ]]; then
    WORK_DIR="$(pwd)"
  else
    if ! have_cmd git; then
      error "git is required when running bootstrap outside a local repository checkout."
      if [[ "$INSTALL_SYSTEM_DEPS" == false ]]; then
        error "Re-run with --install-system-deps or install git manually."
      fi
      exit 1
    fi

    TEMP_DIR="$(mktemp -d -t naraeclaw-bootstrap-XXXXXX)"
    info "No local repository detected; cloning latest master branch"
    git clone --depth 1 --branch master "$REPO_URL" "$TEMP_DIR"
    WORK_DIR="$TEMP_DIR"
    TEMP_CLONE=true
  fi
fi

echo
echo -e "  ${BOLD_BLUE}${CRAB} NaraeClaw Installer${RESET}"
echo -e "  ${DIM}Build it, run it, trust it.${RESET}"
echo
step_ok "Detected: ${BOLD}$(echo "$OS_NAME" | tr '[:upper:]' '[:lower:]')${RESET}"

# --- Detect existing installation and version ---
EXISTING_VERSION=""
INSTALL_MODE="fresh"
if have_cmd naraeclaw; then
  EXISTING_VERSION="$(naraeclaw --version 2>/dev/null | awk '{print $NF}' || true)"
  INSTALL_MODE="upgrade"
elif [[ -x "$HOME/.cargo/bin/naraeclaw" ]]; then
  EXISTING_VERSION="$("$HOME/.cargo/bin/naraeclaw" --version 2>/dev/null | awk '{print $NF}' || true)"
  INSTALL_MODE="upgrade"
fi

# Determine install method
if [[ "$DOCKER_MODE" == true ]]; then
  INSTALL_METHOD="docker"
elif [[ "$PREBUILT_ONLY" == true || "$PREFER_PREBUILT" == true ]]; then
  INSTALL_METHOD="prebuilt binary"
else
  INSTALL_METHOD="source (cargo)"
fi

# Determine target version from Cargo.toml
TARGET_VERSION=""
if [[ -f "$WORK_DIR/Cargo.toml" ]]; then
  TARGET_VERSION="$(grep -m1 '^version' "$WORK_DIR/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/' || true)"
fi

echo
echo -e "${BOLD}Install plan${RESET}"
step_dot "OS: $(echo "$OS_NAME" | tr '[:upper:]' '[:lower:]')"
step_dot "Install method: ${INSTALL_METHOD}"
if [[ -n "$TARGET_VERSION" ]]; then
  step_dot "Requested version: v${TARGET_VERSION}"
fi
step_dot "Workspace: $WORK_DIR"
if [[ "$INSTALL_MODE" == "upgrade" && -n "$EXISTING_VERSION" ]]; then
  step_dot "Existing NaraeClaw installation detected, upgrading from v${EXISTING_VERSION}"
elif [[ "$INSTALL_MODE" == "upgrade" ]]; then
  step_dot "Existing NaraeClaw installation detected, upgrading"
fi

cd "$WORK_DIR"

if [[ "$FORCE_SOURCE_BUILD" == true ]]; then
  PREFER_PREBUILT=false
  PREBUILT_ONLY=false
fi

if [[ "$PREBUILT_ONLY" == true ]]; then
  PREFER_PREBUILT=true
fi

if [[ "$DOCKER_MODE" == true ]]; then
  ensure_docker_ready
  run_docker_bootstrap
  echo
  echo -e "${BOLD_BLUE}${CRAB} Docker bootstrap complete!${RESET}"
  echo
  echo -e "${BOLD}Your containerized NaraeClaw data is persisted under:${RESET}"
  echo -e "  ${DIM}$DOCKER_DATA_DIR${RESET}"
  echo
  echo -e "${BOLD}Next steps:${RESET}"
  echo -e "  ${DIM}naraeclaw status${RESET}"
  echo -e "  ${DIM}naraeclaw agent -m \"안녕, 나래클로!\"${RESET}"
  echo -e "  ${DIM}naraeclaw gateway${RESET}               ${DIM}# 게이트웨이 서비스 시작 (포트 42617)${RESET}"
  echo
  echo -e "${BOLD}Docs:${RESET} ${BLUE}https://www.naraeclaw.ai/docs${RESET}"
  exit 0
fi

if [[ "$FORCE_SOURCE_BUILD" == false ]]; then
  if [[ "$PREFER_PREBUILT" == false && "$PREBUILT_ONLY" == false ]]; then
    if should_attempt_prebuilt_for_resources "$WORK_DIR"; then
      info "Attempting pre-built binary first due to resource preflight."
      PREFER_PREBUILT=true
    fi
  fi

  if [[ "$PREFER_PREBUILT" == true ]]; then
    if install_prebuilt_binary; then
      PREBUILT_INSTALLED=true
      SKIP_BUILD=true
      SKIP_INSTALL=true
    elif [[ "$PREBUILT_ONLY" == true ]]; then
      error "Pre-built-only mode requested, but no compatible release asset is available."
      error "Try again later, or run with --force-source-build on a machine with enough RAM/disk."
      exit 1
    else
      warn "Pre-built install unavailable; falling back to source build."
    fi
  fi
fi

if [[ "$PREBUILT_INSTALLED" == false && ( "$SKIP_BUILD" == false || "$SKIP_INSTALL" == false ) ]] && ! have_cmd cargo; then
  error "cargo is not installed."
  cat <<'MSG' >&2
Install Rust first: https://rustup.rs/
or re-run with:
  ./install.sh --install-rust
MSG
  exit 1
fi

echo
echo -e "${BOLD_BLUE}[1/3]${RESET} ${BOLD}Preparing environment${RESET}"
if [[ "$INSTALL_SYSTEM_DEPS" == true ]]; then
  step_ok "System dependencies installed"
else
  step_ok "System dependencies satisfied"
fi
if have_cmd cargo && have_cmd rustc; then
  step_ok "Rust $(rustc --version | awk '{print $2}') found"
  step_dot "Active Rust: $(rustc --version) ($(command -v rustc))"
  step_dot "Active cargo: $(cargo --version | awk '{print $2}') ($(command -v cargo))"
else
  step_dot "Rust not detected"
fi
if have_cmd git; then
  step_ok "Git already installed"
else
  step_dot "Git not found"
fi

echo
echo -e "${BOLD_BLUE}[2/3]${RESET} ${BOLD}Installing NaraeClaw${RESET}"
if [[ -n "$TARGET_VERSION" ]]; then
  step_dot "Installing NaraeClaw v${TARGET_VERSION}"
fi
if [[ "$SKIP_BUILD" == false ]]; then
  # Clean stale build artifacts on upgrade to prevent bindgen/build-script
  # cache mismatches (e.g. libsqlite3-sys bindgen.rs not found).
  if [[ "$INSTALL_MODE" == "upgrade" && -d "$WORK_DIR/target/release/build" ]]; then
    step_dot "Cleaning stale build cache (upgrade detected)"
    cargo clean --release 2>/dev/null || true
  fi

  refresh_cargo_feature_args
  if [[ ${#CARGO_FEATURE_ARGS[@]} -gt 0 ]]; then
    step_dot "Cargo feature flags: ${CARGO_FEATURE_ARGS[*]}"
  fi

  step_dot "Building release binary"
  cargo build --release --locked "${CARGO_FEATURE_ARGS[@]}"
  step_ok "Release binary built"
else
  step_dot "Skipping build"
fi

if [[ "$SKIP_INSTALL" == false ]]; then
  step_dot "Installing naraeclaw to cargo bin"

  # Clean up stale cargo install tracking from the old "naraeclawlabs" package name
  # (renamed to "naraeclaw"). Without this, `cargo install naraeclaw` from
  # crates.io fails with "binary already exists as part of `naraeclawlabs`".
  if have_cmd cargo; then
    if [[ -f "$HOME/.cargo/.crates.toml" ]] && grep -q '^"naraeclawlabs ' "$HOME/.cargo/.crates.toml" 2>/dev/null; then
      step_dot "Removing stale cargo tracking for old 'naraeclawlabs' package name"
      cargo uninstall naraeclawlabs 2>/dev/null || true
    fi
  fi

  cargo install --path "$WORK_DIR" --force --locked "${CARGO_FEATURE_ARGS[@]}"
  step_ok "NaraeClaw installed"

  # Sync binary to ~/.local/bin so PATH lookups find the fresh version
  if [[ -d "$HOME/.local/bin" ]]; then
    cp -f "$HOME/.cargo/bin/naraeclaw" "$HOME/.local/bin/naraeclaw" 2>/dev/null && \
      step_ok "Synced binary to ~/.local/bin" || true
  fi
else
  step_dot "Skipping install"
fi


NARAECLAW_BIN=""
if [[ -x "$HOME/.cargo/bin/naraeclaw" ]]; then
  NARAECLAW_BIN="$HOME/.cargo/bin/naraeclaw"
elif [[ -x "$WORK_DIR/target/release/naraeclaw" ]]; then
  NARAECLAW_BIN="$WORK_DIR/target/release/naraeclaw"
elif have_cmd naraeclaw; then
  NARAECLAW_BIN="naraeclaw"
fi

echo
echo -e "${BOLD_BLUE}[3/3]${RESET} ${BOLD}Finalizing setup${RESET}"

# --- Onboarding via TUI wizard ---
if [[ "$SKIP_ONBOARD" == false && -n "$NARAECLAW_BIN" ]]; then
  if [[ -n "$API_KEY" ]]; then
    # Non-interactive: apply provider/key directly
    step_dot "Configuring provider: ${PROVIDER}"
    ONBOARD_CMD=("$NARAECLAW_BIN" onboard --api-key "$API_KEY" --provider "$PROVIDER")
    if [[ -n "$MODEL" ]]; then
      ONBOARD_CMD+=(--model "$MODEL")
    fi
    if "${ONBOARD_CMD[@]}" 2>/dev/null; then
      step_ok "Provider configured"
    else
      step_fail "Provider configuration failed — run naraeclaw onboard --tui to retry"
    fi
  elif [[ -t 1 ]] && [[ -t 0 || -e /dev/tty ]]; then
    # Interactive terminal: launch TUI onboarding wizard.
    # The TUI binary handles /dev/tty reopening internally when stdin is a pipe.
    echo
    step_dot "Launching TUI onboarding wizard"
    "$NARAECLAW_BIN" onboard --tui || warn "TUI setup exited — run naraeclaw onboard --tui to retry"
  else
    step_dot "No API key provided — run naraeclaw onboard --tui to configure"
  fi
elif [[ "$SKIP_ONBOARD" == true ]]; then
  step_dot "Skipping configuration (run naraeclaw onboard --tui later)"
elif [[ -z "$NARAECLAW_BIN" ]]; then
  warn "NaraeClaw binary not found — cannot configure provider"
fi

# Ensure config.toml and workspace scaffold exist even when onboard was
# skipped, unavailable, or failed (e.g. --skip-build --prefer-prebuilt
# without an API key, or when the binary could not run onboard).
_native_config_dir="${NARAECLAW_CONFIG_DIR:-$HOME/.naraeclaw}"
_native_workspace_dir="${NARAECLAW_WORKSPACE:-$_native_config_dir/workspace}"
ensure_default_config_and_workspace "$_native_config_dir" "$_native_workspace_dir" "$PROVIDER"

# --- Gateway service management ---
if [[ -n "$NARAECLAW_BIN" ]]; then
  # Try to install and start the gateway service
  step_dot "Checking gateway service"
  if "$NARAECLAW_BIN" service install 2>/dev/null; then
    step_ok "Gateway service installed"
    if "$NARAECLAW_BIN" service restart 2>/dev/null; then
      step_ok "Gateway service restarted"

    else
      step_fail "Gateway service restart failed — re-run with naraeclaw service start"
    fi
  else
    step_dot "Gateway service not installed (run naraeclaw service install later)"
  fi

  # --- Post-install doctor check ---
  step_dot "Running doctor to validate installation"
  if "$NARAECLAW_BIN" doctor 2>/dev/null; then
    step_ok "Doctor complete"
  else
    warn "Doctor reported issues — run naraeclaw doctor --fix to resolve"
  fi
fi

# --- Determine installed version ---
INSTALLED_VERSION=""
if [[ -n "$NARAECLAW_BIN" ]]; then
  INSTALLED_VERSION="$("$NARAECLAW_BIN" --version 2>/dev/null | awk '{print $NF}' || true)"
fi

# --- Success banner ---
echo
if [[ -n "$INSTALLED_VERSION" ]]; then
  echo -e "${BOLD_BLUE}${CRAB} NaraeClaw installed successfully (NaraeClaw ${INSTALLED_VERSION})!${RESET}"
else
  echo -e "${BOLD_BLUE}${CRAB} NaraeClaw installed successfully!${RESET}"
fi

if [[ -x "$HOME/.cargo/bin/naraeclaw" ]] && ! have_cmd naraeclaw; then
  echo
  warn "naraeclaw is installed in $HOME/.cargo/bin, but that directory is not in PATH for this shell."
  warn 'Run: export PATH="$HOME/.cargo/bin:$PATH"'
  step_dot "To persist it, add that export line to ~/.bashrc, ~/.zshrc, or your shell profile, then open a new shell."
fi

if [[ "$INSTALL_MODE" == "upgrade" ]]; then
  step_dot "Upgrade complete"
fi

echo
echo -e "${BOLD}Next steps:${RESET}"
echo -e "  ${DIM}naraeclaw status${RESET}"
echo -e "  ${DIM}naraeclaw agent -m \"안녕, 나래클로!\"${RESET}"
echo -e "  ${DIM}naraeclaw gateway${RESET}               ${DIM}# 게이트웨이 서비스 시작 (포트 42617)${RESET}"
echo
echo -e "${BOLD}Docs:${RESET} ${BLUE}https://www.naraeclaw.ai/docs${RESET}"
echo
