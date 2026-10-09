# Requirements

Status values: `draft` | `approved` | `implemented` | `verified` | `rejected`.
Only the project owner sets `approved`. IDs are never reused, even after rejection.
Statements follow the EARS pattern where it fits ("When <trigger>, the cockpit shall <response>").
Numeric targets marked *(proposal)* are initial values to be confirmed during approval.

All requirements below, except REQ-019, were approved by the project owner on 2026-10-05 (statement in the chat: "alle REQ genehmigt"; REQ-019 stays draft until the decision on the server-side source). They are derived from the [project brief](project-brief.md), the [project description](project-description.md) and the [research note](research/data-sources.md). Second revision 2026-10-05: gaps for a finished, tested version closed with REQ-020 to REQ-033 and REQ-109 to REQ-116.

Terms used below:

- **Window:** one of the two usage limits, the 5-hour window or the 7-day (weekly) window.
- **Record:** one set of values delivered by Claude Code to the status line command at one point in time.
- **Bridge:** the small helper started by Claude Code as status line command; it hands each record to the cockpit.
- **Cockpit:** the window application that displays the data.
- **Compact view / detailed view:** the small always-visible display and the larger view with all statistics.

## Functional requirements

### REQ-001 5-hour window display
- Statement: While usage data for the 5-hour window is available, the cockpit shall display the used percentage, the remaining percentage and the time until the window resets.
- Acceptance: With a sample record (used 23.5 %, reset in 2 h 10 min), the cockpit shows 23.5 % used, 76.5 % remaining and 2 h 10 min to reset, rounded consistently.
- Type: functional · Origin: brief · Status: approved

### REQ-002 Weekly limit display
- Statement: While usage data for the 7-day window is available, the cockpit shall display the used percentage, the remaining percentage and the time until the window resets.
- Acceptance: Same check as REQ-001 with a 7-day sample record; days are shown when more than 24 h remain.
- Type: functional · Origin: brief · Status: approved

### REQ-003 Pace indicator
- Statement: For each window, the cockpit shall compare actual usage with the linear target (elapsed share of the window) and show one of three states: under pace, on pace, over pace.
- Acceptance: With a tolerance band of ±5 percentage points *(proposal, configurable)*: actual 30 % at 50 % elapsed shows "under pace"; actual 52 % shows "on pace"; actual 70 % shows "over pace". The states are distinguishable by more than colour alone (shape, position or text).
- Type: functional · Origin: brief · Status: approved

### REQ-004 Deviation and pace factor
- Statement: For each window, the cockpit shall display the deviation (actual − target, in percentage points) and the pace factor (actual ÷ target).
- Acceptance: Actual 60 % at 40 % elapsed shows +20 pp and factor 1.5. At 0 % elapsed the factor is shown as not defined instead of dividing by zero.
- Type: functional · Origin: brief · Status: approved

### REQ-005 Exhaustion forecast
- Statement: When a current usage rate is available (REQ-021), the cockpit shall forecast the time at which 100 % would be reached at that rate and show whether this is before or after the reset; otherwise it shall show the forecast as not available.
- Acceptance: A synthetic series with a constant rate of 20 %/h starting at 40 % forecasts exhaustion in 3 h; if the reset is in 2 h, "reset first" is shown.
- Type: functional · Origin: brief · Status: approved

### REQ-006 Projected unused remainder
- Statement: When a current usage rate is available (REQ-021), the cockpit shall show for each window the share of the quota that would remain unused at the reset if that rate continued; otherwise it shall show the value as not available.
- Acceptance: Rate 5 %/h, 40 % used, reset in 4 h: projected unused remainder 40 %.
- Type: functional · Origin: brief · Status: approved

### REQ-007 Recommended pace
- Statement: For each window, the cockpit shall show the usage rate that would use the remaining quota exactly by the reset.
- Acceptance: 40 % remaining, reset in 4 h: recommended rate 10 %/h.
- Type: functional · Origin: brief · Status: approved

### REQ-008 Binding limit
- Statement: When both windows are available, the cockpit shall indicate which of the two limits would be reached first at the current pace.
- Acceptance: With synthetic data where the weekly limit is exhausted before the 5-hour window resets, the weekly limit is marked as binding.
- Type: functional · Origin: derived · Status: approved

