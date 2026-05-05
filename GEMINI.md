# GEMINI.md - NaraeClaw Project Context

## Project Overview
**NaraeClaw (나래클로)** is a lightweight, Korean-first AI agent runtime for macOS, Windows, and Linux. It is a fork derived from ZeroClaw, focusing on low latency, in-memory security, and a streamlined developer experience.

- **Primary Goal:** Provide a fast, responsive assistant with a Korean-first UX across CLI, Desktop, and Web surfaces.
- **Key Optimization:** Modular configuration, stable service management, and gateway/webhook support where needed.
- **Security Mandate:** Unified credential filtering (`CredentialFilter`), in-memory zeroization (`Zeroize`), and thread-safe environment management.
- **Core Stack:** Rust (Edition 2024), Tokio (Async), Axum (Gateway/Webhooks), ChaCha20-Poly1305 (Encryption).

## Core Architecture
The project is a modular Rust workspace, recently refactored for maximum maintainability:

- **`naraeclaw` (Binary):** CLI entry point in `src/main.rs`.
- **`naraeclaw-runtime`:** The core engine.
  - `agent/`: Main execution loop with anti-narration and integrated scrubbing.
  - `security/`: **CredentialFilter** (unified leak detection), **SecretStore** (AEAD), and sandbox.
- **`naraeclaw-channels`:** Messaging and webhook integrations.
  - Uses a **Feature-gate system** to exclude non-core channels, minimizing binary size.
  - Keep core channels and remove legacy surface area aggressively.
- **`naraeclaw-config`:** Highly modularized configuration system.
  - **`schema/` Directory:** Split into `mod.rs`, `config_types.rs`, `channels.rs`, `providers.rs`, `security.rs`, `tools.rs`, and `automation.rs`.
  - **Security Macros**: Automatically implements `Zeroize` on Drop for `#[secret]` fields.
- **`naraeclaw-providers`:** LLM backend factory with centralized environment key management.

## Development & Security Guide

### Building and Running
- **Lightweight (Default):** `cargo build --release`
- **Full Build:** `cargo build --release --features channels-full`
- **Onboarding:** `cargo run -- onboard` (Korean-first UI).

### Security Implementation
- **Unified Filtering:** All outbound messages pass through `CredentialFilter` (handles Base64, Hex, URL-encoding, and streaming chunks).
- **Memory Safety:** Every sensitive field (API keys, tokens) is wiped from memory on Drop via the `Zeroize` trait.
- **Env Var Safety:** Serialized access to environment variables via `OnceLock` and `Mutex` to prevent UB in async contexts.

### Cleanup & Pruning (Completed)
- **Dead Code Removal:** Removed legacy providers (`glm.rs`) and experimental configs (`ConversationalAi`, `ProjectIntel`, etc.).
- **Resource Optimization:** Only Korean (`ko`) and English (`en`) docs/descriptions are retained.

## Key Files & Modules
- `src/main.rs`: CLI definitions and thread-safe early initialization.
- `crates/naraeclaw-runtime/src/security/leak_detector.rs`: The **CredentialFilter** engine.
- `crates/naraeclaw-runtime/src/service/mod.rs`: Cross-platform service management.
- `crates/naraeclaw-config/src/schema/mod.rs`: Entry point for the modularized config system.
- `crates/naraeclaw-gateway/src/`: HTTP/WebSocket gateway (port 42617).
- `Plan.md`: Historical record of architectural evolution and future roadmap.
