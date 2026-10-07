# Concept

Status: draft, 2026-10-05. Implements the decisions in [ADR 0002](decisions/0002-technology-rust-egui.md) and covers the [requirements](requirements.md). Implementation issues refer to the section numbers of this document. Values marked *(proposal)* follow the requirements and change with them.

## 1. Overview

One executable, `usage-cockpit`, with two roles:

```
Claude Code ──stdin JSON──▶ usage-cockpit bridge ──▶ data directory ──▶ usage-cockpit (window)
   (status line command)      store record, print text   latest.json        reads, calculates, draws
                                                         history-v1.jsonl
~/.claude/projects/**/*.jsonl ───────────────────────────────────────────▶ transcript statistics
```

- The **bridge** runs for every status line update (debounced by Claude Code at 300 ms, cancelled if a newer update arrives). It must be fast and must never fail visibly (REQ-020, REQ-109).
- The **cockpit** is a long-running window. It polls the data directory, keeps periods and metrics in memory, and repaints once per second (REQ-010, REQ-105).
- All calculations live in a GUI-free library crate so they can be unit tested (REQ-110).

## 2. Repository layout

Cargo workspace, Rust edition 2024, stable toolchain pinned in `rust-toolchain.toml` (at least Rust 1.89, needed for `std::fs::File::try_lock`). The executable name is `usage-cockpit`; ADR 0002 uses the working name `cockpit` for the same thing.

```
Cargo.toml                     workspace: members = ["crates/core", "crates/app"]
rust-toolchain.toml            channel = "stable" (pinned version), components rustfmt, clippy
deny.toml                      cargo-deny: allowed licences (§12)
crates/core/                   package "cockpit-core" (library, no GUI dependencies)
  src/lib.rs
  src/model.rs                 §4 types
  src/parse.rs                 §4.1 status line JSON → Record
  src/store.rs                 §5 data directory, latest.json, history, locking, retention
  src/periods.rs               §6 period detection
  src/metrics.rs               §7 formulas
  src/planning.rs              §7.8 weekly planning
  src/transcripts.rs           §8 source B
  src/settings.rs              §9 settings file
  src/paths.rs                 §5.1 per-OS directories
  src/logging.rs               §10 rotating log
  src/format.rs                §11.5 durations, percentages, local times
  src/viewmodel.rs             §11.4 display model built from metrics (pure, testable)
  tests/                       integration tests, fixtures in tests/fixtures/
crates/app/                    package "usage-cockpit" (binary)
  build.rs                     §13 version and commit
  src/main.rs                  §3 command line dispatch
  src/bridge.rs                §5.3 bridge mode
  src/setup.rs                 §5.4 bridge setup and removal in Claude Code settings
  src/instance.rs              §11.6 single instance lock
  src/gui/mod.rs               eframe app, view switching, polling
  src/gui/compact.rs           §11.2
  src/gui/detailed.rs          §11.3
  src/gui/chart.rs             §11.3 history chart
  src/gui/settings_view.rs     §11.3 settings dialog
  src/gui/theme.rs             §11.1 colours, glyphs
  tests/                       command line integration tests (assert_cmd)
docs/user-guide.md             REQ-112
```

Every source file starts with `// SPDX-License-Identifier: Apache-2.0` and no copyright line until the owner has decided the author line format.

**Dependencies (initial set, all MIT and/or Apache-2.0 unless noted):** `serde`, `serde_json` (feature `preserve_order`), `toml`, `directories`, `clap` (derive), `log`, `chrono` (feature `clock`), `eframe`, `egui`, `egui_plot`, `winit` (same version as used by `eframe`, feature `x11`, for the X11 backend selection), `windows-sys` (Windows only, `Win32_System_Console`), `anyhow` (app only), `thiserror` (core), dev: `assert_cmd`, `predicates`, `tempfile`, `chrono-tz`. New dependencies need a reason in the PR and must pass `cargo deny check licenses`.

## 3. Command line