### REQ-009 Data age
- Statement: The cockpit shall always display the age of the most recent usage record and shall mark the data as stale when it is older than 10 minutes *(proposal, configurable)*.
- Acceptance: A record written 11 minutes ago is shown with its age and a visible stale marker; a fresh record removes the marker within the refresh interval.
- Type: functional · Origin: research (source A only delivers data while Claude Code runs) · Status: approved

### REQ-010 Automatic refresh
- Statement: When a new usage record becomes available, the cockpit shall display it within 2 seconds *(proposal)* without any user action.
- Acceptance: Writing a new record to the data source updates the display within 2 s in 10 of 10 trials.
- Type: functional · Origin: brief · Status: approved

### REQ-011 Status line source
- Statement: The cockpit shall obtain the 5-hour and 7-day usage percentages and reset times from the documented Claude Code status line data, and shall tolerate missing fields or windows.
- Acceptance: Records with both windows, with only one window and with no `rate_limits` object are each handled without error; missing values are shown as "no data".
- Type: functional · Origin: research · Status: approved

### REQ-012 Status line coexistence
- Statement: Where the user already has a status line configured, the cockpit's bridge shall preserve the user's existing status line output, and it shall change the Claude Code configuration only after explicit user consent.
- Acceptance: With an existing status line command configured, enabling the bridge keeps the previous output visible in Claude Code; without consent no configuration file is modified.
- Type: functional · Origin: derived · Status: approved

### REQ-013 Local history
- Statement: The cockpit shall keep a local history of usage records sufficient to compute usage rates and to show usage over the current and previous windows.
- Acceptance: After a restart, the rate calculation and history views use records stored before the restart.
- Type: functional · Origin: brief ("all derivable data") · Status: approved

### REQ-014 Absolute token statistics
- Statement: Where local session transcripts are readable, the cockpit shall display absolute token counts per model, split into input, output and cache, the cache share, and daily totals for the retained history (REQ-025), and shall degrade gracefully when the format is not understood.
- Acceptance: With sample transcripts, totals per model and per day match a reference count and the cache share equals cache tokens ÷ all input tokens; an unknown format produces a "not available" notice, not a crash.
- Type: functional · Origin: research (source B) · Status: approved

### REQ-015 Token-per-percent estimate
- Statement: Where both percentage data and absolute token counts are available, the cockpit may display an estimate of tokens per percentage point, always labelled as an estimate.
- Acceptance: The value is shown only when both inputs exist and carries a visible "estimate" label.
- Type: functional · Origin: derived · Status: approved

### REQ-016 No-data state
- Statement: When no usage data is available (no subscription data, no session yet, or API-key billing), the cockpit shall explain the reason in plain language instead of showing zero values.
- Acceptance: Starting without any data source shows an explanatory message and no numeric usage values.
- Type: functional · Origin: research · Status: approved

### REQ-017 Several sessions
- Statement: When several Claude Code sessions deliver usage records, the cockpit shall use the most recent record.
- Acceptance: Two interleaved record streams result in the display of the record with the latest timestamp.
- Type: functional · Origin: research · Status: approved

### REQ-018 Compact window
- Statement: The cockpit shall run as a small window that can optionally stay on top and that remembers its position and size between starts.
- Acceptance: The window can be toggled to always-on-top; after a restart it reopens at the previous position and size.
- Type: functional · Origin: brief · Status: approved

### REQ-019 Other server-side usage sources
- Statement: Optionally, the cockpit may obtain the usage percentages from a server-side source other than the stored token of Claude Code (REQ-124): an own sign-in of the cockpit, a long-lived token made with `claude setup-token`, or a documented interface of Anthropic if one appears.
- Acceptance: To be defined when the owner decides to build one of these sources.
- Type: functional · Origin: research (source C) · Status: draft, deferred; to be decided when the source of REQ-124 does not work or is not enough
- Facts: research of 2026-10-08 found no official way for third-party programs to register as a sign-in client and reports that the token of `claude setup-token` has only the inference scope and cannot read the usage figures (community reports, not official); see [research/data-sources.md](research/data-sources.md), source C.

