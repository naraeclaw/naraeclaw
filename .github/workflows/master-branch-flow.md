# Master Branch Delivery Flow

This repository is in fast development mode while the NaraeClaw desktop app and Korean-first runtime settle.

## Current Policy

- `master` is the main development branch.
- Direct commits and fast merges to `master` are allowed for owner-approved work.
- Pull requests are optional. Use them for review-heavy, risky, or parallel-agent changes.
- Release and publish workflows are manual only until the project reaches a steadier release cadence.

## Active Automatic Workflows

| File | Trigger | Purpose |
| --- | --- | --- |
| `ci-run.yml` | `push` to `master`, `pull_request` to `master`, manual dispatch | Fast validation: format check, workspace check, library tests |

## Manual Workflows

| File | Purpose |
| --- | --- |
| `checks-on-pr.yml` | Full historical CI matrix when a deeper validation pass is needed |
| `cross-platform-build-manual.yml` | Cross-platform build smoke test |
| `pre-release-validate.yml` | Release readiness checks before tagging or publishing |
| `release-beta-on-push.yml` | Manual beta release |
| `release-stable-manual.yml` | Manual stable release |
| `publish-crates.yml` / `publish-crates-auto.yml` | Manual crates.io publishing |
| `version-sync.yml` | Manual version reference sync |
| `pub-homebrew-core.yml` / `pub-scoop.yml` | Manual package manager publishing |
| `discord-release.yml` / `tweet-release.yml` | Manual or called release announcement helpers |
| `pr-path-labeler.yml` | Disabled manual no-op during fast development mode |

## Fast CI

`ci-run.yml` runs a single Ubuntu job:

```bash
cargo fmt --all -- --check
cargo check --workspace --exclude zeroclaw-desktop --locked
cargo test --workspace --lib --locked
```

This is intentionally smaller than the previous PR gate. It catches formatting and broad Rust compile/test regressions without running clippy, nextest installation, security audit, docs gates, benchmark compile, or cross-platform release builds on every iteration.

## When To Run More

Run `checks-on-pr.yml` or local full validation when touching:

- release automation;
- security policy or credential handling;
- gateway/webhook boundaries;
- tool execution permissions;
- dependency upgrades;
- platform-specific packaging.

For normal desktop UI, Korean copy, docs, and focused runtime fixes, the fast CI job plus targeted local checks is enough during this phase.
