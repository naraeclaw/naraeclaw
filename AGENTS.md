# AGENTS.md — NaraeClaw

Cross-tool agent instructions for any AI coding assistant working on this repository.

## Project Identity

NaraeClaw is a Korean-first, lightweight autonomous agent runtime for server management and personal knowledge workflows. The CLI and gateway API are the primary surfaces (Desktop/Web removed 2026-05-05).

The repository is currently in fast cleanup and consolidation mode. Keep the CLI and gateway paths healthy, remove stale compatibility surfaces when they are no longer useful, and avoid growing new surface area without a concrete use case.

Internal crate and binary names use `naraeclaw-*` / `naraeclaw`. Legacy `ZEROCLAW_*` environment variables are retained only as compatibility fallbacks.

## Commands

```bash
# Format
cargo fmt --all -- --check

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Tests
cargo test -p <crate-name> --lib
cargo test -p <crate-name> <test-name>
cargo test -p <crate-name> --test <integration-test-name>

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
2. `naraeclaw-channels` transport layer
3. `naraeclaw-runtime/agent/` agent loop
4. `naraeclaw-providers` LLM call
5. `naraeclaw-tools` tool execution
6. Response delivery

Core architecture is trait-driven and modular. Extend by implementing traits and registering in factory modules.

Key extension points:

- `crates/naraeclaw-api/src/provider.rs` (`Provider`)
- `crates/naraeclaw-api/src/channel.rs` (`Channel`)
- `crates/naraeclaw-api/src/tool.rs` (`Tool`)
- `crates/naraeclaw-api/src/memory_traits.rs` (`Memory`)
- `crates/naraeclaw-api/src/observability_traits.rs` (`Observer`)
- `crates/naraeclaw-api/src/runtime_traits.rs` (`RuntimeAdapter`)

## Current Priorities

1. Keep the CLI/runtime stable for server management and personal knowledge workflows.
2. Continue desktop and web UX work without regressing the CLI path.
3. Remove stale compatibility assumptions in small, scoped passes.
4. Prefer fast validation on `master` until the project settles.

## Stability Tiers

Every workspace crate carries a stability tier per the Microkernel Architecture RFC.

| Crate | Tier | Notes |
|-------|------|-------|
| `naraeclaw-api` | Experimental | Stable at v1.0.0 (formal milestone) |
| `naraeclaw-config` | Beta | Stable at v0.8.0 |
| `naraeclaw-providers` | Beta | — |
| `naraeclaw-memory` | Beta | — |
| `naraeclaw-infra` | Beta | — |
| `naraeclaw-tool-call-parser` | Beta | Stable at v0.8.0 |
| `naraeclaw-channels` | Experimental | Messaging platform integrations |
| `naraeclaw-tools` | Experimental | Tool execution surface |
| `naraeclaw-runtime` | Experimental | Agent runtime (agent loop, security, cron, SOP, skills, observability) |
| `naraeclaw-gateway` | Experimental | Separate binary at v0.9.0 |
| `naraeclaw-tui` | Experimental | TUI onboarding wizard |
| `naraeclaw-macros` | Beta | Tightly coupled to config schema |

**Tiers**: Stable = covered by breaking-change policy. Beta = breaking changes permitted in MINOR with changelog notes. Experimental = no stability guarantee.

Tiers are promoted, never demoted, through deliberate team decision.

## Repository Map

- `src/main.rs` — CLI entrypoint and command routing
- `src/lib.rs` — module re-exports and CLI command enum definitions
- `crates/naraeclaw-api/` — public trait definitions (Provider, Channel, Tool, Memory, Observer)
- `crates/naraeclaw-config/` — schema, config loading/merging
- `crates/naraeclaw-macros/` — Configurable derive macro
- `crates/naraeclaw-providers/` — model providers and resilient wrapper
- `crates/naraeclaw-channels/` — messaging platform integrations kept behind explicit Cargo features
- `crates/naraeclaw-channels/src/orchestrator/` — channel lifecycle, routing, media pipeline
- `crates/naraeclaw-tools/` — tool execution surface (shell, file, memory, browser)
- `crates/naraeclaw-runtime/` — agent loop, security, cron, SOP, skills, onboarding wizard, observability
- `crates/naraeclaw-memory/` — memory backends (markdown, sqlite, embeddings, vector merge)
- `crates/naraeclaw-infra/` — shared infrastructure (debounce, session, stall watchdog)
- `crates/naraeclaw-gateway/` — webhook/gateway server (separate binary)
- `crates/naraeclaw-tui/` — TUI onboarding wizard
- `crates/naraeclaw-tool-call-parser/` — tool call parsing
- `docs/` — topic-based documentation (setup-guides, reference, ops, security, contributing, maintainers)
- `.github/` — CI, templates, automation workflows

## Architecture Notes

- `crates/naraeclaw-runtime/src/agent/` — agent loop core, including `loop_.rs` and `agent.rs`.
- `crates/naraeclaw-runtime/src/security/` — access control and policy. Treat as high risk.
- `crates/naraeclaw-runtime/src/cron/` — cron scheduler.
- `crates/naraeclaw-runtime/src/sop/` — SOP engine.
- `crates/naraeclaw-runtime/src/skills/` and `crates/naraeclaw-runtime/src/skillforge/` — skill system.
- `crates/naraeclaw-runtime/src/onboard/` — TUI onboarding wizard.
- `crates/naraeclaw-channels/` — channel integrations gated by `channel-<name>` Cargo features.
- `crates/naraeclaw-channels/src/orchestrator/` — channel lifecycle, routing, and media pipeline.
- `crates/naraeclaw-config/` — TOML-based config. `Configurable` derive generates schema. Config keys are public contract; document defaults and migration path when changing them.
- `tests/support/` — shared mocks such as `MockProvider`, `MockChannel`, and `EchoTool`.
- `tests/fixtures/traces/` — JSON fixture replay data for `TraceLlmProvider`.


## Risk Tiers

- **Low risk**: docs/chore/tests-only changes
- **Medium risk**: most `crates/*/src/**` behavior changes without boundary/security impact
- **High risk**: `crates/naraeclaw-runtime/src/**` (especially `src/security/`), `crates/naraeclaw-gateway/src/**`, `crates/naraeclaw-tools/src/**`, `.github/workflows/**`, access-control boundaries

When uncertain, classify as higher risk.

## Fast Development Mode

The project is currently in fast development mode while the core CLI, desktop, and web paths settle.

Default branch policy during this phase:

- Direct work on `master` is allowed for small, owner-approved changes.
- Agents may commit on `master` when the user explicitly asks to commit, merge, or push quickly.
- Pull requests are optional, not mandatory. Use PRs for larger changes, risky security/runtime work, or when the user asks for review.
- Keep commits small and reversible. Prefer one coherent change per commit even when skipping the PR queue.
- Run the fastest relevant validation before pushing. Do not block urgent iteration on the full historical CI matrix.

Suggested fast validation:

```bash
cargo fmt --all -- --check
cargo check --workspace
```

Use targeted tests when the change scope needs runtime coverage. Keep the historical full-suite checks out of the fast path until they are explicitly repaired. For docs-only changes, `git diff --check` is enough unless the edited docs have a dedicated checker.

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
- In fast development mode, direct commits and pushes to `master` are allowed when the user explicitly requests them or when the task is explicitly scoped for fast iteration.
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