### REQ-020 Bridge record hand-over
- Statement: When Claude Code runs the bridge with a record on standard input, the bridge shall store the record for the cockpit in a per-user data directory, written so that a reader never sees a partially written file, and shall then print a status line text and exit.
- Acceptance: Feeding 1,000 records in a row while a reader polls the data file never yields an unparsable file; each record appears in the data store with the time it was received.
- Type: functional · Origin: research (source A; Claude Code cancels a running status line command when a new update arrives) · Status: approved

### REQ-021 Usage rate from the current window
- Statement: The cockpit shall compute the current usage rate of a window in percent per hour from the records of the current window only, over a sliding period of 30 minutes *(proposal, configurable)*; with fewer than two records in that period the rate shall be shown as not available.
- Acceptance: Records at 10:00 (40 %), 10:15 (45 %) and 10:30 (50 %) give 20 %/h; a single record gives "not available"; records from the previous window are ignored.
- Type: functional · Origin: derived (needed by REQ-005 to REQ-007) · Status: approved

### REQ-022 Window reset detection
- Statement: When the reset time of a window passes, or a record carries a reset time more than 10 minutes *(proposal)* later than the previous record of that window, the cockpit shall start a new window period, so that values from different periods are never combined in one calculation; smaller shifts of the reset time shall not start a new period.
- Acceptance: A record series crossing a reset (used 95 % → 3 %, reset time moves forward by 5 h) produces two separate periods; rate, forecast and pace use only the new period; the history keeps both. A series whose reset time varies by up to 60 seconds between records stays one period.
- Type: functional · Origin: derived · Status: approved

### REQ-023 Bridge setup and removal
- Statement: The cockpit shall offer to set up the bridge as the Claude Code status line command and to remove it again; before changing the Claude Code settings file it shall ask for consent and save a backup, and removal shall restore the previous status line setting.
- Acceptance: Setup on a settings file without a status line adds the bridge entry and creates a backup; setup on a file with an existing status line keeps that command for REQ-012; removal restores the file to its previous status line content; declining consent leaves the file byte-identical.
- Type: functional · Origin: derived (REQ-012) · Status: approved

### REQ-024 Settings
- Statement: The cockpit shall store its settings (pace tolerance band, stale threshold, rate period, always-on-top, view mode, window position and size) in a per-user configuration file and shall start with documented defaults when the file is missing or invalid.
- Acceptance: Changing the tolerance band in the settings view persists across a restart; deleting or corrupting the file starts the cockpit with defaults and a log entry.
- Type: functional · Origin: derived · Status: approved

