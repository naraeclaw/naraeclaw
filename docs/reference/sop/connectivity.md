# SOP Connectivity

This page describes the currently wired SOP entry points.

Last verified: **July 29, 2026**.

## Current Runtime Contract

SOP definitions may contain manual, MQTT, webhook, and cron trigger schemas, and the SOP
engine has matchers for those event types. The current daemon/gateway does not wire MQTT,
webhook, or cron events into `dispatch_sop_event`.

In particular:

- there is no `POST /sop/{*rest}` gateway route;
- `POST /webhook` performs the normal webhook chat flow and does not dispatch SOPs first;
- defining an MQTT, webhook, or cron trigger does not by itself create a live external
  subscription or gateway route.

Treat those trigger types as internal engine capability, not an available operator surface.

## Supported Entry Point

Start and progress SOP runs from an active agent session with the registered tools:

- `sop_execute` — start a named SOP manually;
- `sop_status` — inspect a run;
- `sop_approve` — approve a waiting step;
- `sop_advance` — record step completion and advance the run.

The CLI commands manage definitions only:

```bash
naraeclaw sop list
naraeclaw sop validate
naraeclaw sop show <name>
```

## Audit Limitation

`SopAuditLogger` still writes through the legacy `Memory` compatibility interface. In the
default ByoriDB mode that handle is a no-op, so SOP audit entries are not durable. Runtime
state and metrics remain available. Do not put operational audit events into ByoriDB as
user knowledge without a separate design decision.

## Reintroducing Event Fan-In

A future MQTT/webhook/cron integration must include the actual daemon/router wiring,
authentication and idempotency boundaries, focused end-to-end tests, and updates to this
document. Gateway work is high risk and needs an explicit rollback plan.

See [SOP Syntax](syntax.md), [Observability](observability.md), and
[Gateway API](../../setup-guides/gateway-api.md).
