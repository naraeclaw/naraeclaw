# Nextcloud Talk Integration (Unsupported)

**Status:** Removed from the current public channel configuration.

The active `ChannelsConfig` supports CLI and Slack only. Although the gateway keeps a
`POST /nextcloud-talk` path for compatibility, the handler returns `404 Not Found` and does
not verify signatures, dispatch messages, or send Talk replies.

Do not add `[channels_config.nextcloud_talk]` to a current configuration and do not publish
the stub route as a webhook target. This page remains only to prevent older links from
being mistaken for a working setup guide.

Reintroducing Nextcloud Talk requires a scoped channel/gateway change, public config schema,
signature and allowlist tests, failure-mode coverage, and updated network/security docs.
Follow [Change Playbooks](../contributing/change-playbooks.md) and treat the gateway boundary
as high risk.

Current supported surfaces are listed in
[Channels Reference](../reference/api/channels-reference.md).
