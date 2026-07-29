# Matrix E2EE Integration (Unsupported)

**Status:** Removed from the current public channel configuration.

Matrix and its `channel-matrix` Cargo feature are not part of the current runtime contract.
Older homeserver, access-token, room, device, and E2EE instructions have been removed so
they cannot be mistaken for a supported security procedure.

Reintroducing Matrix E2EE requires a scoped channel implementation, explicit Cargo feature,
public config schema, cryptographic state persistence, allowlist tests, and an end-to-end
encrypted-room runbook. Treat that work as security-sensitive.

See [Channels Reference](../reference/api/channels-reference.md) for current surfaces and
[Change Playbooks](../contributing/change-playbooks.md) for contribution requirements.
