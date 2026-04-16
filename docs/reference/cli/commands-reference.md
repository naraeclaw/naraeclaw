# NaraeClaw Commands Reference

This reference is derived from the current CLI surface (`naraeclaw --help`).

Last verified: **March 26, 2026**.

## Top-Level Commands

| Command | Purpose |
|---|---|
| `onboard` | Initialize workspace/config quickly or interactively |
| `agent` | Run interactive chat or single-message mode |
| `gateway` | Start webhook and WhatsApp HTTP gateway |
| `acp` | Start ACP (Agent Control Protocol) server over stdio |
| `daemon` | Start supervised runtime (gateway + channels + optional heartbeat/scheduler) |
| `service` | Manage user-level OS service lifecycle |
| `doctor` | Run diagnostics and freshness checks |
| `status` | Print current configuration and system summary |
| `estop` | Engage/resume emergency stop levels and inspect estop state |
| `cron` | Manage scheduled tasks |
| `models` | Refresh provider model catalogs |
| `providers` | List provider IDs, aliases, and active provider |
| `channel` | Manage channels and channel health checks |
| `integrations` | Inspect integration details |
| `skills` | List/install/remove skills |
| `migrate` | Import from external runtimes (currently OpenClaw) |
| `props` | View, set, or initialize config properties |
| `config` | Export machine-readable config schema |
| `completions` | Generate shell completion scripts to stdout |

## Command Groups

### `onboard`

- `naraeclaw onboard`
- `naraeclaw onboard --channels-only`
- `naraeclaw onboard --force`
- `naraeclaw onboard --reinit`
- `naraeclaw onboard --api-key <KEY> --provider <ID> --memory <sqlite|lucid|markdown|none>`
- `naraeclaw onboard --api-key <KEY> --provider <ID> --model <MODEL_ID> --memory <sqlite|lucid|markdown|none>`
- `naraeclaw onboard --api-key <KEY> --provider <ID> --model <MODEL_ID> --memory <sqlite|lucid|markdown|none> --force`

`onboard` safety behavior:

- If `config.toml` already exists, onboarding offers two modes:
  - Full onboarding (overwrite `config.toml`)
  - Provider-only update (update provider/model/API key while preserving existing channels, tunnel, memory, hooks, and other settings)
- In non-interactive environments, existing `config.toml` causes a safe refusal unless `--force` is passed.
- Use `naraeclaw onboard --channels-only` when you only need to rotate channel tokens/allowlists.
- Use `naraeclaw onboard --reinit` to start fresh. This backs up your existing config directory with a timestamp suffix and creates a new configuration from scratch.

### `agent`

- `naraeclaw agent`
- `naraeclaw agent -m "Hello"`
- `naraeclaw agent --provider <ID> --model <MODEL> --temperature <0.0-2.0>`

Tip:

- In interactive chat, you can ask for route changes in natural language (for example “conversation uses kimi, coding uses gpt-5.3-codex”); the assistant can persist this via tool `model_routing_config`.

### `acp`

- `naraeclaw acp`
- `naraeclaw acp --max-sessions <N>`
- `naraeclaw acp --session-timeout <SECONDS>`

Start the ACP (Agent Control Protocol) server for IDE and tool integration.

- Uses JSON-RPC 2.0 over stdin/stdout
- Supports methods: `initialize`, `session/new`, `session/prompt`, `session/stop`
- Streams agent reasoning, tool calls, and content in real-time as notifications
- Default max sessions: 10
- Default session timeout: 3600 seconds (1 hour)

### `gateway` / `daemon`

- `naraeclaw gateway [--host <HOST>] [--port <PORT>]`
- `naraeclaw daemon [--host <HOST>] [--port <PORT>]`

### `estop`

- `naraeclaw estop` (engage `kill-all`)
- `naraeclaw estop --level network-kill`
- `naraeclaw estop --level domain-block --domain "*.chase.com" [--domain "*.paypal.com"]`
- `naraeclaw estop --level tool-freeze --tool shell [--tool browser]`
- `naraeclaw estop status`
- `naraeclaw estop resume`
- `naraeclaw estop resume --network`
- `naraeclaw estop resume --domain "*.chase.com"`
- `naraeclaw estop resume --tool shell`
- `naraeclaw estop resume --otp <123456>`

Notes:

- `estop` commands require `[security.estop].enabled = true`.
- When `[security.estop].require_otp_to_resume = true`, `resume` requires OTP validation.
- OTP prompt appears automatically if `--otp` is omitted.

### `service`

- `naraeclaw service install`
- `naraeclaw service start`
- `naraeclaw service stop`
- `naraeclaw service restart`
- `naraeclaw service status`
- `naraeclaw service uninstall`

### `cron`

- `naraeclaw cron list`
- `naraeclaw cron add <expr> [--tz <IANA_TZ>] <command>`
- `naraeclaw cron add-at <rfc3339_timestamp> <command>`
- `naraeclaw cron add-every <every_ms> <command>`
- `naraeclaw cron once <delay> <command>`
- `naraeclaw cron remove <id>`
- `naraeclaw cron pause <id>`
- `naraeclaw cron resume <id>`

