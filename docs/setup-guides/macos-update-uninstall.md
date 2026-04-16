# macOS Update and Uninstall Guide

This page documents supported update and uninstall procedures for NaraeClaw on macOS (OS X).

Last verified: **February 22, 2026**.

## 1) Check current install method

```bash
which naraeclaw
naraeclaw --version
```

Typical locations:

- Homebrew: `/opt/homebrew/bin/naraeclaw` (Apple Silicon) or `/usr/local/bin/naraeclaw` (Intel)
- Cargo/bootstrap/manual: `~/.cargo/bin/naraeclaw`

If both exist, your shell `PATH` order decides which one runs.

## 2) Update on macOS

### A) Homebrew install

```bash
brew update
brew upgrade naraeclaw
naraeclaw --version
```

### B) Clone + bootstrap install

From your local repository checkout:

```bash
git pull --ff-only
./install.sh --prefer-prebuilt
naraeclaw --version
```

If you want source-only update:

```bash
git pull --ff-only
cargo install --path . --force --locked
naraeclaw --version
```

### C) Manual prebuilt binary install

Re-run your download/install flow with the latest release asset, then verify:

```bash
naraeclaw --version
```

## 3) Uninstall on macOS

### A) Stop and remove background service first

This prevents the daemon from continuing to run after binary removal.

```bash
naraeclaw service stop || true
naraeclaw service uninstall || true
```

Service artifacts removed by `service uninstall`:

- `~/Library/LaunchAgents/com.naraeclaw.daemon.plist`

### B) Remove the binary by install method

Homebrew:

```bash
brew uninstall naraeclaw
```

Cargo/bootstrap/manual (`~/.cargo/bin/naraeclaw`):

```bash
cargo uninstall naraeclaw || true
rm -f ~/.cargo/bin/naraeclaw
```

### C) Optional: remove local runtime data

Only run this if you want a full cleanup of config, auth profiles, logs, and workspace state.

```bash
rm -rf ~/.naraeclaw
```

## 4) Verify uninstall completed

```bash
command -v naraeclaw || echo "naraeclaw binary not found"
pgrep -fl naraeclaw || echo "No running naraeclaw process"
```

If `pgrep` still finds a process, stop it manually and re-check:

```bash
pkill -f naraeclaw
```

## Related docs

- [One-Click Bootstrap](one-click-bootstrap.md)
- [Commands Reference](../reference/cli/commands-reference.md)
- [Troubleshooting](../ops/troubleshooting.md)
