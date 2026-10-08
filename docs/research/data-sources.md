# Research: data sources for usage limits

Status: research note, 2026-10-04.

## Summary

The usage percentages of the 5-hour window and the weekly limit can be read locally by your own software. Absolute token limits are not available.

## Source A: Claude Code status line (documented)

Claude Code runs a user-configured status line command and passes JSON on stdin on every update ([official documentation](https://code.claude.com/docs/en/statusline)). Relevant fields:

| Field | Meaning |
|---|---|
| `rate_limits.five_hour.used_percentage` | Percentage of the 5-hour window used (0–100) |
| `rate_limits.five_hour.resets_at` | Reset time of the 5-hour window (Unix epoch seconds) |
| `rate_limits.seven_day.used_percentage` | Percentage of the 7-day window used |
| `rate_limits.seven_day.resets_at` | Reset time of the 7-day window |

Further fields include the active model, context window usage, an estimated session cost and the path of the session transcript. The remaining share is `100 − used_percentage`. The command runs locally and consumes no API tokens.

**Update behaviour (per documentation):** event-driven, debounced at 300 ms. The optional `refreshInterval` setting re-runs the command every N seconds (minimum 1) in addition to events. Claude Code also re-runs it when a rate-limit window reaches its `resets_at` time.

**Limitations (per documentation):**

- `rate_limits` appears only for Claude.ai Pro and Max subscribers (or behind a gateway with a spend limit), and only after the first API response in a session. Each window may be absent independently.
- Values are percentages. The documentation states no absolute token amount per window.
- Data arrives only while a Claude Code session runs.

**Minimal example** (`settings.json`, Bash with `jq`, as in the documentation):

```json
{
  "statusLine": {
    "type": "command",
    "command": "jq -r '\"5h: \\(.rate_limits.five_hour.used_percentage // \"?\")%  7d: \\(.rate_limits.seven_day.used_percentage // \"?\")%\"'"
  }
}
```

On Windows, `jq` or a short PowerShell or Python script that parses stdin works the same way.

## Source B: local session transcripts (undocumented format)

Claude Code stores session transcripts as JSONL files in the user's Claude configuration directory. They contain token counts per message and model (input, output, cache). This allows absolute usage figures and usage rates, but contains no limit information. The format is not documented and may change with any Claude Code version.

## Source C: server-side usage endpoint (undocumented)

Status 2026-10-08: **tried once, works, undocumented.** `GET https://api.anthropic.com/api/oauth/usage` with the headers `Authorization: Bearer <access token>` and `anthropic-beta: oauth-2025-04-20` returns the same figures as `/usage` of Claude Code. The agent made one read-only call with the sign-in token that Claude Code had stored on the owner's computer (Windows: `%USERPROFILE%\.claude\.credentials.json`, key `claudeAiOauth`, which holds `accessToken`, `refreshToken`, `expiresAt`, `scopes`, `subscriptionType`). The answer was HTTP 200 with, among other fields:

| Field | Meaning |
|---|---|
| `five_hour.utilization`, `five_hour.resets_at` | percentage used and reset time (ISO 8601, UTC) of the 5-hour window |
| `seven_day.utilization`, `seven_day.resets_at` | the same for the 7-day window |
| further fields (`seven_day_opus`, `seven_day_sonnet`, `extra_usage`, `limits` and others with changing names) | present or `null`; not used |

The token needs the scope `user:profile`, which the stored token of Claude Code has. Its access part is valid for about 8 hours; Claude Code renews it when it is used. A second program that renews it could invalidate the sign-in of Claude Code, so the cockpit never renews it (REQ-019).

What is **not** known or **not official** (community reports, found by a research agent on 2026-10-08 and not verified by hand; the sources are forums, issue trackers and blogs, not Anthropic documents):

- No official way exists for third-party programs to register as a sign-in client or to read subscription usage; Anthropic's statements say that sign-in with a subscription is for Claude Code and Claude.ai, and that third parties may not offer sign-in with it or route requests through subscription credentials ([Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)). Whether reading one's own usage figures with one's own token is covered is **not stated anywhere official** that was found.
- The token made by `claude setup-token` reportedly has only the scope `user:inference` and cannot read the usage figures.
- The endpoint may answer with rate limiting (HTTP 429), may change or disappear without notice, and the reports name no account action against usage monitors (none found, not a guarantee).

Decision of the owner (2026-10-08): use the stored token of Claude Code, read-only, never renewed, nothing else (REQ-019, REQ-103, REQ-104). Whether the terms of use allow this is the owner's position to take (decision D3 in REQ-019).

## Sources

- [Claude Code: Customize your status line](https://code.claude.com/docs/en/statusline) (official)
- [Claude-Code-Usage-Monitor, issue #202](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor/issues/202) (community, unverified)
- [jesdi/agent-ops, issue #94](https://github.com/jesdi/agent-ops/issues/94) (community, unverified)
