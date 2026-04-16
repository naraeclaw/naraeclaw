# GitHub Actions

Fast development mode keeps GitHub Actions intentionally small.

## Active workflow

- `ci-run.yml` — runs on pushes and pull requests to `master`.

## Fast CI commands

```bash
cargo fmt --all -- --check
cargo check --workspace --exclude naraeclaw-desktop
```

Library tests are currently run locally or in targeted follow-up work while the historical test suite is being repaired. Release, package publishing, CodeQL, label automation, and heavy matrix builds are disabled until the project reaches a steadier release cadence.
