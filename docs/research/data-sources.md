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

Community projects report an undocumented endpoint that returns the server-side state of both windows, apparently the same data shown by Claude Code's `/usage` command. It would provide data even when no session runs. It is not documented, has not been verified for this project, depends on the local Claude login, and could change or disappear at any time. Whether to use it is an open decision of the project owner.

## Sources

- [Claude Code: Customize your status line](https://code.claude.com/docs/en/statusline) (official)
- [Claude-Code-Usage-Monitor, issue #202](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor/issues/202) (community, unverified)
- [jesdi/agent-ops, issue #94](https://github.com/jesdi/agent-ops/issues/94) (community, unverified)
