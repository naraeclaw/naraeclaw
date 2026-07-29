---
name: byoridb-memory
description: Use ByoriDB for durable recall, standalone facts, and typed knowledge relationships across NaraeClaw sessions.
---

# ByoriDB Knowledge

ByoriDB is NaraeClaw's durable, workspace-scoped knowledge store. Use it for
information that should survive the current conversation. Keep transient chatter,
credentials, tokens, and other secrets out of it.

## Recall first

- Call `byoridb__memory_read` at the start of non-trivial work and whenever the
  user refers to an earlier decision, preference, incident, or task. Include
  links when causes or dependencies matter.
- Use `byoridb__memory_recall` only for the compatibility note layer when a
  note-only, recency-ordered lookup is specifically useful.
- Use `byoridb__memory_query_read` for read-only traversals or historical queries
  that the simpler read tools cannot express.

## Record standalone facts

Use `byoridb__memory_remember` for one durable fact whose value does not depend on
graph relationships. Give it a stable canonical name, such as
`preference:korean-responses` or `context:release-process`. Reuse that name when
the fact changes so its history remains connected.

Before writing, recall nearby knowledge and update an existing canonical item when
appropriate. Record at meaningful checkpoints, not after every conversational turn.

## Build the typed wiki

Use `byoridb__memory_wiki_upsert` when relationships carry the meaning. Supported
node types are:

- `module`
- `decision`
- `bug`
- `incident`
- `concept`
- `entity`
- `task`

Use a canonical `<type>:<stable-slug>` name. Include the rationale in decisions,
the root cause in bugs or incidents, and a useful summary in modules and concepts.

After both endpoints exist, connect them with `byoridb__memory_link`. Prefer the
narrowest accurate relation:

- `part_of`
- `depends_on`
- `affects`
- `caused_by`
- `fixed_by`
- `supersedes`
- `about`
- `relates_to`

For a changed decision, create or update the new decision, mark the old state as
superseded, and link the new node to the old one with `supersedes`. For a resolved
bug or incident, preserve the causal chain with `caused_by`, `fixed_by`, and
`about` or `affects` links.

## Quality rules

- One clear durable fact per note or node.
- Prefer stable canonical names over sentence-shaped identifiers.
- Recall before creating so equivalent knowledge is updated instead of forked.
- Use typed wiki nodes only when their links will improve later recall.
- Never store secrets or personal data without an explicit, durable need.
- If ByoriDB is unavailable, continue the current task and report that durable
  knowledge could not be read or written; do not invent recalled context.
