# syntax=docker/dockerfile:1.7

# ── Stage 0: Frontend build ─────────────────────────────────────
FROM node:25-bookworm-slim AS web-builder
WORKDIR /web
COPY web/package.json web/package-lock.json* ./
RUN npm ci --ignore-scripts 2>/dev/null || npm install --ignore-scripts
COPY web/ .
RUN npm run build

# ── Stage 1: Build ────────────────────────────────────────────
FROM rust:1.94-slim@sha256:cf09adf8c3ebaba10779e5c23ff7fe4df4cccdab8a91f199b0c142c53fef3e1a AS builder

WORKDIR /app
ARG NARAECLAW_CARGO_FEATURES="whatsapp-web"

# Install build dependencies
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

# 1. Copy manifests to cache dependencies
COPY Cargo.toml Cargo.lock ./
# Include workspace member manifests and sources for path dependencies.
COPY crates/ crates/
# Include tauri workspace member manifest (desktop app, but needed for workspace resolution).
# .dockerignore whitelists only Cargo.toml; src and build.rs are stubbed below.
COPY apps/tauri/Cargo.toml apps/tauri/Cargo.toml
# Create dummy targets declared in Cargo.toml so manifest parsing succeeds.
RUN mkdir -p src apps/tauri/src \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && echo "fn main() {}" > apps/tauri/src/main.rs \
    && echo "fn main() {}" > apps/tauri/build.rs
RUN --mount=type=cache,id=naraeclaw-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=naraeclaw-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=naraeclaw-target,target=/app/target,sharing=locked \
    if [ -n "$NARAECLAW_CARGO_FEATURES" ]; then \
      cargo build --release --locked --features "$NARAECLAW_CARGO_FEATURES"; \
    else \
      cargo build --release --locked; \
    fi
RUN rm -rf src

# 2. Copy build-relevant source paths (avoid cache-busting on docs/tests/scripts)
COPY src/ src/
COPY *.rs .
RUN touch src/main.rs
RUN --mount=type=cache,id=naraeclaw-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=naraeclaw-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=naraeclaw-target,target=/app/target,sharing=locked \
    rm -rf target/release/.fingerprint/naraeclaw-* \
           target/release/deps/naraeclaw-* \
           target/release/incremental/naraeclaw-* && \
    if [ -n "$NARAECLAW_CARGO_FEATURES" ]; then \
      cargo build --release --locked --features "$NARAECLAW_CARGO_FEATURES"; \
    else \
      cargo build --release --locked; \
    fi && \
    cp target/release/naraeclaw /app/naraeclaw && \
    strip /app/naraeclaw
RUN size=$(stat -c%s /app/naraeclaw) && \
    if [ "$size" -lt 1000000 ]; then echo "ERROR: binary too small (${size} bytes), likely dummy build artifact" && exit 1; fi

# Prepare runtime directory structure and default config inline (no extra stage)
RUN mkdir -p /naraeclaw-data/.naraeclaw /naraeclaw-data/workspace && \
    printf '%s\n' \
        'workspace_dir = "/naraeclaw-data/workspace"' \
        'config_path = "/naraeclaw-data/.naraeclaw/config.toml"' \
        'api_key = ""' \
        'default_provider = "openrouter"' \
        'default_model = "anthropic/claude-sonnet-4-20250514"' \
        'default_temperature = 0.7' \
        '' \
        '[gateway]' \
        'port = 42617' \
        'host = "[::]"' \
        'allow_public_bind = true' \
        'require_pairing = false' \
        'web_dist_dir = "/naraeclaw-data/web/dist"' \
        '' \
        '[autonomy]' \
        'level = "supervised"' \
        'auto_approve = ["file_read", "file_write", "file_edit", "memory_recall", "memory_store", "web_search_tool", "web_fetch", "calculator", "glob_search", "content_search", "image_info", "weather", "git_operations"]' \
        > /naraeclaw-data/.naraeclaw/config.toml && \
    chown -R 65534:65534 /naraeclaw-data

# ── Stage 2: Development Runtime (Debian) ────────────────────
FROM debian:trixie-slim@sha256:4ffb3a1511099754cddc70eb1b12e50ffdb67619aa0ab6c13fcd800a78ef7c7a AS dev

# Install essential runtime dependencies only (use docker-compose.override.yml for dev tools)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /naraeclaw-data /naraeclaw-data
COPY --from=builder /app/naraeclaw /usr/local/bin/naraeclaw
COPY --from=web-builder /web/dist /naraeclaw-data/web/dist

# Overwrite minimal config with DEV template (Ollama defaults)
COPY dev/config.template.toml /naraeclaw-data/.naraeclaw/config.toml
RUN chown 65534:65534 /naraeclaw-data/.naraeclaw/config.toml

# Environment setup
# Ensure UTF-8 locale so CJK / multibyte input is handled correctly
ENV LANG=C.UTF-8
# Use consistent workspace path
ENV NARAECLAW_WORKSPACE=/naraeclaw-data/workspace
ENV HOME=/naraeclaw-data
# Defaults for local dev (Ollama) - matches config.template.toml
ENV PROVIDER="ollama"
ENV NARAECLAW_MODEL="llama3.2"
ENV NARAECLAW_GATEWAY_PORT=42617

# Note: API_KEY is intentionally NOT set here to avoid confusion.
# It is set in config.toml as the Ollama URL.

WORKDIR /naraeclaw-data
USER 65534:65534
EXPOSE 42617
HEALTHCHECK --interval=60s --timeout=10s --retries=3 --start-period=10s \
    CMD ["naraeclaw", "status", "--format=exit-code"]
ENTRYPOINT ["naraeclaw"]
CMD ["daemon"]

# ── Stage 3: Production Runtime (Distroless) ─────────────────
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:8f960b7fc6a5d6e28bb07f982655925d6206678bd9a6cde2ad00ddb5e2077d78 AS release

COPY --from=builder /app/naraeclaw /usr/local/bin/naraeclaw
COPY --from=builder /naraeclaw-data /naraeclaw-data
COPY --from=web-builder /web/dist /naraeclaw-data/web/dist

# Environment setup
# Ensure UTF-8 locale so CJK / multibyte input is handled correctly
ENV LANG=C.UTF-8
ENV NARAECLAW_WORKSPACE=/naraeclaw-data/workspace
ENV HOME=/naraeclaw-data
# Default provider and model are set in config.toml, not here,
# so config file edits are not silently overridden
#ENV PROVIDER=
ENV NARAECLAW_GATEWAY_PORT=42617

# API_KEY must be provided at runtime!

WORKDIR /naraeclaw-data
USER 65534:65534
EXPOSE 42617
HEALTHCHECK --interval=60s --timeout=10s --retries=3 --start-period=10s \
    CMD ["naraeclaw", "status", "--format=exit-code"]
ENTRYPOINT ["naraeclaw"]
CMD ["daemon"]
