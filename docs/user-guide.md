# User guide

This guide is for someone who has never seen the program. It shows how to get it running, how to use it, how to read everything it shows, and how to remove it again. Words that are new to you are explained where they first matter and in the [glossary](#9-glossary).

> **Status.** The program is under development. **No release has been published yet: there is nothing to download and no `SHA256SUMS` file exists.** Where this guide describes a download, it describes what a release is planned to look like; until then build the program yourself (see the [README](../README.md)). The Windows parts were tried (a few things were not, see [section 11](#11-known-limitations)). The text for Linux and macOS follows the documentation of those systems and has **not been tried yet**; such places say so.

## Contents

1. [Quick start](#1-quick-start)
2. [What the program does](#2-what-the-program-does)
3. [Installing the portable program](#3-installing-the-portable-program)
4. [Connecting it to Claude Code](#4-connecting-it-to-claude-code)
5. [Using the window](#5-using-the-window)
6. [Reading the display](#6-reading-the-display)
7. [Removing everything again](#7-removing-everything-again)
8. [Where your data is](#8-where-your-data-is)
9. [Glossary](#9-glossary)
10. [Troubleshooting](#10-troubleshooting)
11. [Known limitations](#11-known-limitations)

## 1. Quick start

The first five minutes:

0. **Get the program.** There is no download yet; until a release exists, build it yourself ([section 3.1](#31-get-the-program)).
1. **Start the program** (double-click `usage-cockpit.exe` on Windows). A small window opens and stays on top of other windows. It says *No data yet. Set up the bridge and use Claude Code once.*
2. **Press *Set up bridge*** in that window and confirm with *Yes*. This connects the program to Claude Code (the *bridge*, [section 4](#4-connecting-it-to-claude-code)); it changes one entry in your Claude Code settings and makes a backup of the file first.
3. **Use Claude Code in a terminal, not only in the VS Code extension.** Open a terminal (in VS Code: *Terminal*, *New Terminal*), type `claude`, and send one message. After its response the window fills with your numbers: how much of the **5-hour window** and of the **7-day window** (the weekly limit) you have used, and whether that is faster or slower than an *even pace* (using your quota evenly until the reset).
4. **Press `D`** (or the small button at the lower right) to see the detailed view.

**Important: the percentages come only from the terminal client of Claude Code.** The *status line* that carries the 5-hour and 7-day percentages is run by Claude Code in a terminal, and the VS Code extension apparently does not run it (observed on one computer: no data arrived from the extension). If you work only in the extension, the two rows get no new percentage. Once the last percentage has expired or never came, a row shows, if there are any, the tokens of the last 5 hours or 7 days that the program counts in the files of Claude Code on this computer (*1,234,567 tokens in 5 h*); a percentage that is still valid goes grey with *stale*. The program reads only what Claude Code hands over on its own computer (use on several computers together: not tried yet).

When you no longer want the program: press *Remove everything* in the detailed view and confirm, then delete the program file ([section 7](#7-removing-everything-again)). By default your history stays; the backup copies of your Claude Code settings always stay.

## 2. What the program does

Claude Code can hand the usage of your Claude subscription to a small program of your choice, the *status line command*. The cockpit sets itself up as that program (the *bridge*), stores what it receives on your computer, and shows it in a small window. For the 5-hour window and the 7-day window (the weekly limit) it tells you how much you have used, when the window resets, and whether you are using it faster or slower than an *even pace*, the pace at which you would just use up the whole quota by the reset. The goal: use your whole quota, but never run out before the reset.

Nothing is sent anywhere. The program uses no network connection of its own and no tokens, and it does not log in anywhere. (One link in the detailed view, *Licence notices*, opens your web browser when you click it.)

It reads the conversation logs of Claude Code to count tokens and never changes them. It reads `settings.json` of Claude Code only when you set up or remove the bridge, and that is the only file of Claude Code it changes, only when you ask, and (if the file exists) with a backup.

## 3. Installing the portable program

The program is **portable**: one file, no installer, no entries in the Start menu or in the list of installed programs. "Installing" means putting the file somewhere and starting it.

### 3.1 Get the program

*Planned for a release:* the file for your system will be on the *Releases* page of the project on GitHub, named `usage-cockpit-<version>-<system>` (with `.exe` on Windows), together with a file `SHA256SUMS`; compare the checksum of your download with the entry there before you start it. *Today:* build it with `cargo build --release` as described in the README; the program is `target/release/usage-cockpit` (with `.exe` on Windows).

### 3.2 Put it where you want it

Put the file in any folder you like, for example `C:\Tools\usage-cockpit\` on Windows or `~/bin/` on Linux and macOS. It does not need administrator rights and does not copy itself anywhere.

The *bridge* is explained in [section 4](#4-connecting-it-to-claude-code); here it matters only that the command Claude Code runs contains the path of the program.

- **Windows:** a space in the path is handled (the setup writes the old-style short name of the folder, such as `MYTOOL~1`, which has no space, or quotes the path).
- **Linux and macOS** (not tried yet): a path with a space or another special character is quoted for the shell (`sh`) that Claude Code uses.

If you move the program later, set the bridge up again from its new place (*Set up bridge* replaces the old entry), and, if you use the start entry ([section 4.3](#43-start-with-the-system)), press *Use this file* in the settings.

### 3.3 First start

- **Windows** (tried): double-click `usage-cockpit.exe`. The program is not signed yet, so Windows SmartScreen (the Windows protection against unknown programs) may say that it protected your PC. Choose *More info*, then *Run anyway*, if you trust the file.
- **macOS** (not tried yet): the program is not signed or notarised yet. Open it once; if macOS refuses, open *System Settings*, *Privacy & Security*, scroll down and choose *Open Anyway* for `usage-cockpit`. This is needed only the first time.
- **Linux** (not tried yet): mark the file as executable (`chmod +x usage-cockpit`) and run it from a terminal or your file manager.

The compact window (about 320 by 120 pixels) opens and stays on top. Only one cockpit runs at a time; a second start shows a small notice (*usage-cockpit is already running.*) and ends.

### 3.4 What the program changes on your computer

Until you ask, **nothing outside its own file and two folders**. At the first start it creates its data folder (a log and a lock file) and, when you move or close the window, a settings file in its configuration folder ([section 8](#8-where-your-data-is)). It changes the Claude Code settings only when you press *Set up bridge* (or *Remove bridge*, *Remove everything*), after showing you what will change. It creates a start entry of the system only when you tick the box for it ([section 4.3](#43-start-with-the-system)).

### 3.5 Check that it works

The window shows *No data yet* until Claude Code has delivered data: set up the bridge ([section 4](#4-connecting-it-to-claude-code)) and use Claude Code once. If numbers appear, everything works. If not, see [Troubleshooting](#10-troubleshooting).

## 4. Connecting it to Claude Code

### 4.1 Set up the bridge

**In the window (recommended).** Under *No data yet*, press **Set up bridge**; or open the detailed view and press **Set up bridge** at the bottom. A dialog shows exactly what will change: the file (`settings.json` of Claude Code), the new command, where your old status line is kept, and that a backup is made first (if the file does not exist yet, the dialog says that it will be created and that there is nothing to back up). It asks *Change the Claude Code settings?*; press **Yes** to go on or **No** to change nothing. Afterwards the dialog tells you the result (*The bridge is set up.*) and where the backup is, and you press **OK**. If the bridge is already set up, it says *The bridge is already set up; nothing to change.* If the settings file changed while the dialog was open, it says so and changes nothing: try again.

**On the command line.** `usage-cockpit setup-bridge` shows the same plan and asks `Change Claude Code settings? [y/N]`. On Windows the command line cannot ask; use `usage-cockpit setup-bridge --yes` or use the window. `--yes` skips the question on every system.

What it does, in plain words:

- If `settings.json` exists, it copies it to `settings.json.usage-cockpit-backup-<date>-<time>` next to it. Your Claude Code settings folder is `~/.claude` (on Windows `%USERPROFILE%\.claude`), or the folder named by the environment variable `CLAUDE_CONFIG_DIR`.
- It sets the `statusLine` entry of that file to the path of the cockpit followed by `bridge`. All other entries stay as they are.
- If you already had a status line, it is kept in the cockpit's configuration folder (`bridge-state.json`), and the bridge **still runs it** for you: its output is what Claude Code shows, as before. If it fails or takes longer than one second, the bridge shows its own short text, for example `5h 23.5% · 7d 41.2%`.

Data appears after the next response of a Claude Code session **in a terminal** (the VS Code extension apparently does not send it, see [section 1](#1-quick-start)). A session that was already running may keep using the old settings; if no data arrives, start a new session.

If a project has its own `statusLine` in its `.claude/settings.json`, Claude Code uses that one in that project and the bridge does not run there.

### 4.2 Remove the bridge only

Press **Remove bridge** at the bottom of the detailed view, or run `usage-cockpit remove-bridge` (`--yes` on Windows). The dialog shows what changes, makes a backup, and puts your previous status line back, or removes the entry if there was none; you confirm with **Yes** (or stop with **No**) and close the result with **OK**. If the status line is not the bridge, it says *The Claude Code status line is not the bridge; nothing to change.*

### 4.3 Start with the system

A box in the settings dialog makes the system start the cockpit when you sign in. Its text depends on the system: **Start with Windows (when you sign in)** on Windows, **Start with the desktop session (when you log in)** on Linux and **Start at login** on macOS. In this guide the box is called *Start with Windows* (Windows) or *the start entry* (every system).

- It acts at once and does not wait for *Save*, because it is an entry of the system and not part of the settings file.
- It is **off by default** and the program never switches it on by itself. It needs no administrator rights. Unticking the box removes the entry again.
- **Windows** (tried): one value `UsageCockpit` under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`. When the box switches the entry on or off it also removes the cockpit's own mark under `...\Explorer\StartupApproved\Run` (where Windows keeps "switched off in the list of startup apps"); nothing else in the registry is touched.
- **Linux** (compiled, not tried): a desktop entry `usage-cockpit.desktop` in the autostart folder of your desktop (`$XDG_CONFIG_HOME/autostart`, usually `~/.config/autostart`), which common desktops start at sign-in.
- **macOS** (compiled, not tried): a LaunchAgent file `io.github.josbrig.usage-cockpit.plist` in `~/Library/LaunchAgents`, which macOS loads at the next login. Nothing is started or stopped when the box is ticked, so the entry counts from the next login. A path of the program with a character that cannot be written into the file (not valid text, a control character) is refused with a message.

The box shows what the system really has:

- If the entry starts **another file** (you moved the program), the dialog says *The entry starts another file* and shows which; the button *Use this file* corrects it.
- If you switched the entry off in the startup list of the system, the dialog says so and that ticking the box switches it on again: on Windows *Switched off in the Windows list of startup apps. Ticking the box switches it on again.*, on Linux *Switched off in the startup applications of your desktop. Ticking the box switches it on again.* (macOS does not report this: the program only knows whether the file is there; if you switched the entry off in the login items of the system, that stays in effect until you switch it on there.)

## 5. Using the window

### 5.1 The compact view

Two rows, one per window, and a bottom line. A row (for example `7d`) shows from left to right:

- the **label**: `5h` is the 5-hour window, `7d` the 7-day window;
- a **flag** (⚑) if this window's limit is the one you would reach first (the *binding limit*);
- the **bar**: filled to the share you have used; the **vertical mark** on the bar is the *target*, where you would be at an even pace;
- the **state**: a symbol and a word, for example ▼ *under*, ● *on pace*, ▲ *over* ([section 6.1](#61-the-two-windows));
- the **used share**, for example `23.5%`;
- the **time until the reset**, for example `3h 0m`.

The bottom line says how old the newest data is (for example `updated 12 s ago`) and has, at the right, a small button with two arrows. Its hover text is *Detailed view (D)*. While there is no data, a message replaces the rows ([section 6.3](#63-messages)); under *No data yet* there is the button **Set up bridge**. The dialogs (settings, bridge, *Remove everything*) need more room than the compact view has, so opening one switches to the detailed view first.

### 5.2 The detailed view

The small button, or the key `D`, shows the detailed view (about 520 by 640 pixels, scrolling; switching views always sets the size of the view you switch to to its default); the button *Compact view (D)* at the bottom, or `D` again, takes you back. It has, from top to bottom:

1. a message (if there is one; here it stands above the sections, which then show *no data*) and the **age** of the data;
2. the **5-hour window** and the **7-day window**: the state, a chart of the current period, and a table of values ([section 6.1](#61-the-two-windows));
3. **Binding limit and weekly plan** ([section 6.2](#62-binding-limit-and-weekly-plan));
4. **From Claude Code**: model, context window use and session cost ([section 6.5](#65-from-claude-code-and-previous-periods));
5. **Transcript statistics** with two bar charts and two tables, the cache share and the token estimate ([section 6.4](#64-transcript-statistics-and-the-estimate));
6. **Previous periods**;
7. the **footer**: the version and commit of the program (the same as `usage-cockpit --version`), the link *Licence notices*, and the buttons *Settings*, *Set up bridge*, *Remove bridge*, *Remove everything* and *Compact view (D)*.

### 5.3 Keys, moving, size

- **`D`** (a plain `D`, no Ctrl, Alt or Shift) switches between the views. It does not react while you type in a text field or while a dialog is open.
- **Move** the window by dragging its title bar; **resize** it at its edges. The window remembers its position, size and view and opens there next time. (The setting *Start view* in the settings dialog is also written by this: when you move or close the window, it follows the view you had last, so the saved choice is only kept until then. This is being looked at in issue #147.)
- **Always on top** is a setting (on by default).
- If you unplug a monitor, the window may open outside the visible area; see [Troubleshooting](#10-troubleshooting).

### 5.4 Settings

*Settings* (button in the detailed view) opens a dialog with:

| Setting | Range | Default | Meaning |
|---|---|---|---|
| Tolerance (percentage points) | 0 to 50 | 5 | How far from the even pace still counts as *on pace*. |
| Stale after (seconds) | 60 to 86,400 | 600 | After this age the data is marked *stale*. |
| Rate period (seconds) | 300 to 7,200 | 1,800 | Over which time the *usage rate* is measured. |
| Always on top | on / off | on | Keep the window above other windows. |
| Start view | Compact / Detailed | Compact | Which view opens at start. |

A value outside its range is not accepted: the dialog says which and what the range is, and *Save* stays disabled until it is fixed. *Save* writes `settings.toml` and counts at once (it also switches to the view you chose as *Start view*); *Cancel* changes nothing. A single value of the file that is missing or out of range falls back to its default, while the other values are kept. The dialog also has the box for the start entry ([section 4.3](#43-start-with-the-system); on Windows *Start with Windows*), which does not wait for *Save*.

### 5.5 The charts

- **History chart of a window** (detailed view, under the state of a window): your used share over the current period as a line with points, a dashed grey line for the even pace from the start of the period to the reset, and a vertical line for *now*. The horizontal axis shows local times (every hour for the 5-hour window, every day for the 7-day window). The line has the colour of the window's state.
- **Bar charts of the tokens** (under *Transcript statistics*): one bar per model (horizontal, the name at the bar, in the order of the table, which is alphabetical; names longer than 28 characters are cut in the middle on the axis, the pointing text has the whole name) and one bar per day (the oldest day on the left, the newest on the right, with date labels `MM-DD` thinned out so that there are about seven at most; the newest day is always labelled). The row **Chart shows:** above them chooses what the bars are, for both charts: *Output* (the default), *Input*, *Cache write* or *Cache read*; the titles (for example *Output tokens per day*) and the axis name the choice. The four kinds differ by orders of magnitude (cache read is usually hundreds of times larger than output), which is why only one is shown at a time. The number axis uses `K` (thousand), `M` (million), `B` (billion) and `T` (trillion). Pointing at a bar shows the day or model and the exact number. The choice is kept until you close the window.

## 6. Reading the display

### 6.1 The two windows

Every state has a symbol **and** a word, so colour is never the only hint.

| State | Symbol and colour | Meaning | What you can do |
|---|---|---|---|
| *under* | ▼ blue | You use the quota slower than an even pace. Part of it may expire unused at the reset. | Nothing is wrong; there is room if you want to use more. |
| *on pace* | ● green | Within the tolerance band (5 points by default) around the even pace. | Carry on. |
| *over* | ▲ orange-red | You use the quota faster than an even pace. You may hit the limit before the reset. | Slow down, or expect to wait for the reset. |
| *stale* | ⏸ grey | The values are old ([section 6.3](#63-messages)). The numbers stay, but may no longer be true. | Use Claude Code once, or check the bridge. |
| no data | ○ grey | There is nothing to show for this window. | Read the message next to it. |

The table of values of a window in the detailed view:

| Value | Meaning |
|---|---|
| **Used** / **Remaining** | Share of the window that is used and what is left, in percent, as Claude Code reports it. Example: `23.5%`. |
| **Resets** | When the window starts over, in your local time, and how long that takes. Example: `Sat 14:00 (in 3h 0m)`. Long spans use days and hours, for example `2d 3h`. |
| **Target** | The share you would have used by now at an even pace over the window. |
| **Deviation** | Used minus target in percentage points. `+20.0 pp` means 20 points ahead of an even pace; `−20.0 pp` means 20 points behind it. |
| **Pace factor** | Used divided by target. `1.50×` means 50 % faster than even; `–` while the target is still 0. |
| **Usage rate** | How fast you currently use the window, in percent per hour, from the data that arrived in the last 30 minutes (a setting; each response of Claude Code delivers one data point). Example: `20.0 %/h`. *not available* with fewer than two data points at different times. A usage that falls (a new period begins) counts as `0.0 %/h`, which leads to *reset first*. |
| **Forecast** | What the current rate means for the end of the window: `limit in 3h 0m, before reset` (you would reach 100 % in 3 hours, before the reset), `reset first` (the reset comes before 100 %, or you are not using the window), `limit reached` (you are at 100 %), or `not available` (no rate). |
| **Unused at reset** | How much of the quota would remain unused at the reset at the current rate. Example: `40.0% unused at reset`. |
| **Recommended rate** | The rate that would use up the quota exactly at the reset. Example: `10.0 %/h`. |

A line **This window's limit binds first.** appears under the state of the window that is the binding limit.

**Example.** Halfway through the 7-day window (target 50 %) you have used 30 %: the deviation is `−20.0 pp`, the state *under*. At 52 % you are *on pace*; at 70 % you are *over*. At 60 % after 40 % of the time the deviation is `+20.0 pp` and the pace factor `1.50×`. (`pp` means *percentage points*: the difference between two percentages.)

### 6.2 Binding limit and weekly plan

- **Binding limit:** of the two windows, the one whose limit you would reach first at the current rates (marked with a flag in the compact view). *none* if neither would be reached before its reset, or if one window has no data. If both would be reached at the same moment, the 7-day window is named, because it holds you back longer.
- **Weekly plan:** the weekly quota that is left, divided by the number of 5-hour slots until the reset of the 7-day window (a slot that has begun counts as one). Example: `10 windows left · 6.0% per window`. It assumes that you could use every slot, nights included, so it is a rough guide and not a promise. It needs data for both windows; otherwise *not available*.

### 6.3 Messages

| Where | Text | Meaning |
|---|---|---|
| Top (compact view: instead of the rows) | *No data yet. Set up the bridge and use Claude Code once.* | Nothing has been received yet. Set up the bridge ([section 4.1](#41-set-up-the-bridge)) and use Claude Code. |
| Top (compact view: instead of the rows) | *Claude Code sent no usage limits. They appear only for Pro and Max subscriptions, after the first response of a session.* | Claude Code delivered data but no limits: with an API key there are none; otherwise use Claude Code once more. |
| Top (compact view: instead of the rows) | *Cannot read the data folder:* followed by the folder | The program cannot read its data folder (rights, a broken drive). Check the folder; see [section 8](#8-where-your-data-is). |
| In a window's row | *no data* | This window was not delivered. |
| In a window's row | *Window has reset. Nothing reported since; it fills again with the next report from Claude Code.* | The reset time of the newest data has passed and nothing newer has arrived, so the old value would be wrong and is not shown. Nothing needs fixing: the row fills again with the next report from Claude Code (after your next message there). |
| In a window's row, instead of *no data* or the reset text | *1,234,567 tokens in 5 h* (7 d in the other row) | The percentages come only from the status line of Claude Code, which the terminal client calls and the VS Code extension apparently does not. The program then shows, instead of the text above, the tokens it finds in the transcript files for the last 5 hours (7 days in the other row): all four kinds added up, without a percentage, a pace or a forecast. For percentages use Claude Code in a terminal. |
| Bottom line | `updated 12 s ago` | How old the newest data is. |
| Bottom line, rows grey | `stale, 14 min old` | The newest data is older than the *stale after* setting (10 minutes by default), or a record that could not be read arrived after the newest good one. The values stay, marked with ⏸ and grey; in the compact view they also get a dashed outline. |
| Next to a number | *not available* or `–` | It cannot be computed from the data there is (for example a rate needs two records). |
| Binding limit | *none* | See [section 6.2](#62-binding-limit-and-weekly-plan). |

### 6.4 Transcript statistics and the estimate

Claude Code writes a log of every conversation (*transcripts*). The program reads those files (only files that were changed within the last 35 days, and it never changes them) to count tokens. Everything inside such a file counts, also older lines, so the per-model totals and the cache share can include more than 35 days; the per-day list shows the newest 35 days. This is a second source, separate from the limits: the limits come from the bridge, the token counts from the transcripts.

- **Per model** and **Per day (newest first)** (up to 35 days) list, for each model or day, the tokens in four columns: **Input** (input tokens that were not served from the cache), **Output**, **Cache write** (input tokens written to the cache) and **Cache read** (input tokens read from the cache). A model `<synthetic>` with zeros can appear; it comes from the transcripts and is harmless. The days are your local calendar days.
- **Cache share** is (cache write + cache read) divided by (input + cache write + cache read) over all models together (the totals of the per-model table). Example: `cache share 87.0%`; `cache share not available` if there is no input at all.
- **Tokens per percentage point** is an **estimate**: all tokens of the current 5-hour period (input, output, cache write **and** cache read added together, so the number looks large) divided by how many percentage points the used share rose in that period. Example: `≈ 1,234 tokens per 1% (estimate)`. It shows *not available (estimate)* while the rise is below 1 point or there are no tokens. It is never an invoice.
- The line *transcript statistics not available* means that no transcript file could be read or understood. The format of the transcripts is not documented by Claude Code, so a change on their side can make this appear.
- Only usage on **this computer** is counted in the token statistics, because the transcripts are files of this computer. The percentages of the limits come from Claude Code as it reports them for your account; the two sources are not summed or compared except in the estimate, which can therefore be off if you use Claude elsewhere too.

### 6.5 From Claude Code and previous periods

- **From Claude Code**: **Model** (the active model of the newest data), **Context used** (how full the context window of the session is, for example `42.0%`) and **Session cost** (the cost of the session in US dollars as Claude Code computes it, for example `0.01 USD`). Each reads *no data* when Claude Code did not send it.
- **Previous periods**: the last three finished periods of each window with their end and the final used share, for example `5h  ended Sat 14:00  final 87.5%`. *none yet* before a period has ended.

## 7. Removing everything again

The program installs nothing, so removing it is deleting its file. Before you do, let it undo what it did outside its own file:

1. **Press *Remove everything*** at the bottom of the detailed view (or run `usage-cockpit uninstall`, with `--yes` on Windows). A dialog lists what will happen before it happens, and nothing is changed until you press its own button **Remove everything** (**Cancel** closes it without changing anything):
   - the bridge is removed from the Claude Code settings (a backup is made first, your previous status line comes back);
   - the start entry of the system is removed, if you had switched the start entry on (*Start with Windows* on Windows);
   - **only if you tick the box** *Also delete the history, logs and settings* (unticked at first, so the history is kept by default): the data and configuration folders from [section 8](#8-where-your-data-is) are deleted. The history is lost then. On the command line this is the option `--remove-data`; if a cockpit window is open it keeps the data and says so.
2. **Read the result and press OK.** The cockpit closes itself. If you chose to delete the data, a small hidden helper of the same program (started when you pressed *Remove everything*) waits until the window has closed and then deletes the folders; it deletes nothing if the window is still open after five minutes or if the bridge is still set up. It has no window and cannot report problems: if a folder of the table in section 8 is still there after a minute, delete it by hand.
3. **Delete the program file by hand.** The program cannot delete itself.

*Remove everything* works the same on Linux and macOS (compiled and linted there, not tried): the hidden helper waits for the window with `kill -0` instead of a Windows call, and on macOS, where the data and the configuration are one folder, that folder is cleaned once.

What stays: the copies of your Claude Code settings (`settings.json.usage-cockpit-backup-*`) next to `settings.json`, and any file in the data or configuration folder that the cockpit did not create (the folders are cleaned file by file and only removed when they are empty). If something in the list could not be done, the cockpit stays open and says what failed (it does not close, so you can read it); the history and settings are then kept, because the stored status line is needed to put your old one back.

By hand, the same is: remove the bridge ([section 4.2](#42-remove-the-bridge-only)), untick the start entry (*Start with Windows* on Windows), close the cockpit, delete the folders of section 8, delete the program file.

## 8. Where your data is

| | Windows | macOS | Linux |
|---|---|---|---|
| Data (records, history, log) | `%LOCALAPPDATA%\usage-cockpit\data` | `~/Library/Application Support/usage-cockpit` | `$XDG_DATA_HOME/usage-cockpit` (default `~/.local/share/usage-cockpit`) |
| Configuration (settings, kept status line) | `%APPDATA%\usage-cockpit\config` | same folder as the data | `$XDG_CONFIG_HOME/usage-cockpit` (default `~/.config/usage-cockpit`) |

The environment variable `USAGE_COCKPIT_HOME` overrides both (data in `<folder>/data`, configuration in `<folder>/config`); it is meant for tests.

Files in the data folder: `latest.json` (the newest record), `history-v1.jsonl` (the history, kept for 35 days and at most 50 MiB), `last_error.json` (set when the bridge got input it could not read), `log.txt` and `log.1.txt` (a log of at most about 5 MB each, without the content of what Claude Code sent), the lock files `history.lock`, `log.lock` and `cockpit.lock`, and, only while *Remove everything* is at work, the marker `uninstall-pending`. In the configuration folder: `settings.toml`, `bridge-state.json` (your previous status line) and, if a settings file was broken, `settings.toml.invalid`. Leftover `*.tmp` files of an interrupted write can appear for a moment and are removed.

The program also *reads* the transcript files of Claude Code (`projects/**/*.jsonl` in the Claude Code settings folder) to count tokens. It never changes them.

## 9. Glossary

| Term | Meaning |
|---|---|
| **5-hour window**, **7-day window** | The two limits of a Claude subscription: how much you may use within five hours, and within seven days (the *weekly limit*). Each runs from one *reset* to the next. |
| **Period** | One run of a window from one reset to the next. The program never mixes numbers of different periods. |
| **Even pace**, **target** | Using the quota evenly over the window. The target is the share you would have used by now at an even pace. |
| **Deviation** | Used minus target, in percentage points (pp). |
| **Pace factor** | Used divided by target. |
| **Tolerance band** | How far from the target still counts as *on pace* (5 points by default). |
| **Binding limit** | The window whose limit you would reach first at the current rates. |
| **Usage rate** | Percent of a window used per hour, measured over the last 30 minutes (a setting). |
| **Stale** | Said of data that is older than the *stale after* setting, or of a case where a record that could not be read came after the newest good one. |
| **Bridge** | This program in its role as the *status line command* of Claude Code: it receives the usage from Claude Code and stores it. |
| **Status line** | The text line Claude Code shows below its input; it is produced by a command you can choose. The cockpit uses that command to receive the usage. |
| **Kept status line** | The status line you had before; the bridge keeps it and still runs it for you. |
| **Transcripts** | The conversation logs of Claude Code, read to count tokens. |
| **Input / Output / Cache write / Cache read** | The four kinds of tokens in the statistics. Cache write: input written to the cache; cache read: input served from the cache. |
| **Cache share** | (cache write + cache read) ÷ (input + cache write + cache read). |
| **Portable** | One file that needs no installation and leaves nothing behind except what you create with it. |
| **Token** | The unit in which Claude counts text, roughly a short word or word part. The statistics count tokens. |
| **Cache** | Claude Code stores parts of a conversation so that it need not read them again; reading from it is cheap. *Cache write* is storing, *cache read* is reading. |
| **Context window** | How much of a conversation Claude can hold at once; *Context used* tells how full it is. |
| **Record**, **data point** | One set of numbers that Claude Code hands to the program after a response. The window says *data*. |
| **pp** | Percentage points: the difference between two percentages (from 40 % to 60 % is 20 pp). |
| **SmartScreen**, **Gatekeeper** | The Windows and macOS protections against unknown programs; they may ask before the first start of an unsigned program. |

## 10. Troubleshooting

- **Nothing happens when I double-click the program (Windows).** SmartScreen may have asked; choose *More info*, then *Run anyway*. If another cockpit already runs, you get the notice *usage-cockpit is already running.*; the first cockpit may be hidden behind other windows or minimised.
- **The window says *No data yet*.** Set up the bridge, then use Claude Code: data arrives after its next response. A Claude Code session that was already running may keep using the old settings; if no data arrives, start a new session.
- **The window says *Claude Code sent no usage limits*.** Claude Code delivers limits only for Pro and Max subscriptions and only after the first response of a session.
- **Values are grey and say *stale*.** The newest record is older than the threshold, or a record that could not be read arrived. Use Claude Code once; check the log (`log.txt` in the data folder).
- **A window says *Window has reset. Nothing reported since; ...*, *no data* or *... tokens in 5 h* (or *7 d*).** The reset time has passed, or no percentage has arrived. Open Claude Code in a terminal (`claude`) and send a message; the extension alone sends no percentages.
- **I work in the VS Code extension and nothing updates.** That is the limitation above: start `claude` in a terminal once. The program cannot read the percentages from the extension.
- **The bridge does not seem to run.** Open the detailed view and press **Set up bridge**: it says if the bridge is already set up. Check that no project of yours has its own `statusLine`, and that the program path is the one in the Claude Code settings. If you moved the program, set the bridge up again.
- **Claude Code shows my old status line only.** That is what the *kept* status line is for: the bridge runs it and shows its output. The cockpit still receives the data. If the old command fails or takes longer than a second, the bridge shows its own text.
- **The window is gone or outside the screen.** Close the cockpit, open `settings.toml` in the configuration folder, and delete the `[window]` section (or set `x` and `y` to small positive numbers), then start it again. The window then opens at its default place and at the default size of the compact view; press `D` for the detailed view.
- **The box for the start entry (*Start with Windows*) says the entry starts another file.** You moved the program. Press *Use this file*.
- **I ticked *Start with Windows*, but the program does not start.** Look at the box: if it says the entry is switched off in the Windows list of startup apps, tick it again (or switch it on in *Task Manager*, *Startup apps*). On Linux look at the startup applications of your desktop, on macOS the login items; on macOS the entry only counts from the next login.
- **After *Remove everything* a folder is still there.** The helper that deletes the data has no window and may have found a file still in use. Delete the folder by hand (the places are in [section 8](#8-where-your-data-is)). Before you delete the *configuration* folder, make sure the bridge is removed from the Claude Code settings: that folder holds your previous status line.
- **Settings are reset.** An invalid `settings.toml` is renamed to `settings.toml.invalid` and the defaults are used.
- **The numbers of the transcripts look too small.** Only this computer and only the last 35 days are counted.
- **Reporting a problem.** Open an issue on GitHub; say the version (`usage-cockpit --version`) and your system, and do not paste anything private. For security problems use the private reporting described in [SECURITY.md](../SECURITY.md).

## 11. Known limitations

- **No percentages from the VS Code extension (observed).** Claude Code hands over the percentages through its status line; the terminal client calls it, the extension apparently does not. Without a terminal session a row without a valid percentage shows token counts from the local files, if there are any, and a percentage that is still valid goes stale.
- **No data without a running Claude Code session.** Claude Code hands over usage only while it runs and after its first response of a session. The percentages exist only for Claude.ai Pro and Max subscriptions.
- **Other computers are not visible in the token statistics.** The cockpit shows what Claude Code on this computer hands over, and its transcript statistics cover this computer only.
- **No support for API-key billing.** With an API key there are no such usage windows.
- **A project-level `statusLine` overrides the bridge.** If a project has its own `statusLine` in its `.claude/settings.json`, Claude Code uses that one in that project and the bridge does not run there.
- **On Windows the command-line commands need `--yes`.**
- **Wayland (Linux).** Under a Wayland desktop without X11 support, "always on top" and the saved position may not work; under X11 and XWayland they are planned to work (not tried yet).
- The saved window position is not checked against your monitors: if you unplug a monitor, the window may open outside the visible area (see *Troubleshooting*).
- The percentages come from Claude Code as they are; the token counts of the transcript statistics are an estimate and not an invoice.
- Not tried yet: Linux and macOS, a Windows without development tools, the light theme, display scaling other than 100 %, several monitors.
- The settings *Start view* and the remembered view overwrite each other (issue #147).