Notes:

- Mutating schedule/cron actions require `cron.enabled = true`.
- Shell command payloads for schedule creation (`create` / `add` / `once`) are validated by security command policy before job persistence.

### `models`

- `naraeclaw models refresh`
- `naraeclaw models refresh --provider <ID>`
- `naraeclaw models refresh --force`

`models refresh` currently supports live catalog refresh for provider IDs: `openrouter`, `openai`, `anthropic`, `groq`, `mistral`, `deepseek`, `xai`, `together-ai`, `gemini`, `ollama`, `llamacpp`, `sglang`, `vllm`, `astrai`, `venice`, `fireworks`, `cohere`, `moonshot`, `glm`, `zai`, `qwen`, and `nvidia`.

### `doctor`

- `naraeclaw doctor`
- `naraeclaw doctor models [--provider <ID>] [--use-cache]`
- `naraeclaw doctor traces [--limit <N>] [--event <TYPE>] [--contains <TEXT>]`
- `naraeclaw doctor traces --id <TRACE_ID>`

`doctor traces` reads runtime tool/model diagnostics from `observability.runtime_trace_path`.

### `channel`

- `naraeclaw channel list`
- `naraeclaw channel start`
- `naraeclaw channel doctor`
- `naraeclaw channel bind-telegram <IDENTITY>`
- `naraeclaw channel add <type> <json>`
- `naraeclaw channel remove <name>`

Runtime in-chat commands (Telegram/Discord while channel server is running):

- `/models`
- `/models <provider>`
- `/model`
- `/model <model-id>`
- `/new`

Channel runtime also watches `config.toml` and hot-applies updates to:
- `default_provider`
- `default_model`
- `default_temperature`
- `api_key` / `api_url` (for the default provider)
- `reliability.*` provider retry settings

`add/remove` currently route you back to managed setup/manual config paths (not full declarative mutators yet).

### `integrations`

- `naraeclaw integrations info <name>`

### `skills`

- `naraeclaw skills list`
- `naraeclaw skills audit <source_or_name>`
- `naraeclaw skills install <source>`
- `naraeclaw skills remove <name>`

`<source>` accepts git remotes (`https://...`, `http://...`, `ssh://...`, and `git@host:owner/repo.git`) or a local filesystem path.

`skills install` always runs a built-in static security audit before the skill is accepted. The audit blocks:
- symlinks inside the skill package
- script-like files (`.sh`, `.bash`, `.zsh`, `.ps1`, `.bat`, `.cmd`)
- high-risk command snippets (for example pipe-to-shell payloads)
- markdown links that escape the skill root, point to remote markdown, or target script files

Use `skills audit` to manually validate a candidate skill directory (or an installed skill by name) before sharing it.

Skill manifests (`SKILL.toml`) support `prompts` and `[[tools]]`; both are injected into the agent system prompt at runtime, so the model can follow skill instructions without manually reading skill files.

### `migrate`

- `naraeclaw migrate openclaw [--source <path>] [--dry-run]`

### `config`

- `naraeclaw config schema`

`config schema` prints a JSON Schema (draft 2020-12) for the full `config.toml` contract to stdout.

### `completions`

- `naraeclaw completions bash`
- `naraeclaw completions fish`
- `naraeclaw completions zsh`
- `naraeclaw completions powershell`
- `naraeclaw completions elvish`

`completions` is stdout-only by design so scripts can be sourced directly without log/warning contamination.

### `props`

Manage individual config properties without editing `config.toml` directly.
Properties are addressed by dotted path (e.g. `channels.matrix.mention-only`).

- `naraeclaw props list` — list all properties with current values
- `naraeclaw props list --secrets` — list only secret (encrypted) fields
- `naraeclaw props list --filter channels.matrix` — filter by path prefix
- `naraeclaw props get <path>` — get a single property value (secrets show set/unset status)
- `naraeclaw props set <path> <value>` — set a property value
- `naraeclaw props set <path>` — secret fields prompt for masked input; enum fields offer interactive selection
- `naraeclaw props set --no-interactive <path> <value>` — scripted mode, no prompts
- `naraeclaw props init <section>` — create an unconfigured section with defaults (`enabled=false`)
- `naraeclaw props init` — initialize all unconfigured sections

Secret fields (API keys, tokens, passwords) are automatically detected via `#[secret]`
annotations. When setting a secret, input is masked regardless of whether a value is
provided on the command line.

Enum fields (e.g. `stream-mode`, `search-mode`) offer interactive selection via arrow
keys when the value is omitted. Provide the value directly to skip the prompt.

Shell tab-completion for property paths is included in `naraeclaw completions <shell>`.

#### Adding new config fields

Config structs derive `Configurable` with `#[prefix]` and `#[nested]` attributes.
Adding a new field to an existing struct makes it immediately available via `props`.
New enum types require a one-line `HasPropKind` impl. See `CONTRIBUTING.md` for details.

## Validation Tip

To verify docs against your current binary quickly:

```bash
zeroclaw --help
zeroclaw <command> --help
```
