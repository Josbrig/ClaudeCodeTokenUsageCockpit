# Measurements

Three requirements are checked by measuring on real machines: the speed of the bridge (REQ-109), the load of the idle cockpit (REQ-105) and the start-up time (REQ-116). The scripts in `scripts/` do the measuring; this page says how to run them and holds the table for the results.

| Requirement | Limit | Script |
|---|---|---|
| REQ-109 Bridge speed | median of 100 calls below 100 ms | `measure-bridge.ps1` |
| REQ-105 Resource use | below 1 % CPU and below 100 MB memory while idle, over 10 minutes | `measure-idle.ps1` |
| REQ-116 Start-up time | window within 2 s, over 5 starts | `measure-startup.ps1` |

The limits are the proposals of the requirements. The scripts for Linux and macOS (`.sh`) are planned in their own issue; until then only the PowerShell scripts for Windows exist.

## How to run (Windows)

1. Build the release program: `cargo build --release`. Measure the release build only; the debug build is much slower and a script warns when it falls back to it.
2. Close other busy programs, plug in the power on a laptop, and leave the machine alone while a measurement runs.
3. In a PowerShell window in the repository folder:

```
powershell -File scripts\measure-bridge.ps1
powershell -File scripts\measure-startup.ps1
powershell -File scripts\measure-idle.ps1
```

Each script takes `-Exe <path>` to measure another build. `measure-bridge.ps1` has `-Runs`, `measure-startup.ps1` has `-Starts` and `-TimeoutSeconds`, `measure-idle.ps1` has `-Seconds` (default 600; use a smaller value only for a trial).

All scripts work in a new temporary folder (`USAGE_COCKPIT_HOME`), so your real data and settings are not touched and nothing is left behind. The start-up and idle scripts open the cockpit window; do not use it during the measurement.

## What the numbers mean

- **Bridge:** the time of a whole call from a script includes the start of the process, which belongs to the operating system and not to the bridge. The first number is the literal measurement of REQ-109 (median of 100 whole calls). The script also runs the same program with `--version` (it starts, prints one line and ends) the same way; the difference shows what the bridge's own work adds to the bare start of the program. Note both numbers in the table.
- **Idle:** CPU is the share of **one** core (a program that keeps one core busy shows 100 %), sampled every second after 15 seconds of settling; memory is the working set. The script prints averages and maxima; the limits apply to the averages.
- **Start-up:** the time from starting the process until the program writes the log line `window ready` after drawing its first frame. The script polls the log file every 20 ms.

## Results

Fill in one row per platform and build. Date, version and commit come from `usage-cockpit --version`.

| Platform | Date | Version (commit) | Bridge: median of 100 calls / same program with `--version` | Idle: CPU avg / memory avg | Start-up: max of 5 | Measured by |
|---|---|---|---|---|---|---|
| Windows 11 x64 (developer machine, see below) | 2026-10-07 | 0.1.0 (build of the day) | 101.3 ms and 99.8 ms (two runs) / `--version` 100.3 ms and 97.7 ms | 0.23 % / 88.0 MB (10 min) | 621 ms | the developer's assistant, release build |
| Linux x64 | | | | | | |
| Linux arm64 (Raspberry Pi) | | | | | | |
| macOS Apple Silicon | | | | | | |
| Windows x64 (reference machine of the owner) | | | | | | |

The Windows row was measured on the machine the program was developed on (release build, normal desktop load: 100 bridge calls, 5 starts, 10 minutes idle). It does not replace the owner's measurement on the reference machine.

### Notes on the Windows row

- **Bridge:** two runs of 100 calls gave medians of 101.3 ms and 99.8 ms (95th percentile 105.3 and 106.5 ms). The same program started with `--version` took 100.3 ms and 97.7 ms, so the bridge's own work adds about 1 to 2 ms; an empty `cmd.exe /c exit` took 83 to 91 ms in earlier trials. Read literally, the criterion (median below 100 ms) is **borderline on this machine: it is missed by about 1 ms in one run and met by 0.2 ms in the other**. Almost all of the time is the start of the program on Windows (loading a 6.5 MB executable, security software), not the work of the bridge. Whether the requirement is meant for the whole call or for the bridge's own processing, and whether a smaller separate executable for the bridge is wanted, is for the owner to decide.
- **Idle, 10 minutes:** CPU average 0.23 % of one core (maximum of a single second 14.0 %), memory average 88.0 MB (maximum 92.1 MB). Both are below the limits, the memory with little room. In this run the cockpit also read the real Claude Code transcripts of this machine (209 files, 811 MB in total, of which the files of the last 35 days count); an extra run of 90 seconds with an empty Claude folder gave 76.4 MB memory and 0.67 % CPU on average (that run was short and its CPU figure was disturbed by single busy seconds). So the transcript statistics cost roughly 12 MB here, and a machine with much more transcript text will need more. This is a point to watch against the 100 MB limit.
- **Start-up:** 599, 588, 591, 618 and 621 ms until the line `window ready` (written after the first frame has been drawn), so about 0.6 s against the 2 s limit.
