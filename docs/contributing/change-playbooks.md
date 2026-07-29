# Change Playbooks

Step-by-step guides for common extension and modification patterns in NaraeClaw.

For complete code examples of each extension trait, see [extension-examples.md](./extension-examples.md).

## Adding a Provider

- Implement `Provider` in `crates/naraeclaw-providers/src/`.
- Register in the factory in `crates/naraeclaw-providers/src/lib.rs`.
- Add focused tests for factory wiring and error paths.
- Avoid provider-specific behavior leaks into shared orchestration code.

## Adding a Channel

- Implement `Channel` in `crates/naraeclaw-channels/src/` and add its public schema in
  `crates/naraeclaw-config/src/schema/channels.rs`.
- Keep `send`, `listen`, `health_check`, typing semantics consistent.
- Cover auth/allowlist/health behavior with tests.

## Adding a Tool

- Implement `Tool` in `crates/naraeclaw-tools/src/` with a strict parameter schema and
  wire it through `crates/naraeclaw-runtime/src/tools/mod.rs`.
- Validate and sanitize all inputs.
- Return structured `ToolResult`; avoid panics in runtime path.

## Changing Durable Knowledge

- ByoriDB is the sole durable knowledge provider; do not add a provider to the legacy
  `Memory` factory.
- Change the managed MCP adapter, safe-profile policy, or bundled `byoridb-memory` skill.
- Preserve workspace isolation and migration/rollback behavior.
- Treat relationship mutation, deletion, prompt injection, and gateway exposure as
  security-sensitive boundaries.
- Update ADR-006 and the ByoriDB setup guide when the contract changes.

## Security / Runtime / Gateway Changes

- Include threat/risk notes and rollback strategy.
- Add/update tests or validation evidence for failure modes and boundaries.
- Keep observability useful but non-sensitive.
- For `.github/workflows/**` changes, keep `.github/workflows/README.md` aligned with the active workflow set.

## Docs System / README / IA Changes

- Treat docs navigation as product UX: preserve clear pathing from README -> docs hub -> SUMMARY -> category index.
- Keep top-level nav concise; avoid duplicative links across adjacent nav blocks.
- When runtime surfaces change, update related references in `docs/reference/`.
- Keep English and Korean entry-point docs aligned when nav or key wording changes.

## Tool Shared State

- Follow the `Arc<RwLock<T>>` handle pattern for any tool that owns long-lived shared state.
- Accept handles at construction; do not create global/static mutable state.
- Use `ClientId` (provided by the daemon) to namespace per-client state — never construct identity keys inside the tool.
- Isolate security-sensitive state (credentials, quotas) per client; broadcast/display state may be shared with optional namespace prefixing.
- Cached validation is invalidated on config change — tools must re-validate before the next execution when signaled.
- See [ADR-004: Tool Shared State Ownership](../architecture/adr-004-tool-shared-state-ownership.md) for the full contract.

## Architecture Boundary Rules

- Extend capabilities by adding trait implementations + factory wiring first; avoid cross-module rewrites for isolated features.
- Keep dependency direction inward to contracts: concrete integrations depend on trait/config/util layers, not on other concrete integrations.
- Avoid cross-subsystem coupling (e.g., provider code importing channel internals, tool code mutating gateway policy directly).
- Keep module responsibilities single-purpose: orchestration in `agent/`, transport in `channels/`, model I/O in `providers/`, policy in `security/`, execution in `tools/`.
- Introduce new shared abstractions only after repeated use (rule-of-three), with at least one real caller.
- For config/schema changes, treat keys as public contract: document defaults, compatibility impact, and migration/rollback path.
