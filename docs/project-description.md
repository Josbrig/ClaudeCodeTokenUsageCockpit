# Project description

Status: draft, 2026-10-04. Basis for the concept document and the [requirements](requirements.md).

## 1. Goal

A small, always visible tool window ("status cockpit") shows the consumption of the Claude usage limits as up to date as possible. At any moment it answers one question at a glance:

> **Am I using my quota faster or slower than an even pace until the next reset?**

Behind this is a twofold goal:

- **Waste nothing:** use the available quota as fully as possible. Whatever is left at the reset expires.
- **Never run dry:** reach the limit as rarely as possible before the reset.

The cockpit shows all available data and calculates every metric that can be derived from it.

## 2. Core functions

1. **Live view of both limits:** the 5-hour window and the weekly limit, each with usage, remainder and time to reset.
2. **Pace visualisation:** actual usage compared with the linear target, readable without thinking:
   - **too slow:** quota will expire unused;
   - **on target;**
   - **too fast:** the limit will be reached before the reset.
3. **Self-updating:** the program keeps itself current without user action and always shows the age of the displayed data.
4. **Statistics:** all available raw data and all derived metrics (section 4).
5. **Compact window:** small, optionally always on top, freely positionable.

## 3. Data sources

This is the critical point of the project: Anthropic offers **no officially documented interface** through which a standalone program can query the limits. Three sources are candidates (details: [research/data-sources.md](research/data-sources.md)).

| Source | Provides | Status | Limitations |
|---|---|---|---|
| **A. Claude Code status line** | 5-hour and 7-day usage in percent, reset times; plus model, context usage, estimated session cost, transcript path | **officially documented** | Claude.ai subscriptions (Pro/Max) only; only after the first API response of a session; only while Claude Code runs; percentages only |
| **B. Local session transcripts** (JSONL) | absolute tokens per message and model (input, output, cache) | available, **format undocumented** | no limit information; format may change with any Claude Code version |
| **C. Server-side usage endpoint** (reported by community projects) | server-side state of both windows, even without a running session | **undocumented, unverified** | depends on the local Claude login; may disappear at any time; terms of use unclear |

For source A, Claude Code runs the status line command on events and, with `refreshInterval`, also at fixed intervals of at least one second. A small bridge program can write the data to a local file on every call, and the cockpit reads that file. Source A consumes no API tokens.

**Consequences:**

- Source A is the reliable basis. "Live" therefore means: as fresh as the source allows. The cockpit shows the data age openly instead of pretending to be current.
- Source B adds absolute amounts and usage rates.
- Whether source C is used is a decision of the project owner. It would be the only way to know the state without a running Claude Code session, at the cost of a security risk (handling the login credential) and a maintenance risk (undocumented).
- Data shown inside the Claude desktop app is not accessible to an external program.

## 4. Metrics

**Raw data (source A):** usage in percent and reset time for the 5-hour window and the week, active model, context usage, estimated session cost.

**Derived per window:**

- remainder in percent and time to reset;
- elapsed share of the window;
- **linear target usage** = elapsed time ÷ window length;
- **deviation** = actual − target, in percentage points;
- **pace factor** = actual ÷ target (1.0 = on target);
- current usage rate in percent per hour, from the cockpit's own history;
- **forecast:** the time at which 100 % is reached at the current pace, compared with the reset;
- **projected unused remainder** at the reset, at the current pace;
- **recommended pace** to use the quota exactly by the reset.

**Across windows:**

- which limit is currently binding (5-hour or weekly);
- how many 5-hour windows remain until the weekly reset, and which usage per window fits.

**From source B:**

- tokens per model, split into input, output and cache;
- cache share;
- history over days and windows.

**Combined A + B (estimate):** tokens per percentage point. This allows an approximate estimate of the absolute size of the limits. It is always labelled as an estimate.

## 5. Platforms and delivery

- **One compiled executable per platform, no installer.** Copy and run.
- Minimum targets:
  - Windows x64;
  - macOS Apple Silicon (M4);
  - Linux x64;
  - Linux arm64 (Raspberry Pi).
- Further common platforms are desirable, for example Windows arm64 and macOS x64.
- **Technology: open.** It will be evaluated in the concept document against these criteria:
  - a single executable;
  - cross-compilation for all target platforms;
  - a small GUI window with custom graphics;
  - low resource usage.
- **Note:** unsigned programs trigger warnings on macOS (Gatekeeper) and Windows (SmartScreen). Whether to sign is an open decision.

## 6. Out of scope

- **No absolute token limits presented as fact.** Anthropic does not publish them; they appear only as a labelled estimate.
- **No support for API-key billing.** Source A provides no limits there.
- **No modification of Claude or Claude Code** beyond the user-approved status line configuration; otherwise read-only.
- **No cloud service.** Everything runs locally; no data leaves the computer, except to Anthropic itself if source C is used.

## 7. Risks and open questions

| Topic | Risk / question | Handling |
|---|---|---|
| Data freshness | Without a running Claude Code session, source A provides nothing | show data age; evaluate source C as an option |
| Format changes | Transcript format (B) and endpoint (C) are undocumented | parser tolerant of missing fields; log the data source version |
| Security | Source C depends on the login credential | read only, never store or log it; owner decision |
| Several sessions or computers | Several Claude Code instances write in parallel; usage on other computers is invisible locally | use the most recent record; document the limitation |
| Technology choice | determines cross-compilation and window size | evaluation in the concept document |

## 8. Next steps

1. **Concept document:**
   - architecture (bridge, local data store, cockpit);
   - technology choice with evaluation;
   - visualisation design;
   - calculation formulas.
2. **Requirements** with unique IDs and testable acceptance criteria: first draft in [requirements.md](requirements.md), to be approved by the project owner.
3. **Owner decisions:**
   - source C yes or no;
   - code signing.
