# Requirements

Status values: `draft` | `approved` | `implemented` | `verified` | `rejected`.
Only the project owner sets `approved`. IDs are never reused, even after rejection.
Statements follow the EARS pattern where it fits ("When <trigger>, the cockpit shall <response>").
Numeric targets marked *(proposal)* are initial values to be confirmed during approval.

All requirements below are a **first draft** derived from the [project brief](project-brief.md) and the [research note](research/data-sources.md).

## Functional requirements

### REQ-001 5-hour window display
- Statement: While usage data for the 5-hour window is available, the cockpit shall display the used percentage, the remaining percentage and the time until the window resets.
- Acceptance: With a sample record (used 23.5 %, reset in 2 h 10 min), the cockpit shows 23.5 % used, 76.5 % remaining and 2 h 10 min to reset, rounded consistently.
- Type: functional · Origin: brief · Status: draft

### REQ-002 Weekly limit display
- Statement: While usage data for the 7-day window is available, the cockpit shall display the used percentage, the remaining percentage and the time until the window resets.
- Acceptance: Same check as REQ-001 with a 7-day sample record; days are shown when more than 24 h remain.
- Type: functional · Origin: brief · Status: draft

### REQ-003 Pace indicator
- Statement: For each window, the cockpit shall compare actual usage with the linear target (elapsed share of the window) and show one of three states: under pace, on pace, over pace.
- Acceptance: With a tolerance band of ±5 percentage points *(proposal, configurable)*: actual 30 % at 50 % elapsed shows "under pace"; actual 52 % shows "on pace"; actual 70 % shows "over pace". The states are distinguishable by more than colour alone (shape, position or text).
- Type: functional · Origin: brief · Status: draft

### REQ-004 Deviation and pace factor
- Statement: For each window, the cockpit shall display the deviation (actual − target, in percentage points) and the pace factor (actual ÷ target).
- Acceptance: Actual 60 % at 40 % elapsed shows +20 pp and factor 1.5. At 0 % elapsed the factor is shown as not defined instead of dividing by zero.
- Type: functional · Origin: brief · Status: draft

### REQ-005 Exhaustion forecast
- Statement: When at least two usage records exist within a window, the cockpit shall forecast the time at which 100 % would be reached at the current usage rate and show whether this is before or after the reset.
- Acceptance: A synthetic series with a constant rate of 20 %/h starting at 40 % forecasts exhaustion in 3 h; if the reset is in 2 h, "reset first" is shown.
- Type: functional · Origin: brief · Status: draft

### REQ-006 Projected unused remainder
- Statement: For each window, the cockpit shall show the share of the quota that would remain unused at the reset if the current usage rate continued.
- Acceptance: Rate 5 %/h, 40 % used, reset in 4 h: projected unused remainder 40 %.
- Type: functional · Origin: brief · Status: draft

### REQ-007 Recommended pace
- Statement: For each window, the cockpit shall show the usage rate that would use the remaining quota exactly by the reset.
- Acceptance: 40 % remaining, reset in 4 h: recommended rate 10 %/h.
- Type: functional · Origin: brief · Status: draft

### REQ-008 Binding limit
- Statement: When both windows are available, the cockpit shall indicate which of the two limits would be reached first at the current pace.
- Acceptance: With synthetic data where the weekly limit is exhausted before the 5-hour window resets, the weekly limit is marked as binding.
- Type: functional · Origin: derived · Status: draft

### REQ-009 Data age
- Statement: The cockpit shall always display the age of the most recent usage record and shall mark the data as stale when it is older than 10 minutes *(proposal, configurable)*.
- Acceptance: A record written 11 minutes ago is shown with its age and a visible stale marker; a fresh record removes the marker within the refresh interval.
- Type: functional · Origin: research (source A only delivers data while Claude Code runs) · Status: draft

### REQ-010 Automatic refresh
- Statement: When a new usage record becomes available, the cockpit shall display it within 2 seconds *(proposal)* without any user action.
- Acceptance: Writing a new record to the data source updates the display within 2 s in 10 of 10 trials.
- Type: functional · Origin: brief · Status: draft

### REQ-011 Status line source
- Statement: The cockpit shall obtain the 5-hour and 7-day usage percentages and reset times from the documented Claude Code status line data, and shall tolerate missing fields or windows.
- Acceptance: Records with both windows, with only one window and with no `rate_limits` object are each handled without error; missing values are shown as "no data".
- Type: functional · Origin: research · Status: draft

### REQ-012 Status line coexistence
- Statement: Where the user already has a status line configured, the cockpit's bridge shall preserve the user's existing status line output, and it shall change the Claude Code configuration only after explicit user consent.
- Acceptance: With an existing status line command configured, enabling the bridge keeps the previous output visible in Claude Code; without consent no configuration file is modified.
- Type: functional · Origin: derived · Status: draft