| Invocation | Behaviour | REQ |
|---|---|---|
| `usage-cockpit` | starts the window | REQ-018, REQ-029 |
| `usage-cockpit bridge` | bridge mode (§5.3) | REQ-020, REQ-109 |
| `usage-cockpit setup-bridge [--yes]` | adds the bridge to Claude Code settings after consent (§5.4) | REQ-023 |
| `usage-cockpit remove-bridge [--yes]` | restores the previous status line (§5.4) | REQ-023 |
| `usage-cockpit --version` | prints `usage-cockpit <version> (<commit>)`, exit 0 | REQ-032 |

Parsing with `clap`. Exit codes: 0 success; 1 user declined or invalid use; 2 error. The bridge always exits 0.

On Windows, `setup-bridge` and `remove-bridge` from the command line require `--yes`: after `AttachConsole` the program shares the console input with the parent shell, so an interactive `[y/N]` prompt is unreliable there. Without `--yes` they print `On Windows use --yes, or set up the bridge from the cockpit window.` and exit 1. On macOS and Linux the prompt is used.

Windows: the binary uses `#![windows_subsystem = "windows"]`. For `--version`, `setup-bridge` and `remove-bridge` the program calls `AttachConsole(ATTACH_PARENT_PROCESS)` (crate `windows-sys`) before printing. Bridge mode writes to the inherited standard output pipe and needs no console.

## 4. Data model

```rust
pub enum WindowKind { FiveHour, SevenDay }

pub struct WindowSample {          // one window inside one record
    pub used_pct: f64,             // 0.0..=100.0 (values above 100 clamp to 100 for display)
    pub resets_at: i64,            // Unix seconds
}

pub struct Record {                // what the bridge stores
    pub received_at_ms: i64,       // bridge clock, Unix milliseconds
    pub session_id: Option<String>,
    pub cc_version: Option<String>,
    pub five_hour: Option<WindowSample>,
    pub seven_day: Option<WindowSample>,
    pub model: Option<String>,     // model.display_name
    pub context_used_pct: Option<f64>,
    pub cost_usd: Option<f64>,     // cost.total_cost_usd
}
```

Window lengths: five-hour 18,000 s, seven-day 604,800 s. The period start is `resets_at − length`.

### 4.1 Parsing the status line JSON

Input fields (official documentation): `session_id`, `version`, `model.display_name`, `context_window.used_percentage`, `cost.total_cost_usd`, `rate_limits.five_hour.{used_percentage,resets_at}`, `rate_limits.seven_day.{used_percentage,resets_at}`.

- Every field is optional. A window is taken only if both `used_percentage` (number) and `resets_at` (integer) are present and `used_percentage` is finite and ≥ 0.
- Unknown fields are ignored; other fields of the input are never stored or logged (REQ-031).
- Invalid JSON returns `ParseError`; the bridge logs one line and stores nothing (REQ-108).
- `rate_limits.spend_limit` is ignored (out of scope, API-key and gateway billing).

## 5. Data directory and bridge

### 5.1 Directories

Using `directories::ProjectDirs::from("", "", "usage-cockpit")`:

| | data (`data_local_dir`) | config (`config_dir`) |
|---|---|---|
| Windows | `%LOCALAPPDATA%\usage-cockpit\data` | `%APPDATA%\usage-cockpit\config` |
| macOS | `~/Library/Application Support/usage-cockpit` | same |
| Linux | `$XDG_DATA_HOME/usage-cockpit` (default `~/.local/share/usage-cockpit`) | `$XDG_CONFIG_HOME/usage-cockpit` |

Environment variable `USAGE_COCKPIT_HOME` overrides both (data in `<home>/data`, config in `<home>/config`); tests use it.

### 5.2 Files

