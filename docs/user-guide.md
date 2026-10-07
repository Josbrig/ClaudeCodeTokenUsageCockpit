# User guide

This guide is for someone who has never seen the project. It shows how to start the cockpit, connect it to Claude Code, read every value, and remove everything again.

> **Status.** The program is under development. **No release has been published yet: there is nothing to download and no `SHA256SUMS` file exists.** Where this guide describes a download, it describes what a release is planned to look like; until then build the program yourself (see the README). Parts that were only tried on Windows say so; the text for Linux and macOS follows the documentation of those systems and has **not been tried yet**.

## What the cockpit does

Claude Code can hand the usage of your Claude subscription to a small program of your choice, the *status line command*. The cockpit sets itself up as that program (the *bridge*), stores what it receives on your computer, and shows it in a small window that stays on top of other windows. It tells you, for the 5-hour window and the weekly window, how much you have used, when the window resets, and whether you are using it faster or slower than an even pace.

Nothing is sent anywhere. The cockpit uses no network and no tokens.

## Download and start

1. **Get the program.** *Planned for a release:* the file for your system will be on the *Releases* page of the project on GitHub, named `usage-cockpit-<version>-<system>` (with `.exe` on Windows), together with a file `SHA256SUMS`; compare the checksum of your download with the entry there before you start it. *Today:* build it with `cargo build --release` as described in the README.
2. **Put it in a folder without spaces**, for example `C:\Tools\usage-cockpit\` on Windows or `~/bin/` on Linux and macOS. The bridge command that Claude Code runs contains the path of the program, and a space in it would need quoting that not every shell accepts. The setup refuses a path with a space and tells you so.
3. **Start it.**
   - **Windows** (tried): double-click `usage-cockpit.exe`. The program is not signed yet, so Windows SmartScreen may say that it protected your PC. Choose *More info*, then *Run anyway*, if you trust the download.
   - **macOS** (not tried yet): the program is not signed or notarised yet. Open it once; if macOS refuses, open *System Settings*, *Privacy & Security*, scroll down and choose *Open Anyway* for `usage-cockpit`. This is needed only the first time.
   - **Linux** (not tried yet): mark the file as executable (`chmod +x usage-cockpit`) and run it from a terminal or your file manager.

The compact window (about 320 by 120 pixels) opens and stays on top. The first time it shows *No data yet*; the next section fixes that.

Only one cockpit runs at a time. A second start shows a small notice and ends.

## Connect it to Claude Code (set up the bridge)

The bridge makes Claude Code call the cockpit with the usage data. You set it up once.

**In the window (recommended).** Under *No data yet*, press **Set up bridge**; or open the detailed view and press **Set up bridge** at the bottom. A dialog shows exactly what will change: the file (`settings.json` of Claude Code), the new command, where your old status line is kept, and that a backup is made first. Press **Yes** to go on or **No** to change nothing. Afterwards the dialog tells you the result and where the backup is.

**On the command line.** `usage-cockpit setup-bridge` shows the same plan and asks `Change Claude Code settings? [y/N]`. On Windows the command line cannot ask; use `usage-cockpit setup-bridge --yes` or use the window. `--yes` skips the question on every system.

What it does, in plain words:

- It copies `settings.json` to `settings.json.usage-cockpit-backup-<date>-<time>` next to it. Your Claude Code settings folder is `~/.claude` (on Windows `%USERPROFILE%\.claude`), or the folder named by the environment variable `CLAUDE_CONFIG_DIR`.
- It sets the `statusLine` entry of that file to the path of the cockpit followed by `bridge`. All other entries stay as they are.
- If you already had a status line, it is kept in the cockpit's configuration folder (`bridge-state.json`), and the bridge **still runs it** for you: its output is what Claude Code shows, as before. If it fails or takes longer than one second, the bridge shows its own short text (for example `5h 23.5% · 7d 41.2%`).

Data appears after the next response of a Claude Code session. Claude Code starts the bridge when it updates its status line.

**Remove the bridge.** Press **Remove bridge** at the bottom of the detailed view, or run `usage-cockpit remove-bridge` (`--yes` on Windows). The dialog shows what changes, makes a backup, and puts your previous status line back, or removes the entry if there was none. If the status line is not the bridge, nothing is changed.

## Where your data is

| | Windows | macOS | Linux |
|---|---|---|---|
| Data (records, history, log) | `%LOCALAPPDATA%\usage-cockpit\data` | `~/Library/Application Support/usage-cockpit` | `$XDG_DATA_HOME/usage-cockpit` (default `~/.local/share/usage-cockpit`) |
| Configuration (settings, kept status line) | `%APPDATA%\usage-cockpit\config` | same folder as the data | `$XDG_CONFIG_HOME/usage-cockpit` (default `~/.config/usage-cockpit`) |

The environment variable `USAGE_COCKPIT_HOME` overrides both (data in `<folder>/data`, configuration in `<folder>/config`); it is meant for tests.

Files in the data folder: `latest.json` (the newest record), `history-v1.jsonl` (the history, kept for 35 days and at most 50 MiB), `last_error.json` (set when the bridge got input it could not read), `log.txt` and `log.1.txt` (a log of at most about 5 MB each, without the content of what Claude Code sent), and the lock files `history.lock`, `log.lock` and `cockpit.lock`. In the configuration folder: `settings.toml` and `bridge-state.json`.

The cockpit also *reads* the transcript files of Claude Code (`projects/**/*.jsonl` in the Claude Code settings folder) to count tokens. It never changes them.

## What the values mean

The window shows one row per window: label (`5h`, `7d`), a flag when it is the *binding* limit, a bar, the state, the used share, and the time until the reset. The vertical mark on the bar is the **target**: where you would be with an even pace. The detailed view shows everything below. You reach it with the small button with two arrows at the lower right of the compact view (its hover text is *Detailed view (D)*) or with the key `D`; the button *Compact view (D)* at the bottom of the detailed view, or `D` again, takes you back. Every state has a symbol and a word, so colour is never the only hint.

| Value | Meaning |
|---|---|
| **Used / Remaining** | Share of the window that is used and what is left, in percent, as Claude Code reports it. |
| **Resets** | When the window starts over, in your local time, and how long that takes. |
| **Target** | The share you would have used by now at an even pace over the window. |
| **Deviation** | Used minus target in percentage points. `+20.0 pp` means 20 points ahead of an even pace. |
| **Pace factor** | Used divided by target. `1.50×` means 50 % faster than even. |
| **State** | *under* (▼ blue): slower than even, part of the quota may expire unused. *on pace* (● green): within the tolerance band (5 points by default). *over* (▲ vermillion): faster than even, you may hit the limit before the reset. |
| **Usage rate** | How fast you currently use the window, in percent per hour, from the records of the last 30 minutes (a setting). *not available* with fewer than two records at different times. |
| **Forecast** | When 100 % would be reached at the current rate: `limit in 3h 0m, before reset`, `reset first` (the reset comes before 100 %), `limit reached`, or `not available`. |
| **Unused at reset** | How much of the quota would remain unused at the reset at the current rate. |
| **Recommended rate** | The rate that would use up the quota exactly at the reset. |
| **Binding limit** | Of the two windows, the one whose limit you would reach first (marked with a flag); none if neither would be reached before its reset. |
| **Weekly plan** | The weekly quota spread over the 5-hour windows that remain until the weekly reset: `10 windows left · 6.0% per window`. |
| **Data age / stale** | How old the newest record is (`updated 12 s ago`). After 10 minutes (a setting), or when a record that could not be read arrived after the newest good one, the values stay but are marked **stale**: the bottom line reads `stale, 14 min old`, the rows are grey with a pause symbol, and in the compact view they also get a dashed outline. |
| **From Claude Code** | Model, context window use and session cost of the newest record, as Claude Code reports them. |
| **Transcript statistics** | Tokens per model and per day from the transcript files, the cache share, and *tokens per percentage point*, which is always labelled an **estimate**. |
| **Previous periods** | The last three finished periods of each window with their end and final used share. |
| **Chart** | Per window: your used share over the current period, the even-pace line, and now. |

The detailed view also has these parts:

- **Context used** is how full the context window of the session is; **Session cost** is the cost of the session in US dollars as Claude Code estimates it. Both come from the newest record and read *no data* when Claude Code did not send them.
- **This window's limit binds first.** appears under the state of the window that is the binding limit.
- **Transcript tables.** *Per model* and *Per day* (newest first, up to 35 days) list for each model or day the tokens in four columns: **Input** (input tokens that were not served from the cache), **Output**, **Cache write** (input tokens written to the cache) and **Cache read** (input tokens read from the cache). **Cache share** is (cache write + cache read) divided by (input + cache write + cache read) over everything listed; its formula is in [docs/concept.md](concept.md), section 8. The line *transcript statistics not available* means that no transcript file could be read.
- **Footer.** The version and commit of the program (the same as `usage-cockpit --version`), a link *Licence notices* to the licences of the components used, and the buttons *Settings*, *Set up bridge*, *Remove bridge* and *Compact view (D)*.

The exact formulas of sections up to *Previous periods* are in [docs/concept.md](concept.md), section 7; the transcript statistics are described in section 8.

**Settings** (button in the detailed view): tolerance band (0 to 50 points), stale threshold (60 to 86,400 seconds), rate period (300 to 7,200 seconds), always on top, and the start view. They are saved in `settings.toml` and count at once. The window also remembers its position, size and view.

## Known limitations

- **No data without a running Claude Code session.** Claude Code hands over usage only while it runs and after its first response of a session. The percentages exist only for Claude.ai Pro and Max subscriptions.
- **Other computers are not visible.** The cockpit shows what Claude Code on this computer hands over, and its transcript statistics cover this computer only. Usage on other computers is not visible to it.
- **No support for API-key billing.** With an API key there are no such usage windows.
- **A project-level `statusLine` overrides the bridge.** If a project has its own `statusLine` in its `.claude/settings.json`, Claude Code uses that one in that project and the bridge does not run there.
- **The program must be in a folder without spaces**, and on Windows the command-line setup needs `--yes`.
- **Wayland (Linux).** Under a Wayland desktop without X11 support, "always on top" and the saved position may not work; under X11 and XWayland they are planned to work (not tried yet).
- The saved window position is not checked against your monitors: if you unplug a monitor, the window may open outside the visible area (see *Troubleshooting*).
- The percentages come from Claude Code as they are; the token counts of the transcript statistics are an estimate and not an invoice.

## Uninstall

1. Remove the bridge (see above) so that Claude Code no longer calls the cockpit. Your backup files stay next to `settings.json`; delete them if you do not need them.
2. Close the cockpit and delete the program file.
3. Delete the data and configuration folders from the table above if you want to remove everything.

## Troubleshooting

- **The window says *No data yet*.** Set up the bridge, then use Claude Code: data arrives after its next response. A Claude Code session that was already running may keep using the old settings; if no data arrives, start a new session.
- **The window says *Claude Code sent no usage limits*.** Claude Code delivers limits only for Pro and Max subscriptions and only after the first response of a session.
- **Values are grey and say *stale*.** The newest record is older than the threshold, or a record that could not be read arrived. Use Claude Code once; check the log (`log.txt` in the data folder).
- **A window says *Window reset* instead of values.** The reset time has passed and no newer record has arrived yet; use Claude Code.
- **The bridge does not seem to run.** Open the detailed view and press **Set up bridge**: it says if the bridge is already set up. Check that no project of yours has its own `statusLine`, and that the program path has no spaces.
- **The window is gone or outside the screen.** Close the cockpit, open `settings.toml` in the configuration folder, and delete the `[window]` section (or set `x` and `y` to small positive numbers), then start it again.
- ***usage-cockpit is already running.*** Another cockpit of yours is open, maybe behind other windows or minimised.
- **Settings are reset.** An invalid `settings.toml` is renamed to `settings.toml.invalid` and the defaults are used.
- **Reporting a problem.** Open an issue on GitHub; say the version (`usage-cockpit --version`) and your system, and do not paste anything private. For security problems use the private reporting described in [SECURITY.md](../SECURITY.md).
