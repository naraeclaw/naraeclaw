# ByoriDB Durable Knowledge

Last verified: **July 29, 2026**.

ByoriDB is NaraeClaw's default durable knowledge store. It keeps facts, decisions,
projects, modules, and their relationships in a workspace-scoped graph. The legacy
`[memory]` backends are disabled by default and remain available only for compatibility
and migration.

> **Data-safety note:** “Sole” means NaraeClaw does not write a second durable backend in
> parallel. Byori is currently an early local single-node system; keep an independent
> backup or export for important knowledge. Do not store passwords, API keys, tokens, or
> other secrets. Recalled content may be sent to the configured model provider.

## Install ByoriDB

NaraeClaw does not download or modify ByoriDB automatically. Install the server, MCP
wrapper, and service through Byori's supported installer:

```bash
curl -fsSL https://github.com/byoridb/byori/releases/latest/download/install.sh | bash
```

The installer supports macOS and Linux and places the managed MCP wrapper at
`~/.byoridb/bin/run-mcp.sh`. For a local Byori source checkout, use the same installer
with local assets:

```bash
~/opensource/byori/install.sh --assets ~/opensource/byori
```

Verify the server before starting NaraeClaw:

```bash
curl -fsS http://127.0.0.1:19669/health
test -x ~/.byoridb/bin/run-mcp.sh
```