| File | Format | Written by | REQ |
|---|---|---|---|
| `latest.json` | one JSON object `{"v":1, "record":{...}}` | bridge, atomically: write a uniquely named temp file `latest.json.<pid>.<nanos>.tmp`, then rename onto `latest.json` | REQ-020 |
| `last_error.json` | `{"v":1, "received_at_ms":..., "kind":"malformed"}` | bridge, atomically, when input could not be parsed | REQ-108 |
| `history-v1.jsonl` | one JSON object per line `{"v":1, ...record fields}` | bridge, append under lock | REQ-013 |
| `history.lock` | empty lock file | bridge and cockpit, exclusive lock while appending or pruning | REQ-020 |
| `cockpit.lock` | empty lock file | cockpit, held while running | REQ-033 |
| `log.txt`, `log.1.txt`, `log.lock` | text log; rotation only while holding `log.lock` (`try_lock`; if taken, skip rotating this time) | both | REQ-031 |

Record field names in JSON: `received_at_ms`, `session_id`, `cc_version`, `five_hour` / `seven_day` as `{"used_pct":..,"resets_at":..}` or absent, `model`, `context_used_pct`, `cost_usd`. Every file carries `"v"` (REQ-115). Readers accept every `v` they know and skip lines with an unknown `v` (logged once).

Locking uses `std::fs::File::try_lock` in a retry loop (sleep 10 ms) until the timeout.

- **Rename on Windows:** `std::fs::rename` replaces an existing target; if it fails with `PermissionDenied` (target briefly open by a reader), retry up to 5 times with 20 ms pauses, then give up and log. The cockpit deletes leftover `*.tmp` files older than one minute at start.
- **Appending:** a bridge that Claude Code killed may have left a partial last line. Before appending, the writer checks the last byte of the file and writes a `\n` first if it is not a newline.
- **Reading:** readers need no lock. They skip every line that does not parse (a partial line can therefore also sit in the middle of the file) and log the number of skipped lines once.

### 5.3 Bridge mode

Steps, in this order:

1. Read standard input completely (limit 1 MiB).
2. Parse (§4.1). On success: read the previous `latest.json` to compare `cc_version` (log one line if it changed), write `latest.json` atomically, append one history line under `history.lock` (lock wait at most 200 ms, then skip the append and log). On a parse error: write `last_error.json` and log one line without the input.
3. If a previous status line command is stored in `bridge-state.json` (§5.4), run it through the same shell kind Claude Code would use (§5.5) with the original input on its standard input, timeout 1 s *(proposal)*; if it exits 0 within the timeout, print its standard output unchanged and exit.
4. Otherwise print the bridge's own text: `5h 23.5% · 7d 41.2%`, with `–` for a missing window; with a parse error print `usage-cockpit: no data`.
5. Exit 0 in every case (REQ-109). All errors go to the log only.

Speed: steps 1, 2 and 4 must take under 100 ms (REQ-109); the bridge does not initialise any GUI code.

### 5.4 Bridge setup and removal

Claude Code settings file: `~/.claude/settings.json` (Windows: `%USERPROFILE%\.claude\settings.json`); `CLAUDE_CONFIG_DIR` overrides `~/.claude` if set.

The replaced status line is kept in `<config dir>/bridge-state.json` (`{"v":1, "previous_status_line": <object or null>}`), written atomically and **only** by setup and removal, never by the cockpit window, so a running cockpit cannot overwrite it.

`setup-bridge`:

1. Determine the absolute path of the running executable. Convert to forward slashes on Windows. If the path contains a space: on Windows use the 8.3 short name of the folder (no space, accepted by every shell), or, if the volume has none, put the whole path in double quotes (Git Bash and cmd accept that; PowerShell does not); elsewhere stop with a message asking the user to place the executable in a folder without spaces until the quoting for `sh -c` is done.
2. Show what will change and ask `Change Claude Code settings? [y/N]` (skipped with `--yes`; the GUI shows the same text in a dialog). Declining leaves the file byte-identical, exit 1.
3. If the settings file exists, copy it to `settings.json.usage-cockpit-backup-<YYYYMMDD-HHMMSS>`.
4. If a `statusLine` object exists and is not already the bridge, store it in `bridge-state.json`.
5. Set `statusLine` to `{"type":"command","command":"<path> bridge"}`; keep all other keys and their order (`serde_json` with `preserve_order`); write atomically.
6. If the file does not exist, create it with only the `statusLine` key.

