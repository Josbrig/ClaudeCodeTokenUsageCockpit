# Claude Code Token Usage Cockpit

A tiny, always-on-top cockpit that shows your Claude **5-hour** and **weekly** usage limits as live as possible and tells you at a glance whether you are using them faster or slower than an even pace. The goal: use your whole quota, but never run out before the reset.

> **Status: concept phase.** There is no code and no release yet. Everything below describes the planned product. See [docs/project-description.md](docs/project-description.md) and the draft [requirements](docs/requirements.md).

## Planned features

- Live view of the 5-hour window and the weekly limit: used, remaining, time to reset.
- **Pace indicator:** actual usage compared with a linear target. Too slow means quota will expire unused, too fast means you will hit the limit before the reset.
- Forecast of when 100 % would be reached at the current pace, the projected unused remainder, and the pace that would use the quota exactly by the reset.
- Updates itself without user action and always shows how old the displayed data is.
- One executable per platform, no installer. Targets: Windows x64, macOS Apple Silicon, Linux x64, Linux arm64 (Raspberry Pi).

## Where the data comes from

Claude Code passes the usage percentages and reset times of both windows to a configurable [status line command](https://code.claude.com/docs/en/statusline). The cockpit is planned to read this documented interface locally. It does not consume any tokens. Known limits of this source: it works only for Claude.ai Pro/Max subscriptions, only while Claude Code runs, and it reports percentages, not absolute token counts. Details and alternatives: [docs/research/data-sources.md](docs/research/data-sources.md).

## Documentation

| Document | Content |
|---|---|
| [docs/project-brief.md](docs/project-brief.md) | The original project brief |
| [docs/project-description.md](docs/project-description.md) | Goals, metrics, data sources, risks |
| [docs/requirements.md](docs/requirements.md) | Requirements with IDs (draft) |
| [docs/traceability.md](docs/traceability.md) | Requirement → issue → PR → test |
| [docs/decisions/](docs/decisions/) | Architecture decision records |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How work is organised in this repository |

## AI-assisted development

This project is developed with the help of an AI coding agent (Claude Code), under the direction and responsibility of the maintainer. Commits made with agent assistance carry a `Co-Authored-By` trailer, and issues and pull requests created by the agent carry the `agent` label.

## Disclaimer

This is an unofficial project. It is not affiliated with, endorsed by, or sponsored by Anthropic. "Claude" and "Claude Code" are trademarks of Anthropic, used here only to describe compatibility.

## License

[Apache License 2.0](LICENSE)
