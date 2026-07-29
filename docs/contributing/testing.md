# Testing Guide

NaraeClaw keeps tests in three automated levels plus shared fixtures/support.

## Testing Taxonomy

| Level | What it tests | External boundaries | Directory |
|-------|--------------|-------------------|-----------|
| **Unit** | Single function/struct | Everything mocked | `#[cfg(test)]` blocks in `src/**/*.rs` or separate `src/**/tests.rs` files |
| **Component** | One subsystem within its own boundary | Subsystem real, everything else mocked | `tests/component/` |
| **Integration** | Multiple internal components wired together | Real internals, external APIs mocked | `tests/integration/` |

## Directory Structure

| Directory | Level | Description | Run command |
|-----------|-------|-------------|-------------|
| `src/**/*.rs` | Unit | Co-located `#[cfg(test)]` blocks or separate `tests.rs` files alongside source | `cargo test --lib` |
| `tests/component/` | Component | One subsystem, real implementation, mocked boundaries | `cargo test --test test_component` |
| `tests/integration/` | Integration | Multiple internal components wired together | `cargo test --test test_integration` |
| `tests/support/` | Shared support | Mock providers, channels, tools, helpers, and trace assertions | imported by test binaries |
| `tests/fixtures/` | Fixtures | Static test data such as media files and JSON traces | loaded by tests |

## How to Run Tests

```bash
# Run all automated tests
cargo test --workspace

# Run only unit tests
cargo test --lib

# Run component tests
cargo test --test test_component

# Run integration tests
cargo test --test test_integration

# Filter within a level
cargo test --test test_integration agent

# Extended local validation (heavier than active Fast CI)
./dev/ci.sh all
```

## How to Add a New Test

1. Testing one subsystem in isolation? Use `tests/component/`.
2. Testing multiple components together? Use `tests/integration/`.
3. Avoid tests that require real external services or personal credentials. Prefer mocks and local fixtures.

After creating a test file, add it to the appropriate `mod.rs` and use shared infrastructure from `tests/support/`.

## Shared Infrastructure (`tests/support/`)

All test binaries include `mod support;` making shared mocks available via `crate::support::*`.

| Module | Contents |
|--------|----------|
| `mock_provider.rs` | `MockProvider` (FIFO scripted), `RecordingProvider` (captures requests), `TraceLlmProvider` (JSON fixture replay) |
| `mock_tools.rs` | `EchoTool`, `CountingTool`, `FailingTool`, `RecordingTool` |
| `mock_channel.rs` | `TestChannel` (captures sends, records typing events) |
| `helpers.rs` | `make_memory()`, `make_observer()`, `build_agent()`, `text_response()`, `tool_response()`, `StaticMemoryLoader` |
| `trace.rs` | `LlmTrace`, `TraceTurn`, `TraceStep` types plus `LlmTrace::from_file()` |
| `assertions.rs` | `verify_expects()` for declarative trace assertion |

## JSON Trace Fixtures

Trace fixtures are canned LLM response scripts stored as JSON files in `tests/fixtures/traces/`. They replace inline mock setup with declarative conversation scripts.

1. `TraceLlmProvider` loads a fixture and implements the `Provider` trait.
2. Each `provider.chat()` call returns the next step from the fixture in FIFO order.
3. Real tools execute normally, for example `EchoTool` processes arguments.
4. After all turns, `verify_expects()` checks declarative assertions.
5. If the agent calls the provider more times than there are steps, the test fails.

Fixture response types are `text` for plain text or `tool_calls` for LLM-requested tool execution. Common expectation fields include `response_contains`, `response_not_contains`, `tools_used`, `tools_not_used`, `max_tool_calls`, `all_tools_succeeded`, and `response_matches`.
