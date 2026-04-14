# GEMINI.md - NaraeClaw Project Context

## Project Overview
**NaraeClaw (나래클로)** is a lightweight, Korean-first AI agent runtime optimized for messaging platforms like Telegram. It is a fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw), optimized for low latency, memory safety, and Korean language environment.

- **Primary Goal:** To provide a fast, responsive AI assistant with a "Korean-first" experience.
- **Key Optimization:** High-performance **Telegram Webhooks** (Axum-based) to eliminate polling latency.
- **Security Mandate:** Multi-layered protection including unified credential filtering and in-memory zeroization.
- **Core Stack:** Rust (Edition 2024), Tokio (Async), Ratatui (TUI), Axum (Gateway/Webhooks), ChaCha20-Poly1305 (Encryption).

## Core Architecture
The project is a modular Rust workspace:

- **`naraeclaw` (Binary):** CLI entry point in `src/main.rs`. Handles startup, logging init, and command routing.
- **`zeroclaw-runtime`:** The core engine.
  - `agent/`: Main execution loop with anti-narration logic and tool-call handling.
  - `security/`: **CredentialFilter** (unified leak detection), **SecretStore** (AEAD encryption), and sandbox policies.
- **`zeroclaw-channels`:** Platform integrations.
  - Optimized for **Telegram Webhooks**.
  - Uses a **Feature-gate system** to exclude 24+ non-core channels by default, minimizing binary size.
- **`zeroclaw-config`:** Robust configuration system.
  - **`#[secret]` macros**: Automatically implements `Zeroize` on Drop for sensitive fields.
  - **OnceLock/Mutex**: Thread-safe environment variable management for configuration overrides.
- **`zeroclaw-providers`:** LLM backend factory.
  - Centralized constants for API keys and base URLs for easy override.

## Development & Security Guide

### Building and Running
- **Default (Lightweight):** `cargo build --release` (only core channels like Telegram/Slack).
- **Full Build:** `cargo build --release --features channels-full` (includes all 30+ channels).
- **Setup:** `cargo run -- onboard` (Interactive Korean wizard).
- **Run:** `cargo run -- agent` or `cargo run -- daemon` (for webhooks).

### Security Implementation
- **Unified Filtering:** All outbound messages pass through `CredentialFilter` which detects raw, Base64, Hex, and URL-encoded secrets, even across streaming chunk boundaries.
- **Memory Safety:** `Zeroize` is enforced on all secret fields (API keys, tokens) to prevent data recovery from memory dumps.
- **Env Var Safety:** Production code avoids `unsafe { set_var }` during runtime, using serialized access or early-init patterns.

### Testing
- **Security Check:** `cargo test -p zeroclaw-runtime security::leak_detector`
- **Full Gate:** `just ci` (runs fmt, clippy, and all tests).

## Localization Status (100% Complete)
1. **CLI Help:** Fully translated to Korean in `src/main.rs`.
2. **System Prompts:** Optimized for Korean natural language, including anti-narration and hardware control instructions.
3. **User Docs:** `README.md` and `Plan.md` are maintained in Korean.

## Key Files & Modules
- `src/main.rs`: CLI definitions and early-init security.
- `crates/zeroclaw-runtime/src/security/leak_detector.rs`: The **CredentialFilter** engine.
- `crates/zeroclaw-runtime/src/agent/loop_.rs`: Core agent loop with integrated scrub logic.
- `crates/zeroclaw-channels/src/telegram.rs`: Telegram Webhook and polling implementations.
- `crates/zeroclaw-config/src/schema.rs`: Main configuration schema with security macros.
- `crates/zeroclaw-providers/src/env_keys.rs`: Centralized constants for environment variable keys.
- `Plan.md`: Detailed history of architectural improvements and future roadmap.