### REQ-025 History retention
- Statement: The cockpit shall keep the local history without a time limit and shall delete it only on the owner's action (REQ-121: deletion of everything or of everything older than a chosen date). *(Amended by the owner's decision of 2026-10-08; before: at least 35 days and automatic removal of older records, store below 50 MB.)*
- Acceptance: With synthetic records spanning 400 days, all records are still there after a restart; the deletion of everything older than a date removes exactly those records and asks for confirmation first.
- Type: functional · Origin: derived (REQ-013), amended by owner decision 2026-10-08 (issue #191) · Status: approved (amended)

### REQ-026 Time display
- Statement: The cockpit shall show reset times in the local time zone of the computer and compute all durations from absolute timestamps, so that daylight-saving changes do not distort durations.
- Acceptance: A reset time across a daylight-saving change (e.g. 2026-10-25 in Central Europe) shows the correct local clock time and the correct remaining duration.
- Type: functional · Origin: derived · Status: approved

### REQ-027 Weekly planning
- Statement: While both windows are available, the cockpit shall show how many 5-hour windows remain until the weekly reset and the share of the weekly quota per remaining 5-hour window that would use the weekly quota exactly by its reset.
- Acceptance: 60 % of the week remaining and 50 h to the weekly reset give 10 remaining 5-hour windows and 6 % of the weekly quota per window.
- Type: functional · Origin: project description ("across windows") · Status: approved

### REQ-028 Session details
- Statement: Where present in the latest record, the detailed view shall show the active model, the context window usage and the estimated session cost, labelled as delivered by Claude Code.
- Acceptance: A record with model, context usage 8 % and cost 0.01234 USD shows these three values; a record without them shows "no data" for each.
- Type: functional · Origin: project description (raw data of source A) · Status: approved

### REQ-029 Compact and detailed view
- Statement: The cockpit shall offer a compact view with the pace state, usage and time to reset of both windows plus the data age and stale marker (REQ-009), and a detailed view with all metrics of REQ-003 to REQ-008, REQ-014, REQ-015, REQ-027 and REQ-028; the user shall switch between them with one action.
- Acceptance: In the compact view both pace indicators are visible in a window of at most 320 × 120 logical pixels *(proposal)*; one click or key switches to the detailed view and back.
- Type: functional · Origin: brief (small window, all statistics) · Status: approved

### REQ-030 Usage history chart
- Statement: The detailed view shall show a chart of usage over time for the current period of each window together with the linear target line.
- Acceptance: With a synthetic series, the chart shows the recorded points, the target line from 0 % at the period start to 100 % at the reset, and the current time.
- Type: functional · Origin: brief ("graphical visualisation") · Status: approved

### REQ-031 Logging
- Statement: The cockpit and the bridge shall write a log file in the per-user data directory with timestamps and severity, limited to 5 MB *(proposal)* by rotation, shall record the Claude Code version delivered with a record when it changes, and shall never write credentials or record fields they do not use.
- Acceptance: Triggering a malformed record produces a log line with time and severity; after 6 MB of log output the active log file is below 5 MB; a change of the delivered Claude Code version produces one log line; with seeded fake credential strings in the environment and in an unused record field, a search of all log files finds none of them.
- Type: functional · Origin: derived (REQ-104, REQ-108) · Status: approved

### REQ-032 Version information
- Statement: The executable shall report its version and build commit on request from the command line, and the detailed view shall show the version and a link to the licence notices.
- Acceptance: Running the executable with `--version` prints the version and commit and exits with code 0; the detailed view shows the same version.
- Type: functional · Origin: derived · Status: approved

### REQ-033 Single instance
- Statement: When the cockpit is started while another cockpit instance of the same user is running, the new instance shall bring the running one to the front or exit with a message, and shall not run a second display.
- Acceptance: Starting the cockpit twice results in exactly one cockpit window.
- Type: functional · Origin: derived · Status: approved

## Non-functional requirements

### REQ-101 Single executable
- Statement: The cockpit, including the bridge, shall be delivered as one executable file per platform that runs without an installer and without requiring a separately installed runtime.
- Acceptance: On a clean machine of each target platform, copying the single file and starting it shows the cockpit window.
- Type: non-functional (delivery) · Origin: brief · Status: approved

### REQ-102 Target platforms
- Statement: The cockpit shall be buildable and runnable on at least Windows x64, macOS on Apple Silicon (M4), Linux x64 and Linux arm64 (Raspberry Pi).
- Acceptance: A release build exists for each of the four targets and passes a start-up smoke test on each.
- Type: non-functional (platform) · Origin: brief · Status: approved

### REQ-103 Network use only for the owner's purpose
- Statement: The cockpit may use the internet to sign in to the owner's Anthropic account and to ask Anthropic for the owner's usage data (REQ-124), and for nothing else: it shall send no usage data, no telemetry and no analytics to anyone, shall make no connection to any other destination, and shall send only what the request to Anthropic needs. Without a network connection it shall keep working with the data it has (database, transcript files, status line records) and say that the account data is not reachable. *(Amended by the owner's decision of 2026-10-08; before: fully offline, no data over the network except to the source of REQ-019. The earlier wording was derived by the agent and not asked for in the project brief.)*
- Acceptance: A network capture of a full run shows connections only to the Anthropic hosts that REQ-124 needs and none to any other host; with the network blocked, the views based on the database and the transcript files still work and the account rows say why they are stale; the source code has no other network code (check by `cargo tree` and a search).
- Type: non-functional (privacy, safety) · Origin: derived, amended by owner decision 2026-10-08 · Status: approved (amended)

### REQ-104 Credential handling
- Statement: The cockpit shall never log or display authentication credentials, and shall not store them in its own files. If REQ-124 is approved with the use of the sign-in token that Claude Code stored, the cockpit reads that token only in memory for the requests of REQ-124 and never copies it (decision D2 of REQ-124).
- Acceptance: Code review and a log inspection after a full run show no credential values.
- Type: non-functional (security) · Origin: derived · Status: approved

### REQ-105 Resource usage
- Statement: While idle, the cockpit shall use less than 1 % CPU and less than 100 MB of memory *(proposal)* on the reference platforms.
- Acceptance: Measured over 10 minutes idle on each target platform.
- Type: non-functional (performance) · Origin: brief ("small tool window") · Status: approved

### REQ-106 Readability at a glance
- Statement: The pace state of both windows shall be recognisable within one second without reading numbers.
- Acceptance: In an informal test with at least three people, each identifies the pace state of a screenshot within one second.
- Type: non-functional (usability) · Origin: brief · Status: approved

### REQ-107 Licence compliance
- Statement: All third-party components shall have licences compatible with Apache-2.0 distribution and shall be listed with their licences in the repository.
- Acceptance: A licence inventory exists and contains every bundled dependency.
- Type: non-functional (legal) · Origin: derived · Status: approved

### REQ-108 Robustness against format changes
- Statement: When a data source delivers unknown or malformed data, the cockpit shall keep running, keep the last valid values marked as stale, and report the problem in a log.
- Acceptance: Feeding malformed records does not crash the cockpit; the last valid values remain with a stale marker.
- Type: non-functional (reliability) · Origin: research · Status: approved

### REQ-109 Bridge speed and fallback output
- Statement: The bridge's own processing shall finish within 100 ms *(proposal)* on the reference platforms, and the bridge shall always print a status line text, also when the record is malformed or the data directory is not writable. Where a previous user status line command is kept (REQ-012), the bridge shall store the record before running that command, give it at most 1 second *(proposal)*, and print its own text if the command fails or times out.
- Acceptance: Median run time over 100 calls without a kept user command is below 100 ms on each target platform; with a malformed record or a read-only data directory the bridge still prints a text and exits with code 0; with a kept command that sleeps 5 s the record is stored and the bridge exits after at most about 1 s with its own text.
- Type: non-functional (performance, reliability) · Origin: research (Claude Code cancels slow status line commands) · Status: approved

### REQ-110 Automated tests
- Statement: All calculations (REQ-003 to REQ-008, REQ-021, REQ-022, REQ-027), the record parser and the bridge shall be covered by automated tests whose names contain the REQ-ID, and the tests shall run in CI on every pull request.
- Acceptance: The CI log of a pull request lists passing tests for each of these REQ-IDs.
- Type: non-functional (quality) · Origin: derived (working model) · Status: approved

### REQ-111 Release builds in CI
- Statement: CI shall build the executable for all target platforms of REQ-102 from the same commit and attach the files to a release draft when the owner requests a release.
- Acceptance: A CI run on the release branch or tag produces four executables with the version in their names and a checksum file.
- Type: non-functional (delivery) · Origin: brief · Status: approved

### REQ-112 User documentation
- Statement: The repository shall contain user documentation covering download and start per platform, bridge setup and removal, data and configuration locations, the meaning of every displayed value, known limitations (no data without a running Claude Code session, usage on other computers is not visible, no support for API-key billing), uninstalling, and troubleshooting.
- Acceptance: A person who has not seen the project can set up and remove the bridge on one platform using only the documentation.
- Type: non-functional (usability) · Origin: derived · Status: approved

### REQ-113 Display scaling
- Statement: The cockpit shall render sharply and keep its layout at display scaling factors from 100 % to 200 %.
- Acceptance: Screenshots at 100 %, 150 % and 200 % show no clipped text and no blurred graphics.
- Type: non-functional (usability) · Origin: derived · Status: approved

### REQ-114 Colour-independent states
- Statement: Extending REQ-003 to every state, all states (pace, stale, no data, binding limit) shall be distinguishable without colour, and the colours used shall remain distinguishable with common colour-vision deficiencies.
- Acceptance: A greyscale screenshot still shows every state; a check with a colour-blindness simulator (deuteranopia, protanopia) shows distinct states.
- Type: non-functional (accessibility) · Origin: derived (REQ-003, REQ-106) · Status: approved

### REQ-115 Data format versioning
- Statement: Every file written by the bridge or the cockpit shall carry a format version, and a newer cockpit shall read files written by older versions.
- Acceptance: A data store written with format version 1 is read correctly after an upgrade to a version that writes format version 2.
- Type: non-functional (maintainability) · Origin: derived (REQ-108) · Status: approved

### REQ-116 Start-up time
- Statement: The cockpit shall show its window within 2 seconds *(proposal)* of being started on the reference platforms.
- Acceptance: Measured over 5 starts on each target platform.
- Type: non-functional (performance) · Origin: derived ("small tool window") · Status: approved

### REQ-117 Portable use
- Statement: The cockpit shall run from any folder without installation, and setting up the bridge shall work from any folder, including a folder whose path contains a space. REQ-023 is unchanged. On Windows a space in the path is handled with the short folder name or double quotes, on Linux and macOS by quoting for `sh -c`.
- Acceptance: A copy of the executable in a folder with a space in its path sets up the bridge, and the command written to the Claude Code settings runs the bridge through the shell Claude Code uses on that system (tried on Windows with Git Bash) and is recognised again by the removal.
- Type: non-functional (usability) · Origin: owner decision 2026-10-07 · Status: draft

### REQ-118 Optional start with the system
- Statement: The cockpit shall offer a switch in its settings to start with the user's sign-in (desktop session), which is off by default, needs no administrator rights, can be switched off again, and shows the real state of the system entry (including a "switched off" mark that the user set in the system's list of startup apps).
- Acceptance: Switching on creates exactly one per-user start entry for the current executable path (and clears the cockpit's own "switched off" mark of the system, if there is one), switching off removes the entry and that mark, and the first start never creates it by itself.
- Type: functional · Origin: owner decision 2026-10-07 · Status: rejected (owner decision 2026-10-08: the switch suggests live values after a system start that the data source cannot give; removed with issue #181)

### REQ-119 Remove everything
- Statement: The cockpit shall offer a function (window and command line) that undoes everything it created outside its own executable: the bridge entry in the Claude Code settings (with backup), the start entry of REQ-118 and, on the user's explicit choice, its data and configuration folders; it names the backups that stay, and the executable can then be deleted by hand.
- Acceptance: After the function ran on a test system, a search for entries of the cockpit in the Claude Code settings and in the system's start entries finds none, and the data and configuration folders are gone when chosen.
- Type: functional · Origin: owner decision 2026-10-07 · Status: draft

### REQ-120 Token use without status line data
- Statement: When a window has no current percentage from the status line (no record, or its reset time has passed), the cockpit shall show the tokens of the last window length (5 hours, 7 days) taken from the transcript files, in place of the percentage (no pace, no forecast), so that use in clients that send no status line data (the VS Code extension) is visible.
- Acceptance: With transcript messages inside the last 5 hours and no current five-hour record, the five-hour row names the token sum of those hours and shows only that sum and no percentage; without such messages the row keeps its previous text.
- Type: functional · Origin: owner decision 2026-10-08 · Status: draft

### REQ-121 Persistent usage database
- Statement: The cockpit shall store the usage data it has seen in one local database file (SQLite) in the per-user data folder of the computer on which it runs: the token counts per message (time, model, project, session, the four token kinds) and the status line and account records with their percentages. New data is added incrementally so that every message is counted once. The data is kept without a time limit and removed only on the owner's action (everything, or everything older than a chosen date, after a confirmation that names the amount). The history is shown from the database beyond the current run and beyond 35 days: per hour, day, week and month, per model, project, session and token kind, and further views that the owner asks for. The history stays when Claude Code has deleted its transcript files.
- Acceptance: After the transcripts of an old day are deleted from the Claude Code folder, the day is still shown with its token counts; after a restart the history is shown at once, without reading all transcripts again; every message is counted once also after several starts and after a repeated scan; the deletion asks first, names the amount and removes only the chosen range, and a later scan does not bring the deleted range back.
- Type: functional · Origin: owner request 2026-10-08 (issue #191) · Status: draft

### REQ-122 Live view of the whole token use
- Statement: The cockpit shall show the token use of every Claude Code client on the computer on which it runs (terminal, VS Code extension, other clients that write the transcript files) live: a new answer appears in the numbers and the charts within 2 seconds, also right after the start of the cockpit and of the system, without a running terminal session and without Claude Code needing to call the cockpit. The values come from the transcript files (counts) and are stored by REQ-121.
- Acceptance: With the cockpit running and an answer in the VS Code extension, the changed token count is in the window within 2 seconds in 10 of 10 tries (time measured from the time stamp of the answer in the transcript file); after the start of the cockpit the history is shown at once and a new answer is added within 2 seconds; idle CPU stays below 1 % and memory below 100 MB (REQ-105).
- Type: functional · Origin: owner request 2026-10-08 (the whole Claude token use live and in the database) · Status: draft
- Note: this is the acceptance test of the purpose of the cockpit in the sense of the owner: live in daily work. REQ-010 (display within 2 seconds of a new record) is met by it for the token counts; for the percentages REQ-124 is needed.

### REQ-123 Transfer of the history to another computer
- Statement: The cockpit should be able to export the stored history of its computer into a file and to import such a file from another computer, so that the whole token use of the owner can be seen in one database; imports never count a message twice, name the computer they come from, and the views can be limited to one computer. The file contains only counts, identifiers, times, model and project names, no message text and no credentials.
- Acceptance: Export, then import twice into the same database changes nothing the second time; an import from a sample file of another computer adds exactly its messages and labels them with the computer name; a search of an export for sample message text and for credential strings finds none.
- Type: functional · Origin: owner request 2026-10-08 (desirable) · Status: draft (priority: should)

### REQ-124 Account usage percentages with the stored token of Claude Code
- Statement: The cockpit shall obtain the 5-hour and 7-day usage percentages and their reset times from the owner's own subscription account, with the sign-in token that Claude Code has stored on the same computer, so that the percentages are live without a terminal session and for every way of working with Claude Code (terminal, VS Code extension, other computers of the same account). The cockpit reads the token only into memory for the request, never writes, copies, logs or shows it, and **never refreshes it** (the refresh belongs to Claude Code; a second refresher can invalidate the sign-in of Claude Code). It asks at an interval that the owner can set (default 60 seconds *(proposal)*, minimum 15 seconds), keeps to the answer of the server when it says to wait, shows the age of the values (REQ-009), and falls back to REQ-011 and REQ-120 when the token is missing, expired or the account cannot be reached, saying which of these it is.
- Acceptance: On the owner's computer with a Claude Pro subscription and Claude Code signed in, the cockpit shows the same percentages as `/usage` of Claude Code within 1 percentage point and updates them at the set interval, also after work in the VS Code extension and with no terminal session; with the credentials file removed, with an expired token and with the network blocked, the rows show the last values marked as stale and name the reason; a search of the program's files, logs and database for the token finds none; the credentials file is unchanged after a run (same bytes).
- Type: functional · Origin: owner statement 2026-10-08 · Status: draft (D3 is the owner's to take before it is built)
- **D1 (decided by the owner on 2026-10-08):** first and for now only the stored token of Claude Code. An own sign-in of the cockpit and a token made with `claude setup-token` are optional for later (REQ-019): research of 2026-10-08 found no official way for third-party programs to register as a sign-in client, and reports that the token of `claude setup-token` has only the inference scope and cannot read the usage figures (community reports, not official; see issue #175).
- **D2 (decided by the owner on 2026-10-08):** the program may use the internet for this and for nothing else (REQ-103 amended). Reading the stored token is allowed under the rules above (REQ-104 amended).
- **D3 (position on the terms of use, the owner's to take):** the interface `GET https://api.anthropic.com/api/oauth/usage` is not documented. The Consumer Terms of Service (effective October 8, 2025) list as a prohibited use: "Except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it, to access the Services through automated or non-human means, whether through a bot, script, or otherwise." A program of the cockpit that calls the interface with the sign-in token is such an access; no explicit permission was found. The Claude Code page "Legal and compliance" says that OAuth authentication "is designed to support ordinary use of Claude Code and other native Anthropic applications" and that Anthropic "may [take measures] without prior notice". Community reports name no account action against tools that only read the own usage (that is not a guarantee); the endpoint may answer with rate limiting (429) and may change or disappear without notice. A first call with the stored token on the owner's computer on 2026-10-08 answered 200 with the figures of both windows and the reset times. Details and sources: [research/token-dimensions-and-terms.md](research/token-dimensions-and-terms.md).
- Known limit: the access token of Claude Code is valid for about 8 hours and is renewed by Claude Code when it is used. On a computer where Claude Code is not used for longer, the token expires and the cockpit shows the last values as stale (REQ-009) until Claude Code renews it. Renewing it in the cockpit is excluded by the rule above.
- Facts: [research/data-sources.md](research/data-sources.md), source C.