### REQ-013 Local history
- Statement: The cockpit shall keep a local history of usage records sufficient to compute usage rates and to show usage over the current and previous windows.
- Acceptance: After a restart, the rate calculation and history views use records stored before the restart.
- Type: functional · Origin: brief ("all derivable data") · Status: draft

### REQ-014 Absolute token statistics
- Statement: Where local session transcripts are readable, the cockpit shall display absolute token counts per model, split into input, output and cache, and shall degrade gracefully when the format is not understood.
- Acceptance: With sample transcripts, totals per model match a reference count; an unknown format produces a "not available" notice, not a crash.
- Type: functional · Origin: research (source B) · Status: draft

### REQ-015 Token-per-percent estimate
- Statement: Where both percentage data and absolute token counts are available, the cockpit may display an estimate of tokens per percentage point, always labelled as an estimate.
- Acceptance: The value is shown only when both inputs exist and carries a visible "estimate" label.
- Type: functional · Origin: derived · Status: draft

### REQ-016 No-data state
- Statement: When no usage data is available (no subscription data, no session yet, or API-key billing), the cockpit shall explain the reason in plain language instead of showing zero values.
- Acceptance: Starting without any data source shows an explanatory message and no numeric usage values.
- Type: functional · Origin: research · Status: draft

### REQ-017 Several sessions
- Statement: When several Claude Code sessions deliver usage records, the cockpit shall use the most recent record.
- Acceptance: Two interleaved record streams result in the display of the record with the latest timestamp.
- Type: functional · Origin: research · Status: draft

### REQ-018 Compact window
- Statement: The cockpit shall run as a small window that can optionally stay on top and that remembers its position and size between starts.
- Acceptance: The window can be toggled to always-on-top; after a restart it reopens at the previous position and size.
- Type: functional · Origin: brief · Status: draft

### REQ-019 Server-side usage source (decision pending)
- Statement: Optionally, the cockpit may obtain usage data from a server-side source when no Claude Code session runs.
- Acceptance: To be defined after the owner decision on source C.
- Type: functional · Origin: research (source C) · Status: draft, blocked by owner decision

## Non-functional requirements

### REQ-101 Single executable
- Statement: The cockpit shall be delivered as one executable file per platform that runs without an installer and without requiring a separately installed runtime.
- Acceptance: On a clean machine of each target platform, copying the single file and starting it shows the cockpit window.
- Type: non-functional (delivery) · Origin: brief · Status: draft

### REQ-102 Target platforms
- Statement: The cockpit shall be buildable and runnable on at least Windows x64, macOS on Apple Silicon (M4), Linux x64 and Linux arm64 (Raspberry Pi).
- Acceptance: A release build exists for each of the four targets and passes a start-up smoke test on each.
- Type: non-functional (platform) · Origin: brief · Status: draft

### REQ-103 Local operation
- Statement: The cockpit shall work fully offline and shall not send any data over the network, except to the server-side source if REQ-019 is approved and enabled.
- Acceptance: With network access blocked, all functions based on sources A and B work; a network capture shows no outgoing connections from the cockpit.
- Type: non-functional (privacy) · Origin: derived · Status: draft

### REQ-104 Credential handling
- Statement: The cockpit shall never store, log or display authentication credentials.
- Acceptance: Code review and a log inspection after a full run show no credential values.
- Type: non-functional (security) · Origin: derived · Status: draft

### REQ-105 Resource usage
- Statement: While idle, the cockpit shall use less than 1 % CPU and less than 100 MB of memory *(proposal)* on the reference platforms.
- Acceptance: Measured over 10 minutes idle on each target platform.
- Type: non-functional (performance) · Origin: brief ("small tool window") · Status: draft

### REQ-106 Readability at a glance
- Statement: The pace state of both windows shall be recognisable within one second without reading numbers.
- Acceptance: In an informal test with at least three people, each identifies the pace state of a screenshot within one second.
- Type: non-functional (usability) · Origin: brief · Status: draft

### REQ-107 Licence compliance
- Statement: All third-party components shall have licences compatible with Apache-2.0 distribution and shall be listed with their licences in the repository.
- Acceptance: A licence inventory exists and contains every bundled dependency.
- Type: non-functional (legal) · Origin: derived · Status: draft

### REQ-108 Robustness against format changes
- Statement: When a data source delivers unknown or malformed data, the cockpit shall keep running, keep the last valid values marked as stale, and report the problem in a log.
- Acceptance: Feeding malformed records does not crash the cockpit; the last valid values remain with a stale marker.
- Type: non-functional (reliability) · Origin: research · Status: draft
