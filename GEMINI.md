# GEMINI.md - NaraeClaw Project Context

## Project Overview
**NaraeClaw (나래클로)** is a lightweight, Korean-first AI agent runtime optimized for messaging platforms like Telegram. It is a fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw), aimed at reducing overhead and providing a seamless experience for Korean users.

- **Primary Goal:** Fast, responsive AI assistant via Telegram (webhook-based, no polling latency).
- **V1 Complete (2026-04-13):** Telegram webhook migration, lightweight feature-gating, security hardening (OnceLock, zeroize, CredentialFilter).
- **V2 Focus:** Desktop app porting — Tauri sidecar bundling, Korean UI, native notifications.
- **Language:** Korean-first for CLI messages, help text, and system prompts.
- **Core Stack:** Rust (Edition 2024), Tokio (Async), Ratatui (TUI), Axum (Gateway/Webhooks), Tauri 2.0 (Desktop).

## Core Architecture
The project is a Rust workspace consisting of several specialized crates:

- **`naraeclaw` (Binary):** The main entry point located in `src/main.rs`.
- **`zeroclaw-runtime`:** Contains the core agent loop (`agent/`), security policies (`security/`), and scheduling (`cron/`, `sop/`).
- **`zeroclaw-channels`:** Handles integrations with messaging platforms (Telegram, Discord, Slack, etc.).
- **`zeroclaw-api`:** Defines common traits for providers, channels, and tools.
- **`zeroclaw-providers`:** Interface for various LLM backends (OpenRouter, Anthropic, OpenAI, etc.).
- **`zeroclaw-tools`:** Implementation of tools the agent can use (shell, browser, file I/O).
- **`zeroclaw-config`:** TOML-based configuration system with automatic schema generation.

## Development Guide

### Building and Running
- **Build:** `cargo build --release` (optimized for size via `opt-level = "z"`).
- **Quick Run:** `cargo run -- onboard` (Interactive setup).
- **Start Agent:** `cargo run -- agent`.
- **Start Daemon:** `cargo run -- daemon` (Gateway + Channels + Scheduler).

### Testing
- **All Tests:** `cargo test`.
- **Unit Tests:** `cargo test --lib`.
- **Functional Levels:**
  - `cargo test --test component`: Component-level validation.
  - `cargo test --test integration`: System integration tests.
  - `cargo test --test system`: Full end-to-end flows.
  - `cargo test --test live -- --ignored`: Real API calls (requires keys).

### Automation (`just`)
The project uses `just` for common tasks:
- `just fmt`: Format code.
- `just lint`: Run clippy with strict warnings.
- `just ci`: Run full quality gate (fmt + lint + test).
- `just dev <args>`: Run with development arguments.

## Development Conventions
- **Rust Edition:** Always use Rust 2024 features where appropriate.
- **Error Handling:** Use `anyhow` for application-level errors and `thiserror` for library crates.
- **Logging:** Use the `tracing` crate. Default level is `INFO`.
- **Config:** Config keys are considered a public contract. Document changes and provide migration paths.
- **Security:** High-risk areas like `zeroclaw-runtime/src/security/` and `zeroclaw-gateway/` require extra caution during modification.

## Localization Status
NaraeClaw is Korean-first. Current state:
1. **CLI Help:** Translated to Korean in `src/main.rs`.
2. **System Prompts:** `crates/zeroclaw-runtime/src/agent/system_prompt.rs` — Korean default prompt in place.
3. **Documentation:** `README.md`, `Plan.md`, `CLAUDE.md`, `docs/` are all in Korean.

## Key Files
- `Cargo.toml`: Workspace and feature management.
- `CLAUDE.md`: Claude-specific guidance and command reference.
- `Plan.md`: Current development roadmap and priorities.
- `src/main.rs`: CLI command definitions and routing.
- `crates/zeroclaw-runtime/src/agent/loop_.rs`: The heart of the agent's execution loop.
- `crates/zeroclaw-channels/src/telegram.rs`: Telegram webhook handler (Axum-based, polling removed).
