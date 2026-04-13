# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **Shared instructions live in [`AGENTS.md`](./AGENTS.md).**
> This file contains only Claude Code-specific directives.

Claude Code should read and follow all instructions in `AGENTS.md` at the repository root for project conventions, commands, risk tiers, workflow rules, and anti-patterns.

## Commands

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --all-targets -- -D warnings

# Run all tests (unit + component + integration + system)
cargo test

# Run unit tests only (fastest)
cargo test --lib

# Run a single test by name
cargo test --test integration <test_name>

# Run component / integration / system tests
cargo test --test component
cargo test --test integration
cargo test --test system

# Live tests (requires real API credentials, marked #[ignore])
cargo test --test live -- --ignored

# Local CI using Docker (recommended pre-PR)
./dev/ci.sh all          # lint + test + build + security + docker-smoke
./dev/ci.sh lint         # fmt + clippy correctness gate
./dev/ci.sh lint-strict  # fmt + full clippy warnings gate
./dev/ci.sh build        # release build smoke check

# Justfile shortcuts (requires `just`)
just ci          # fmt-check + lint + test
just fmt         # cargo fmt --all
just build       # cargo build --release --locked
just dev [ARGS]  # cargo run -- [ARGS]
```

## Architecture

ZeroClaw is a Rust-edition-2024 workspace. The `zeroclaw` binary (`src/main.rs`) compiles with `agent-runtime` feature enabled by default, which gates most of the agent subsystems. Without that feature, the binary is just a config+provider+memory CLI.

**Trait contract layer** (`crates/zeroclaw-api/`): All extension points are traits. Concrete crates depend inward toward these contracts; never laterally across concrete crates.

**Key data-flow**: inbound message → `zeroclaw-channels` (transport) → `zeroclaw-runtime/agent/` (agent loop) → `zeroclaw-providers` (LLM) → `zeroclaw-tools` (execution) → response back via the same channel.

**`zeroclaw-runtime/src/` subsystems:**
- `agent/` — main agent loop and turn management
- `security/` — access control and policy enforcement (high-risk)
- `cron/` — scheduled task execution
- `sop/` — standard operating procedures engine
- `skills/` and `skillforge/` — skill registration and forge
- `onboard/` — TUI onboarding wizard (driven by `zeroclaw-tui`)
- `observability/` — Prometheus / OpenTelemetry metrics
- `hooks/` — pre/post-action hook dispatch

**Channels** (`crates/zeroclaw-channels/`): 30+ platform integrations. The `orchestrator/` subdirectory handles channel lifecycle, routing, and the media pipeline. Each channel is behind a `channel-<name>` Cargo feature.

**Config** (`crates/zeroclaw-config/`): TOML-based, schema-validated. The `Configurable` derive macro (from `zeroclaw-macros`) generates schema and merge logic. Config keys are a public contract — document defaults and migration path for any change.

**Test infrastructure** (`tests/support/`): shared mocks (`MockProvider`, `MockChannel`, `EchoTool`, etc.) and `TraceLlmProvider` for JSON fixture replay. All test binaries include `mod support;`. Fixtures live in `tests/fixtures/traces/`.

## Claude Code Settings

## Hooks

_No custom hooks defined yet._

## Slash Commands

_No custom slash commands defined yet._