See the [Byori installation guide](https://github.com/byoridb/byori/blob/main/docs/install.md)
for supported platforms, service management, ports, and installer options.

## Configure NaraeClaw

These are the default values:

```toml
[knowledge]
enabled = true
provider = "byoridb"
byoridb_home = "~/.byoridb"
# space = "my_workspace"
required = false
```

| Key | Behavior |
|---|---|
| `enabled` | Enables ByoriDB as the sole durable knowledge surface. |
| `provider` | Must be `byoridb`. |
| `byoridb_home` | Installation root containing `bin/run-mcp.sh`. |
| `space` | Optional explicit graph space. If omitted, NaraeClaw derives one from the workspace path. |
| `required` | When `true`, configuration validation fails if the MCP wrapper is missing. |

With `required = false`, onboarding can finish before ByoriDB is installed. The durable
knowledge tools are not usable until the wrapper and server are available. Set
`required = true` after installation when startup should fail closed on a missing wrapper.

### Workspace isolation

When `space` is unset, NaraeClaw canonicalizes `workspace_dir`, hashes that path, and uses
an identifier of the form `naraeclaw_<24-hex-characters>`. Different workspace paths
therefore use different ByoriDB spaces automatically.

An explicit space must match `[A-Za-z_][A-Za-z0-9_]{0,63}`. Set one when a workspace may
move but should continue using the same graph. Reusing an explicit space across unrelated
workspaces intentionally removes their isolation.

A space is a logical namespace, not an authorization boundary. Run separate ByoriDB
instances with separate credentials for workspaces that belong to different trust domains.

### Safe MCP profile

NaraeClaw starts the managed server with both of these environment contracts:

- `BYORIDB_MEMORY_SPACE` is the explicit or workspace-derived space.
- `BYORIDB_MCP_PROFILE=safe` hides unrestricted `memory_query` and exposes the guarded
  read-only query surface instead.

`memory_query_read` accepts one read statement beginning with `MATCH`, `FETCH`, `GO`,
`LOOKUP`, `SHOW`, or `WHY`; it rejects mutations, comments, pipelines, and multiple
statements.

Tools appear with the managed server prefix, for example
`byoridb__memory_read`, `byoridb__memory_remember`,
`byoridb__memory_wiki_upsert`, `byoridb__memory_link`,
and `byoridb__memory_query_read`. The compatibility-only note lookup is
`byoridb__memory_recall`. NaraeClaw injects this
managed MCP entry automatically; a second manual `[[mcp.servers]]` entry is not required.
If a configured MCP server is already named `byoridb` (case-insensitive), the managed
local stdio definition replaces it at runtime so status, migration, and agent tools cannot
silently point at different databases. Use another server name for a custom connection.
`byoridb__memory_link` and destructive `byoridb__memory_delete` are not auto-approved.
They require explicit approval in supervised interactive use and are denied on
non-interactive supervised surfaces. Optional OTP policy is disabled by default.

Check the effective installation and space with:

```bash
naraeclaw knowledge status
```

## Container Limitation

The current stock NaraeClaw container image does not include Python or the Byori MCP
wrapper. Mounting `~/.byoridb` alone is therefore insufficient, and durable knowledge is
unavailable in that image even though `[knowledge]` defaults to enabled. Use the native
macOS/Linux installation for the supported ByoriDB path. A Byori-capable image or supported
sidecar transport must be defined before relying on durable knowledge in containers.

`knowledge.required = false` lets the container start without the wrapper; it does not
provide a fallback durable store. Confirm every deployment with
`naraeclaw knowledge status`.

## Migrate Legacy Knowledge

Migration reads legacy stores from the active `config.workspace_dir` and paths retained
from older configuration. There is no source-path flag. Inspect the plan first:

For a safe cutover, `[knowledge].enabled` may remain `false` during both preview and apply;
the migration command still targets the configured/derived ByoriDB space. Enable ByoriDB
only after the migration manifest reports success, then probe it with
`naraeclaw knowledge status`.

When an existing `[knowledge]` table has no Byori-specific key (`provider`, `byoridb_home`,
`space`, or `required`), NaraeClaw treats it as legacy, keeps ByoriDB disabled in memory,
and preserves the table on unrelated config saves. This includes old tables containing
only `enabled` or no explicit fields, whose SQLite path and limits previously came from
defaults. The guard prevents an old `enabled = true` flag from becoming an implicit
ByoriDB cutover. An existing config with no explicit Byori knowledge keys is handled the
same way. When its `[memory]` table or a changed memory-default field is absent, NaraeClaw
temporarily restores the pre-Byori SQLite, auto-save, hygiene, and hydration defaults;
explicit memory values remain authoritative. Migration is intentionally allowed in this
compatibility state: import and verify first, then replace the legacy table explicitly.

```bash
naraeclaw knowledge migrate --dry-run
```

Legacy Qdrant memory is not imported by this command. If the effective legacy memory
backend is `qdrant`, both dry-run and apply stop with an explicit error instead of
reporting zero records. Keep `[knowledge].enabled = false` so the Qdrant backend remains
available, export that collection separately, and only enable ByoriDB after planning a
separate import.

The default plan includes:

- core entries from `MEMORY.md`;
- core and custom entries from `brain.db` (conversation entries are always excluded);
- core entries from `MEMORY_SNAPSHOT.md` only as a recovery fallback when
  `memory/brain.db` is absent. When both exist, `brain.db` is authoritative and the
  snapshot is neither counted nor imported;
- all nodes and edges from the legacy knowledge graph database.

Daily Markdown files and daily `brain.db` entries are opt-in:

```bash
naraeclaw knowledge migrate --dry-run --include-daily
```

Apply only after reviewing the reported source paths and counts:

```bash
naraeclaw knowledge migrate --yes
# Or include daily knowledge explicitly:
naraeclaw knowledge migrate --yes --include-daily
```

### OpenClaw workspace staging

The separate `naraeclaw migrate openclaw` command is legacy-only: it stages an external
OpenClaw workspace into a NaraeClaw `[memory]` backend. `--dry-run` works in ByoriDB mode,
but apply fails before writing unless knowledge is disabled and a persistent staging backend
such as `sqlite` is selected. After staging, run the ByoriDB migration above, enable
knowledge, restore `[memory].backend = "none"`, and verify with
`naraeclaw knowledge status`.

A real migration requires `--yes`. Before writing to ByoriDB, it creates a rollback
snapshot under:

```text
<workspace>/migrations/byori-<UTC-timestamp>-<uuid>/
```

The snapshot contains:

- `config.toml`;
- `markdown/MEMORY.md`, fallback `markdown/MEMORY_SNAPSHOT.md`, and, when requested,
  `markdown/daily/*.md` when those sources are eligible;
- online backups at `sqlite/brain.db` and `sqlite/knowledge-<n>.db`;
- `manifest.json` with source paths, inventory, result counts, and status.

Migration never deletes or rewrites the source files or configuration. A failed run retains
the snapshot, marks the manifest as failed, and can be retried: ByoriDB imports use exact
reads and idempotent upserts.

After a successful import, replace the legacy `[knowledge]` keys with the new provider
settings and disable legacy writes:

```toml
[knowledge]
enabled = true
provider = "byoridb"
byoridb_home = "~/.byoridb"
required = false

[memory]
backend = "none"
auto_save = false
hygiene_enabled = false
auto_hydrate = false
```

Restart NaraeClaw, then verify `naraeclaw knowledge status` and a representative
`byoridb__memory_read` before removing any retained legacy source or snapshot.

## Roll Back to Compatibility Mode

Rollback does not require restoring the legacy source because migration leaves it intact.
To temporarily reactivate an old backend, restore its previous `[memory]` values from the
snapshot's `config.toml`, disable durable knowledge, and restart NaraeClaw. For example:

```toml
[knowledge]
enabled = false
provider = "byoridb"
byoridb_home = "~/.byoridb"
required = false

[memory]
backend = "sqlite"
auto_save = true
```

This is a compatibility path, not a second durable source to run beside ByoriDB. Imported
ByoriDB records remain in the isolated space; the rollback does not delete them. After the
issue is resolved, restore `[knowledge].enabled = true`, disable legacy auto-save again,
and rerun the idempotent migration if needed.

## Troubleshooting

- Missing `run-mcp.sh`: reinstall ByoriDB or correct `knowledge.byoridb_home`.
- Unexpected empty graph: run `naraeclaw knowledge status` and confirm the effective space;
  moving a workspace changes an automatically derived space.
- Migration uncertainty: stop after `--dry-run`; no data is written and no source is removed.
- Destructive cleanup: keep the migration snapshot and legacy source until the imported
  records have been verified through `byoridb__memory_read`.
- Backup policy: keep a separate copy or export of important Byori data; migration
  snapshots protect the cutover, not every future write.
