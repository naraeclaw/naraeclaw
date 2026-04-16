# scripts/

Small operational scripts that are still part of the active project surface.

## Browser helpers

| Script | Purpose |
|--------|---------|
| `browser/start-browser.sh` | Start the local browser automation service. |
| `browser/start-vnc.sh` | Start the VNC session used by browser automation. |
| `browser/stop-vnc.sh` | Stop the VNC session. |

## CI helpers

| Script | Purpose |
|--------|---------|
| `ci/check_binary_size.sh` | Check binary size changes. |
| `ci/collect_changed_links.py` | Collect links touched by docs changes. |
| `ci/docs_links_gate.sh` | Validate documentation links. |
| `ci/docs_quality_gate.sh` | Run documentation quality checks. |
| `ci/rust_quality_gate.sh` | Run the standard Rust quality gate. |
| `ci/rust_strict_delta_gate.sh` | Run stricter Rust checks for changed code. |

## Service file

| File | Purpose |
|------|---------|
| `naraeclaw.service` | Example systemd unit for running NaraeClaw as a long-lived service. |

Keep this directory focused on scripts that are used by development, CI, browser automation, or server deployment.