`remove-bridge`: after consent, restore `statusLine` from `bridge-state.json` or remove the key if there was none; set the stored value to `null`; backup first, write atomically. If the current `statusLine` is not the bridge, change nothing and say so.

The user guide notes that a `statusLine` in project-level Claude Code settings takes precedence over the user-level one, so the bridge does not run in such projects.

### 5.5 Running the kept user command

Unix: `sh -c <command>`. Windows: Git for Windows bash, searched in this order: `CLAUDE_CODE_GIT_BASH_PATH` if set; `%ProgramFiles%\Git\bin\bash.exe`; `%ProgramFiles(x86)%\Git\bin\bash.exe`; `%LOCALAPPDATA%\Programs\Git\bin\bash.exe`; `<directory of git.exe found on PATH>\..\bin\bash.exe`. Never use a `bash.exe` from `C:\Windows\System32` (that is WSL). If none is found: `powershell -NoProfile -Command <command>` (mirrors Claude Code's behaviour).

Running with timeout (std has no wait-with-timeout): spawn with piped stdin and stdout; write the input from a separate thread and close stdin; read stdout in another thread that sends the result over a channel; poll `try_wait` every 10 ms until the deadline; on timeout `kill` the child and use the fallback text without waiting for the reader thread (a grandchild may keep the pipe open).

## 6. Periods (REQ-022)

For each window kind, records are grouped into periods in received order:

- A record continues the current period if `|resets_at − period's last resets_at| ≤ 600` s *(proposal)* **and** its receive time is before the period's last `resets_at`. Receive times are converted with `received_at_ms.div_euclid(1000)` before comparing with `resets_at`.
- Otherwise it starts a new period.
- A period's reset time is the `resets_at` of its latest record.

The **current period** is the latest period whose reset time is in the future. If the latest record's reset time has passed, there is no current period and the window shows "reset, waiting for new data".

## 7. Formulas

Notation for one window: `u` used %, clamped to 0…100 in every calculation, `R` reset time (s), `L` window length (s), `now` (s), `tol` tolerance band in percentage points (default 5), `P` rate period (default 1,800 s).

### 7.1 Basic values (REQ-001, REQ-002)

- remaining = `100 − u`
- time to reset = `max(0, R − now)`

### 7.2 Target and pace (REQ-003, REQ-004)

- elapsed share `e = clamp((now − (R − L)) / L, 0, 1)`; target `t = 100·e`
- deviation `d = u − t` (pp)
- pace factor `f = u / t` if `t > 0`, else undefined
- state: `d < −tol` → Under; `d > tol` → Over; otherwise On

Check: e = 0.5, u = 30 → d = −20 → Under; u = 52 → On; u = 70 → Over. u = 60, e = 0.4 → +20 pp, f = 1.5.

### 7.3 Usage rate (REQ-021)

Samples: the records of the current period with `received_at ≥ now − P`. With at least two samples at distinct times, rate `r` = least-squares slope of `used_pct` over time, in % per hour. Otherwise not available. A negative slope is reported as 0.

Check: (10:00, 40), (10:15, 45), (10:30, 50) → r = 20 %/h.

### 7.4 Exhaustion forecast (REQ-005)

In this order: `u ≥ 100` → "limit reached"; `r` not available → "not available"; `r = 0` → "reset first"; otherwise `T100 = now + (100 − u) / r · 3600` and "limit first" if `T100 < R`, else "reset first". Check: u = 40, r = 20 → 3 h.

### 7.5 Projected unused remainder (REQ-006)

If `r` available: `max(0, 100 − (u + r · max(0, R − now) / 3600))`. Check: 40 + 5·4 = 60 → 40 %.

### 7.6 Recommended rate (REQ-007)

If `R > now`: `(100 − u) / ((R − now) / 3600)` %/h, else undefined. Check: 40 % remaining, 4 h → 10 %/h.

### 7.7 Binding limit (REQ-008)

For each window with "limit first", time to exhaustion `T100`; a window with "limit reached" counts as exhausted now. The binding limit is the window with the earlier `T100`; if neither reaches its limit before its reset, there is none. If only one window has data, there is no comparison.

### 7.8 Weekly planning (REQ-027)

With both windows and `R7 > now`: `n = (R7 − now).div_ceil(18_000)` in integer arithmetic; share per window = `(100 − u7) / n` % of the weekly quota. If `R7 ≤ now`, no plan. Check: 50 h, 60 % remaining → n = 10, 6 %.

### 7.9 Data age and stale (REQ-009)

age = `now − received_at` of the latest record; stale if age > 600 s *(proposal, setting)*, **or** if `last_error.json` is newer than the latest record (a malformed record arrived; REQ-108 asks for the marker right away). The displayed values stay, marked stale.

### 7.10 Tokens per percentage point (REQ-015)

For the current five-hour period: tokens (input + output + cache creation + cache read) from §8 with timestamps from the receive time of the period's first record until now ÷ (`u` now − `u` of that first record). Shown only if the denominator ≥ 1 pp and tokens > 0, always labelled "estimate".

## 8. Transcript statistics (REQ-014)

Location: `<claude dir>/projects/**/*.jsonl` (claude dir as in §5.4). Format undocumented; parsing is tolerant.

- Use lines with `"type":"assistant"` that contain `message.usage`.
- Fields: `timestamp` (RFC 3339), `message.model`, `message.id`, `requestId`, `message.usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens}` (missing counts as 0).
- Deduplicate by (`message.id`, `requestId`); keep the last occurrence.
- Aggregates: per model and per local calendar day: input, output, cache creation, cache read. Cache share = (cache creation + cache read) ÷ (input + cache creation + cache read).
- Incremental: remember per file the byte offset and modification time; rescan a file whose size shrank. Scan at start and every 60 s in a background thread; only files modified within the retention period (REQ-025).
- If no line could be understood in any file, the view shows "transcript statistics not available".

## 9. Settings (REQ-024)

`<config dir>/settings.toml` (§5.1):

```toml
version = 1
tolerance_pp = 5.0          # REQ-003
stale_after_s = 600         # REQ-009
rate_period_s = 1800        # REQ-021
always_on_top = true        # REQ-018
start_view = "compact"      # "compact" | "detailed"
[window]                    # REQ-018, written on exit and after moving/resizing (debounced 1 s)
x = 100.0
y = 100.0
width = 320.0
height = 120.0
```

The replaced Claude Code status line is not stored here but in `bridge-state.json` (§5.4).

Missing file → defaults; invalid file → defaults, the invalid file is renamed to `settings.toml.invalid` and one log line is written. Values out of range (e.g. negative tolerance) fall back to the default for that key.

## 10. Logging (REQ-031)

`log` facade with a small logger in `cockpit-core::logging`: line format `2026-10-05T14:03:22Z WARN bridge: <message>`; when `log.txt` exceeds 5 MB it is renamed to `log.1.txt` (replacing an older one) while holding `log.lock`. Logged: start, version, errors, malformed input (without content), settings fallbacks, setup/removal, changes of `cc_version`. Never logged: record contents beyond used fields, environment variables, settings file contents of Claude Code.

## 11. User interface

### 11.1 Visual language (REQ-003, REQ-106, REQ-114)

| State | Glyph | Text | Colour (Okabe–Ito) |
|---|---|---|---|
| Under pace | ▼ | `under` | blue `#0072B2` |
| On pace | ● | `on pace` | bluish green `#009E73` |
| Over pace | ▲ | `over` | vermillion `#D55E00` |
| Stale | ⏸ plus dashed outline | `stale 14 min` | grey `#7F7F7F` |
| No data | ○ | plain-language reason | grey |
| Binding limit | ⚑ next to the window label | `binding` | as the window's state |

Every state has glyph and text, so colour is never the only carrier. Both dark and light themes use the same hues.

### 11.2 Compact view (REQ-029)

Default size 320 × 120 logical px, always on top by default, no window decorations beyond the system title bar.

```
5h ⚑ [██████████░░░░░░|░░░░░░░]  ▲ over   62.0%  1h 12m
7d   [████░░░░|░░░░░░░░░░░░░░░]  ▼ under  21.0%  3d 4h
updated 12 s ago                                   ⤢
```

- The bar shows used % (filled) and the target `t` as a vertical marker.
- Bottom line: data age, or the stale marker; the ⤢ button (or key `D`) switches to the detailed view.

### 11.3 Detailed view

Default 520 × 640 px, scrollable:

1. Per window: used, remaining, reset (local time and countdown), target, deviation, pace factor, rate, forecast, projected unused remainder, recommended rate (§7).
2. Binding limit and weekly planning (§7.7, §7.8).
3. History chart per window (`egui_plot`): points of the current period, target line from (period start, 0 %) to (reset, 100 %), vertical line at now (REQ-030).
4. Session details from the latest record, under the heading "From Claude Code": model, context usage, session cost (REQ-028).
5. Transcript statistics: table per model, daily totals for the retained history (up to 35 days, scrollable), cache share, tokens per percentage point labelled "estimate" (REQ-014, REQ-015).
6. Previous periods: per window the last three finished periods with reset time and final used % (REQ-013).
7. Footer: version and commit, link to licence notices, buttons "Settings", "Set up bridge" / "Remove bridge", "Compact view".

Settings dialog: fields of §9 with validation; "Save" writes the file.

### 11.4 View model

`cockpit-core::viewmodel::build(&Inputs) -> ViewModel` returns all strings, numbers and states the views draw. The GUI code only lays out what the view model provides; all decisions are unit-tested there.

```rust
pub struct Inputs<'a, Tz: chrono::TimeZone> {
    pub now_ms: i64,                    // Unix milliseconds
    pub records: &'a [Record],          // full history, received order
    pub last_error_ms: Option<i64>,     // from last_error.json
    pub load_error: Option<&'a str>,    // e.g. data directory unreadable (§11.7)
    pub settings: &'a Settings,
    pub stats: Option<&'a Stats>,       // transcript statistics (§8)
    pub tz: &'a Tz,                     // chrono::Local in the app, a fixed zone in tests
}
```

The exact `ViewModel` structure is defined in the view model issues; it contains no egui types.

### 11.5 Formatting

- Percentages always with one decimal (`4.5%`, `23.5%`, `62.0%`), rounded half away from zero.
- Durations: `< 1 h` → `42m`; `< 24 h` → `1h 12m`; otherwise `3d 4h`; negative → `0m`.
- Local times in the zone passed in (`chrono::Local` in the app), format `Mon 14:30`; durations always from absolute timestamps (REQ-026).
- Data age: `updated 12 s ago` (< 60 s), `updated 4 min ago` (< 60 min), otherwise `updated 2h 5m ago`; when stale: `stale, 14 min old` (same duration rules).
- Metric texts: deviation `+20.0 pp` / `−3.5 pp`; pace factor `1.50×` or `–` when undefined; rates `20.0 %/h` or `not available`; forecast `limit in 3h 0m, before reset` / `reset first` / `limit reached` / `not available`; projected unused `40.0% unused at reset`; recommended `10.0 %/h`; weekly plan `10 windows left · 6.0% per window`.

### 11.6 Platform behaviour

- **Single instance (REQ-033):** the cockpit takes `cockpit.lock` with `try_lock`; if taken, it logs the fact and shows a small window with the text "usage-cockpit is already running." and an OK button (a GUI application has no visible console), then exits 0.
- **Linux:** if `DISPLAY` is set, the event loop is created with the X11 backend (`winit` `EventLoopBuilderExtX11::with_x11` through `eframe::NativeOptions::event_loop_builder`), so always-on-top and position restore work under XWayland; otherwise Wayland with the documented limitation.
- **Scaling (REQ-113):** egui uses logical pixels and the system scale factor; no fixed-pixel images.
- **Repaint:** `ctx.request_repaint_after(1 s)`; file polling every 500 ms on a background thread that sends new records over a channel.

### 11.7 No-data reasons (REQ-016)

Instead of zero values the window shows one of these texts (decided in the view model):

| Situation | Text |
|---|---|
| No `latest.json` yet | `No data yet. Set up the bridge and use Claude Code once.` plus the "Set up bridge" button |
| Latest record has neither window | `Claude Code sent no usage limits. They appear only for Pro and Max subscriptions, after the first response of a session.` |
| One window missing | that row shows `no data` |
| Reset time passed, no newer record | `Window reset. Waiting for new data from Claude Code.` |
| Data directory unreadable | `Cannot read the data folder: <path>` |

## 12. Licences (REQ-107)

`cargo deny check licenses` in CI with `deny.toml` allowing: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, OFL-1.1, Ubuntu-font-1.0, MPL-2.0 (file-level copyleft, unmodified use). `THIRD_PARTY_LICENSES.md` is generated with `cargo about` (or a script over `cargo metadata`) and committed for each release.

## 13. Versioning

Version from `crates/app/Cargo.toml` (`0.x.y` until the first release). `build.rs` sets `GIT_COMMIT` from `git rev-parse --short HEAD`, or `unknown` when git is unavailable.

## 14. Tests and CI

- Unit tests next to the code; integration tests in `crates/*/tests`. Test names start with the REQ-ID in lower case, e.g. `req_021_rate_from_three_samples` (REQ-110).
- Fixtures: status line JSON samples and transcript samples in `crates/core/tests/fixtures/`, synthetic, no real data.
- CI job `test` (the required check keeps this name) on `ubuntu-24.04`, `ubuntu-24.04-arm`, `windows-latest`, `macos-latest`: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`, plus the existing project-file and requirement-ID checks. `cargo deny check licenses` once on Linux.
- Release workflow on a tag `v*` set by the owner: builds the four executables (§ADR 0002 runners, Linux inside `debian:bookworm`), names them `usage-cockpit-<version>-<target>[.exe]`, writes `SHA256SUMS`, creates a draft release (REQ-111). Workflow changes need the owner's approval.
- Measurements (REQ-105, REQ-109, REQ-116) run as scripts in `scripts/` and are executed by the owner on real hardware where CI cannot measure.

## 15. Requirement coverage

| Section | Requirements |
|---|---|
| §3 | REQ-018, REQ-023, REQ-029, REQ-032 |
| §4–§5 | REQ-011, REQ-012, REQ-013, REQ-017, REQ-020, REQ-023, REQ-025, REQ-101, REQ-103, REQ-104, REQ-108, REQ-109, REQ-115 |
| §6–§7 | REQ-001 to REQ-009, REQ-015, REQ-021, REQ-022, REQ-027 |
| §8 | REQ-014 |
| §9–§10 | REQ-024, REQ-031 |
| §11 | REQ-003, REQ-010, REQ-016, REQ-018, REQ-026, REQ-028, REQ-029, REQ-030, REQ-033, REQ-106, REQ-113, REQ-114, REQ-116 |
| §12–§14 | REQ-102, REQ-105, REQ-107, REQ-110, REQ-111, REQ-112 |
| not covered | REQ-019 (owner decision pending) |

Retention (REQ-025): the cockpit prunes `history-v1.jsonl` at start and every hour under `history.lock`, keeping records of the last 35 days; if the file is still larger than 50 MB, the oldest lines are dropped until it is below 45 MB. Several sessions (REQ-017): the latest record by `received_at_ms` wins for the displayed values; all records go into the history. No network access anywhere (REQ-103).
