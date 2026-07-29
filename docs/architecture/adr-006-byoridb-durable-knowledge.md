# ADR-006: ByoriDB as the Sole Durable Knowledge Surface

**Status:** Accepted

**Date:** 2026-07-29

**Related modules:** `crates/naraeclaw-config`, `crates/naraeclaw-tools`,
`crates/naraeclaw-runtime`, `src/knowledge`, `crates/naraeclaw-memory`

## Context

NaraeClaw previously exposed Markdown, SQLite, Qdrant, and related embedding paths as
interchangeable long-term memory backends. That created two problems: users could end up
with competing sources of truth, and relationship-rich project knowledge was flattened
into standalone entries.

ByoriDB provides a local typed graph, temporal history, provenance, a safe MCP profile,
and an agent skill that defines when facts and relationships should be recalled or stored.
The project needs one predictable durable-knowledge contract while retaining a safe way to
import and roll back old data.

## Decision

1. `[knowledge]` is enabled by default with `provider = "byoridb"`. No other durable
   knowledge provider is supported.
2. NaraeClaw injects a managed local `byoridb` MCP server. It sets a workspace-derived or
   explicit `BYORIDB_MEMORY_SPACE` and forces `BYORIDB_MCP_PROFILE=safe`.
3. The bundled `byoridb-memory` skill is exposed only when its core callable tools are
   present. The agent uses ByoriDB for durable facts, decisions, modules, incidents, and
   relationships.
4. When ByoriDB knowledge is active, the legacy `Memory` runtime handle is a no-op and
   legacy memory tools are not exposed. NaraeClaw must not write two durable stores in
   parallel.
5. `crates/naraeclaw-memory` remains for migration, source reading, and explicit rollback
   compatibility. It is not an extension point for a second durable knowledge provider.
6. `naraeclaw knowledge migrate` previews, snapshots, and imports eligible legacy data.
   Conversation entries are always excluded; daily entries are opt-in; Qdrant requires a
   separate export/import plan.
7. Migration keeps the original sources and an auditable manifest. Rollback temporarily
   disables `[knowledge]` and restores one legacy backend; it never runs beside ByoriDB.

## Isolation and Security Contract

- The default space is derived from the canonical workspace path. An explicit space is
  required when a moving workspace must keep the same identity.
- A ByoriDB space is a logical namespace, not an authorization boundary. Separate trust
  domains require separate ByoriDB instances and credentials.
- The safe MCP profile hides unrestricted `memory_query`; only the guarded read-query tool
  is exposed.
- Relationship mutations and deletion are not auto-approved in supervised mode.
- Secrets, credentials, tokens, and personal sensitive data must not be stored as
  knowledge. Recalled content can be sent to the configured model provider.

## Consequences

Benefits:

- one durable source of truth per NaraeClaw workspace;
- typed relationships and temporal history instead of flat memory only;
- consistent skill, prompt, CLI status, gateway status, and migration behavior;
- deterministic isolation and a least-privilege query surface.

Costs and current limitations:

- ByoriDB and its MCP wrapper are an external local dependency;
- Byori is an early, local single-node system, so important data still needs an
  independent backup;
- the current stock container image does not bundle the Byori MCP wrapper or Python
  runtime, so durable knowledge is unavailable there until a Byori-capable image or
  supported sidecar contract is provided;
- SOP audit entries and ADR-005 skill-evolution index/stat/candidate records still use the
  legacy `Memory` interface and are therefore not durable in default ByoriDB mode;
- moving a workspace without an explicit `space` selects a different graph.

“Sole durable knowledge surface” means sole within the NaraeClaw runtime. It does not mean
that ByoriDB should be the only backup copy of important information.

## Alternatives Considered

### Keep multiple active memory backends

Rejected because concurrent writes and retrieval would preserve ambiguity about the source
of truth and make migrations hard to verify.

### Add a native Rust ByoriDB backend behind `Memory`

Rejected for now because it would flatten the typed graph into the legacy memory contract
and duplicate the maintained MCP adapter.

### Require users to configure a generic MCP entry manually

Rejected because server name, safe profile, and workspace space could drift between status,
migration, and agent execution. NaraeClaw owns the managed entry instead.

## Operational Contract

Before enabling the provider in an existing workspace:

1. install and health-check ByoriDB;
2. run `naraeclaw knowledge migrate --dry-run`;
3. apply with `--yes` only after reviewing paths and counts;
4. verify `naraeclaw knowledge status` and one representative read;
5. keep the migration snapshot and original sources until the imported graph is verified.

See [ByoriDB Durable Knowledge](../setup-guides/byoridb-knowledge.md) for the full procedure
and [Config Reference](../reference/api/config-reference.md#knowledge) for defaults.
