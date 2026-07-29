# Getting Started Docs

For first-time setup and quick orientation.

## Start Path

1. Main overview and quick start: [../../README.md](../../README.md)
2. Install and configure durable knowledge: [byoridb-knowledge.md](byoridb-knowledge.md)
3. Connect an external client: [gateway-api.md](gateway-api.md)
4. One-click setup and dual bootstrap mode: [one-click-bootstrap.md](one-click-bootstrap.md)
5. Update or uninstall on macOS: [macos-update-uninstall.md](macos-update-uninstall.md)
6. Find commands by tasks: [../reference/cli/commands-reference.md](../reference/cli/commands-reference.md)
7. Register other MCP servers: [mcp-setup.md](mcp-setup.md)

## Choose Your Path

| Scenario | Command |
|----------|---------|
| I have an API key, want fastest setup | `naraeclaw onboard --api-key sk-... --provider openrouter` |
| I want guided prompts | `naraeclaw onboard` |
| I need durable cross-session knowledge | Install [ByoriDB knowledge](byoridb-knowledge.md) |
| Config exists, just fix channels | `naraeclaw onboard --channels-only` |
| Config exists, I intentionally want full overwrite | `naraeclaw onboard --force` |
| Using subscription auth | See [`naraeclaw auth`](../reference/cli/commands-reference.md#auth) |

## Onboarding and Validation

- Quick onboarding: `naraeclaw onboard --api-key "sk-..." --provider openrouter`
- Guided onboarding: `naraeclaw onboard`
- Existing config protection: reruns require explicit confirmation (or `--force` in non-interactive flows)
- Ollama cloud models (`:cloud`) require a remote `api_url` and API key (for example `api_url = "https://ollama.com"`).
- Validate environment: `naraeclaw status` + `naraeclaw doctor`
- Validate durable knowledge: `naraeclaw knowledge status`

## Next

- Runtime operations: [../ops/README.md](../ops/README.md)
- Reference catalogs: [../reference/README.md](../reference/README.md)
- macOS lifecycle tasks: [macos-update-uninstall.md](macos-update-uninstall.md)
