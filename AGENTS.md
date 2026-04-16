# AGENTS.md — NaraeClaw

Cross-tool agent instructions for any AI coding assistant working on this repository.

## Project Identity

NaraeClaw is a Korean-first, lightweight fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw).

V1 fork goals (all completed as of 2026-04-13):

- ✅ **Telegram webhook migration** — replaced polling with Axum-based webhook handling.
- ✅ **Lightweight defaults** — removed unnecessary channels from default Cargo features.
- ✅ **Security hardening** — eliminated `unsafe set_var`, introduced `OnceLock`, `zeroize`, and `CredentialFilter`.

Current focus: V2 desktop app porting (see `Plan.md`).

Internal crate names still use `zeroclaw-*`, but the binary name is `naraeclaw`.

## Commands

```bash
# Format
cargo fmt --all
cargo fmt --all -- --check

# Lint
cargo clippy --all-targets -- -D warnings

# Tests
cargo test
cargo test --lib
cargo test --test component
cargo test --test integration
cargo test --test system
cargo test --test integration <test-name>

# Live tests require real API keys and are marked #[ignore]
cargo test --test live -- --ignored

# Development mode
cargo run -- onboard
cargo run -- agent

# Justfile shortcuts, if just is installed
just ci
just fmt
just dev
```

Full pre-PR validation (recommended):

```bash
./dev/ci.sh all
```

Docs-only changes: run markdown lint and link-integrity checks. If touching bootstrap scripts: `bash -n install.sh`.

## Project Snapshot

NaraeClaw is a Rust edition 2024 autonomous agent runtime optimized for Korean-first operation, performance, efficiency, stability, extensibility, sustainability, and security.

The `naraeclaw` binary (`src/main.rs`) enables the `agent-runtime` feature by default, which activates most agent subsystems.

Message flow:

1. Incoming message
2. `zeroclaw-channels` transport layer
3. `zeroclaw-runtime/agent/` agent loop
4. `zeroclaw-providers` LLM call
5. `zeroclaw-tools` tool execution
6. Response delivery

Core architecture is trait-driven and modular. Extend by implementing traits and registering in factory modules.

Key extension points:

- `crates/zeroclaw-api/src/provider.rs` (`Provider`)
- `crates/zeroclaw-api/src/channel.rs` (`Channel`)
- `crates/zeroclaw-api/src/tool.rs` (`Tool`)
- `crates/zeroclaw-api/src/memory_traits.rs` (`Memory`)
- `crates/zeroclaw-api/src/observability_traits.rs` (`Observer`)
- `crates/zeroclaw-api/src/runtime_traits.rs` (`RuntimeAdapter`)

## Current Priorities

1. Convert Telegram from polling to webhooks.
2. Remove unnecessary channels from default Cargo features.
3. Localize the default system prompt in `crates/zeroclaw-runtime/src/agent/system_prompt.rs`.
4. Localize CLI help text.

## Stability Tiers

Every workspace crate carries a stability tier per the Microkernel Architecture RFC.

| Crate | Tier | Notes |
|-------|------|-------|
| `zeroclaw-api` | Experimental | Stable at v1.0.0 (formal milestone) |
| `zeroclaw-config` | Beta | Stable at v0.8.0 |
| `zeroclaw-providers` | Beta | — |
| `zeroclaw-memory` | Beta | — |
| `zeroclaw-infra` | Beta | — |
| `zeroclaw-tool-call-parser` | Beta | Stable at v0.8.0 |
| `zeroclaw-channels` | Experimental | Plugin migration at v1.0.0 |
| `zeroclaw-tools` | Experimental | Plugin migration at v1.0.0 |
| `zeroclaw-runtime` | Experimental | Agent runtime (agent loop, security, cron, SOP, skills, observability) |
| `zeroclaw-gateway` | Experimental | Separate binary at v0.9.0 |
| `zeroclaw-tui` | Experimental | TUI onboarding wizard |
| `zeroclaw-plugins` | Experimental | WASM plugin system — foundation for v1.0.0 plugin ecosystem |
| `zeroclaw-macros` | Beta | Tightly coupled to config schema |

**Tiers**: Stable = covered by breaking-change policy. Beta = breaking changes permitted in MINOR with changelog notes. Experimental = no stability guarantee.

Tiers are promoted, never demoted, through deliberate team decision.

## Repository Map

- `src/main.rs` — CLI entrypoint and command routing
- `src/lib.rs` — module re-exports and CLI command enum definitions
- `crates/zeroclaw-api/` — public trait definitions (Provider, Channel, Tool, Memory, Observer, Peripheral)
- `crates/zeroclaw-config/` — schema, config loading/merging
- `crates/zeroclaw-macros/` — Configurable derive macro
- `crates/zeroclaw-providers/` — model providers and resilient wrapper
- `crates/zeroclaw-channels/` — messaging platform integrations (30+ channels)
- `crates/zeroclaw-channels/src/orchestrator/` — channel lifecycle, routing, media pipeline
- `crates/zeroclaw-tools/` — tool execution surface (shell, file, memory, browser)
- `crates/zeroclaw-runtime/` — agent loop, security, cron, SOP, skills, onboarding wizard, observability
- `crates/zeroclaw-memory/` — memory backends (markdown, sqlite, embeddings, vector merge)
- `crates/zeroclaw-infra/` — shared infrastructure (debounce, session, stall watchdog)
- `crates/zeroclaw-gateway/` — webhook/gateway server (separate binary)
- `crates/zeroclaw-tui/` — TUI onboarding wizard
- `crates/zeroclaw-plugins/` — WASM plugin system
- `crates/zeroclaw-tool-call-parser/` — tool call parsing
- `docs/` — topic-based documentation (setup-guides, reference, ops, security, contributing, maintainers)
- `.github/` — CI, templates, automation workflows

