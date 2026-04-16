# GEMINI.md - NaraeClaw Project Context

## Project Overview
**NaraeClaw (나래클로)** is a lightweight, Korean-first AI agent runtime optimized for messaging platforms like Telegram. It is a highly optimized fork derived from ZeroClaw, focusing on low latency, in-memory security, and a streamlined developer experience.

- **Primary Goal:** To provide a fast, responsive AI assistant with a "Korean-first" experience.
- **Key Optimization:** High-performance **Telegram Webhooks** (Axum-based) and a modularized configuration system.
- **Security Mandate:** Unified credential filtering (`CredentialFilter`), in-memory zeroization (`Zeroize`), and thread-safe environment management.
- **Core Stack:** Rust (Edition 2024), Tokio (Async), Axum (Gateway/Webhooks), ChaCha20-Poly1305 (Encryption).

## Core Architecture
The project is a modular Rust workspace, recently refactored for maximum maintainability:

- **`naraeclaw` (Binary):** CLI entry point in `src/main.rs`.
- **`zeroclaw-runtime`:** The core engine.
  - `agent/`: Main execution loop with anti-narration and integrated scrubbing.
  - `security/`: **CredentialFilter** (unified leak detection), **SecretStore** (AEAD), and sandbox.
- **`zeroclaw-channels`:** Platform integrations.
  - Optimized for **Telegram Webhooks**.
  - Uses a **Feature-gate system** to exclude non-core channels, minimizing binary size.
- **`zeroclaw-config`:** Highly modularized configuration system.
  - **`schema/` Directory:** Formerly a single 17k line file, now split into `mod.rs`, `config_types.rs`, `channels.rs`, `providers.rs`, `security.rs`, `tools.rs`, and `automation.rs`.
  - **Security Macros**: Automatically implements `Zeroize` on Drop for `#[secret]` fields.
- **`zeroclaw-providers`:** LLM backend factory with centralized environment key management.

## Development & Security Guide

### Building and Running
- **Lightweight (Default):** `cargo build --release`
- **Full Build:** `cargo build --release --features channels-full`
- **Onboarding:** `cargo run -- onboard` (Full Korean UI).

### Security Implementation
- **Unified Filtering:** All outbound messages pass through `CredentialFilter` (handles Base64, Hex, URL-encoding, and streaming chunks).
- **Memory Safety:** Every sensitive field (API keys, tokens) is wiped from memory on Drop via the `Zeroize` trait.
- **Env Var Safety:** Serialized access to environment variables via `OnceLock` and `Mutex` to prevent UB in async contexts.

### Cleanup & Pruning (Completed)
- **Dead Code Removal:** Removed legacy providers (`glm.rs`) and experimental configs (`ConversationalAi`, `ProjectIntel`, etc.).
- **Resource Optimization:** Only Korean (`ko`) and English (`en`) docs/descriptions are retained.

## Key Files & Modules
- `src/main.rs`: CLI definitions and thread-safe early initialization.
- `crates/zeroclaw-runtime/src/security/leak_detector.rs`: The **CredentialFilter** engine.
- `crates/zeroclaw-config/src/schema/mod.rs`: Entry point for the modularized config system.
- `crates/zeroclaw-channels/src/telegram.rs`: Telegram Webhook implementation.
- `Plan.md`: Historical record of architectural evolution and future roadmap.
