# usage-cockpit 0.1.0 — pre-release

**This is a pre-release for Windows x64, tried by the author on one computer. It is not what the first brief asked for: it is not live in every way of working with Claude Code.** Read this page before you rely on it.

## What it is

A small window that shows how much of the 5-hour and the 7-day limit of Claude Code is used, with pace, forecast and charts, plus token statistics per model and per day from the files Claude Code writes on the same computer. One portable executable, no installer.

## What it cannot do

- **Live percentages only with Claude Code in a terminal.** The percentages come from the status line of Claude Code. The terminal client calls it after every answer; the VS Code extension apparently does not. If you work only in the extension, the percentages stand still (stale) and the window shows only a token count for the last 5 hours or 7 days. After a system start nothing is live until a terminal session has answered.
- **One computer only.** Other computers are not visible.
- **No Claude Code, no data.** The program does not read your account; it has no way to ask Anthropic for the numbers (for a Pro subscription there is no official interface for them).
- **Linux and macOS:** the code is compiled and linted for Linux x64, Linux arm64 and macOS arm64, but it has never been run there. No executables for them are part of this release.
- **Not signed.** Windows SmartScreen may warn.
- **Not tried on a clean Windows** (without development tools).

## How to use it

1. Start `usage-cockpit-0.1.0-windows-x64.exe` (any folder).
2. Press *Set up bridge* and confirm.
3. Open Claude Code **in a terminal** (`claude`) and send a message. The rows fill after the answer (picture: [starting-claude-code-in-a-terminal.png](images/starting-claude-code-in-a-terminal.png)).

The full manual: [user-guide.md](user-guide.md). Where the program stands and why: [current-state.md](current-state.md). What went wrong in the project and what was learned: [retrospective.md](retrospective.md).

## What is in it

See the [changelog](../CHANGELOG.md). Requirements and their state: [requirements.md](requirements.md); three of them (portable use, token use without status line data, remove everything) are still *draft*, one (the start with the system) was rejected and the switch was removed.

## Checks behind this release

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` run on Windows with all tests passing; clippy also runs for the three other targets.
- `cargo deny check licenses` accepts every dependency; the list is [THIRD_PARTY_LICENSES.md](../THIRD_PARTY_LICENSES.md).
- The CMake build with the `dist` target writes the executable and `SHA256SUMS` (tried on Windows).
- The automatic check on GitHub only looks for required files and unique requirement IDs; it does not build or test (the workflow change waits for the owner's go-ahead).

## Verify your download

Compare the SHA-256 of the file with the entry in `SHA256SUMS` of the release before you start it (`Get-FileHash <file>` in PowerShell).
