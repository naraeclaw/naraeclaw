# Operations & Deployment Docs

For operators running NaraeClaw in persistent or production-like environments.

## Core Operations

- Day-2 runbook: [./operations-runbook.md](./operations-runbook.md)
- Troubleshooting matrix: [./troubleshooting.md](./troubleshooting.md)
- Safe network/gateway deployment: [./network-deployment.md](./network-deployment.md)

## Common Flow

1. Validate runtime (`status`, `doctor`, `knowledge status`, `channel doctor`)
2. Apply one config change at a time
3. Restart service/daemon
4. Verify channel and gateway health
5. Roll back quickly if behavior regresses

## Related

- Config reference: [../reference/api/config-reference.md](../reference/api/config-reference.md)
- ByoriDB knowledge operations: [../setup-guides/byoridb-knowledge.md](../setup-guides/byoridb-knowledge.md)
- Security collection: [../security/README.md](../security/README.md)