## Architecture Notes

- `crates/zeroclaw-runtime/src/agent/` — agent loop core, including `loop_.rs` and `agent.rs`.
- `crates/zeroclaw-runtime/src/security/` — access control and policy. Treat as high risk.
- `crates/zeroclaw-runtime/src/cron/` — cron scheduler.
- `crates/zeroclaw-runtime/src/sop/` — SOP engine.
- `crates/zeroclaw-runtime/src/skills/` and `crates/zeroclaw-runtime/src/skillforge/` — skill system.
- `crates/zeroclaw-runtime/src/onboard/` — TUI onboarding wizard.
- `crates/zeroclaw-channels/` — channel integrations gated by `channel-<name>` Cargo features.
- `crates/zeroclaw-channels/src/orchestrator/` — channel lifecycle, routing, and media pipeline.
- `crates/zeroclaw-config/` — TOML-based config. `Configurable` derive generates schema. Config keys are public contract; document defaults and migration path when changing them.
- `tests/support/` — shared mocks such as `MockProvider`, `MockChannel`, and `EchoTool`.
- `tests/fixtures/traces/` — JSON fixture replay data for `TraceLlmProvider`.

Telegram latency-related code:

- `crates/zeroclaw-channels/src/telegram.rs:2871` — polling timeout of 30 seconds; target for webhook replacement.
- `crates/zeroclaw-channels/src/telegram.rs:372` — draft update interval of 1000 ms.
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:1844` — memory recall query performed on every message.

## Risk Tiers

- **Low risk**: docs/chore/tests-only changes
- **Medium risk**: most `crates/*/src/**` behavior changes without boundary/security impact
- **High risk**: `crates/zeroclaw-runtime/src/**` (especially `src/security/`), `crates/zeroclaw-gateway/src/**`, `crates/zeroclaw-tools/src/**`, `.github/workflows/**`, access-control boundaries

When uncertain, classify as higher risk.

## Fast Development Mode

The project is currently in fast development mode until the desktop app and Korean-first runtime settle.

Default branch policy during this phase:

- Direct work on `master` is allowed for small, owner-approved changes.
- Agents may commit on `master` when the user explicitly asks to commit, merge, or push quickly.
- Pull requests are optional, not mandatory. Use PRs for larger changes, risky security/runtime work, or when the user asks for review.
- Keep commits small and reversible. Prefer one coherent change per commit even when skipping the PR queue.
- Run the fastest relevant validation before pushing. Do not block urgent iteration on the full historical CI matrix.

Suggested fast validation:

```bash
cargo fmt --all -- --check
cargo check --workspace --exclude zeroclaw-desktop
```

Use targeted tests when the change scope needs runtime coverage. The historical full lib-test suite still has known cleanup debt, so do not make fast CI depend on it until those tests are repaired. For docs-only changes, `git diff --check` is enough unless the edited docs have a dedicated checker.

## Worktree Guidance

Worktrees are optional coordination tools, not a hard requirement.

Use a dedicated worktree when:

- multiple agents are editing in parallel;
- a change is large or risky;
- the user asks for isolated review before merge;
- files overlap with active work in the main checkout.

For isolated worktrees:

```bash
git worktree add ../naraeclaw-<owner> -b <owner>/<task-name>
# Example: git worktree add ../naraeclaw-codex -b codex/telegram-webhook
```

Parallel work rules:

- Design task slices so different agents do not edit the same files.
- If overlapping files are unavoidable, serialize the work: merge the first task before starting the next.
- Ignore unrelated untracked or modified files in another worktree unless the user explicitly asks to handle them.

## Workflow

1. **Read before write** — inspect existing module, factory wiring, and adjacent tests before editing.
2. **One concern per PR** — avoid mixed feature+refactor+infra patches.
3. **Implement minimal patch** — no speculative abstractions, no config keys without a concrete use case.
4. **Validate by risk tier** — docs-only: lightweight checks. Code changes: full relevant checks.
5. **Document impact** — update PR notes for behavior, risk, side effects, and rollback.
6. **Queue hygiene** — stacked PR: declare `Depends on #...`. Replacing old PR: declare `Supersedes #...`.

Branch/commit/PR rules:
- In fast development mode, direct commits and pushes to `master` are allowed when the user explicitly requests them.
- Use short-lived branches or worktrees for large, risky, or parallel-agent work.
- Use conventional commit titles. Keep commits small and easy to revert.
- PRs are optional for fast iteration; when opening one, follow `.github/pull_request_template.md`.
- Never commit secrets, personal data, or real identity information (see `@docs/contributing/pr-discipline.md`).

## Anti-Patterns

- Do not add heavy dependencies for minor convenience.
- Do not silently weaken security policy or access constraints.
- Do not add speculative config/feature flags "just in case".
- Do not mix massive formatting-only changes with functional changes.
- Do not modify unrelated modules "while here".
- Do not bypass failing checks without explicit explanation.
- Do not hide behavior-changing side effects in refactor commits.
- Do not include personal identity or sensitive information in test data, examples, docs, or commits.

## Linked References

- `@docs/contributing/change-playbooks.md` — adding providers, channels, tools; security/gateway changes; architecture boundaries
- `@docs/contributing/pr-discipline.md` — privacy rules, superseded-PR attribution/templates, handoff template
- `@docs/contributing/docs-contract.md` — docs system contract, i18n rules, locale parity
