# Development guide

How to develop the program by hand, without an AI assistant: set up the tools, understand the code, make a change, try it, test it and hand it in. Everything here was tried on Windows; places for Linux and macOS say when they were not.

## Contents

1. [Setting up the tools](#1-setting-up-the-tools)
2. [Building with CMake](#2-building-with-cmake)
3. [Getting around](#3-getting-around)
4. [Making a change, step by step](#4-making-a-change-step-by-step)
5. [Trying a change without touching real data](#5-trying-a-change-without-touching-real-data)
6. [Checks and handing a change in](#6-checks-and-handing-a-change-in)
7. [Rules of the repository](#7-rules-of-the-repository)
8. [Debugging](#8-debugging)
9. [What is unfinished or untried](#9-what-is-unfinished-or-untried)

## 1. Setting up the tools

### 1.1 What is needed

| Tool | Why |
|---|---|
| **Rust**, the toolchain pinned in `rust-toolchain.toml`, with `rustfmt` and `clippy` | builds and checks the program (installed with `rustup`) |
| A **C++ toolchain** that the Rust linker uses | on Windows the Microsoft C++ Build Tools (MSVC); on Linux `build-essential`; on macOS the Xcode Command Line Tools |
| **Git** | the repository; on Windows its `bash` is also used by some tests |
| **CMake** and **Ninja** | the CMake builds (section 2); not needed for plain `cargo` |
| On Linux: the development packages of the window toolkit (X11/Wayland, xkbcommon, OpenGL) and `pkg-config` | the window is built with them (not tried yet) |

### 1.2 Windows

The script `scripts/setup-dev-windows.ps1` looks at what is there, says what is missing, and installs only that, after asking:

```
powershell -File scripts\setup-dev-windows.ps1 -CheckOnly   # report only, installs nothing
powershell -File scripts\setup-dev-windows.ps1              # report, ask, install what is missing, build once
```

- `-CheckOnly` installs nothing and exits with 1 if something is missing. The exit code is also 1 when you decline the question or no question can be asked, 2 when an installation fails, and 0 when everything is there or was installed.
- Without it, the script lists what it would install and asks `Install the missing tools? [y/N]`. `-Yes` skips the question (for use in a script); in a place where no question can be asked (no terminal) and without `-Yes` it installs nothing.
- Programs are installed **for the current user** where the package allows it (`--scope user` is tried first). The **C++ Build Tools can only be installed for the whole computer**: a Windows prompt asks for administrator consent. rustup is installed without a default toolchain; the script then installs only the pinned one (this part was not tried: rustup was already installed on the machine where the script was tried).
- It uses `winget` for the programs (Git: `Git.Git`; CMake: `Kitware.CMake`; Ninja: `Ninja-build.Ninja`; the C++ Build Tools: `Microsoft.VisualStudio.2022.BuildTools` with the workload *Desktop development with C++*; rustup: `Rustlang.Rustup`) and `rustup` for the toolchain. If `winget` is missing it says which program to install by hand.
- It can be run again at any time; what is present is not touched.
- At the end it runs `cargo build` once to prove that the setup works (`-SkipBuild` leaves that out) and prints the commands to build and test.
- After an installation, open a **new terminal** so that the new tools are on the `PATH` of the programs you start. The script itself also looks at the `PATH` that Windows has stored, so running it again in the same terminal does not install a tool twice.

The pure parts of the script (reading the toolchain channel, the plan, the table, the `winget` arguments, the stored `PATH`) and its flow (exit codes, consent, which tools would be installed, with the installers replaced by fakes) have a test script that installs nothing: `powershell -File scripts\tests\setup-dev-windows.tests.ps1`. Tried for real: Ninja was installed by the script (for the user, exit code 0).

**By hand**, the same: install [rustup](https://rustup.rs) and run `rustup toolchain install <channel from rust-toolchain.toml> --profile minimal -c rustfmt -c clippy`; install the [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/) with the C++ workload; install Git, CMake and Ninja.

### 1.3 Linux and macOS

The scripts `scripts/setup-dev-linux.sh` and `scripts/setup-dev-macos.sh` are planned (#152, #153) and do not exist yet. Until then, by hand:

- **Linux** (Debian, Ubuntu, Raspberry Pi OS; not tried): `sudo apt install build-essential pkg-config cmake ninja-build git libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev`, then rustup as above.
- **macOS** (not tried): `xcode-select --install`, then rustup as above, and `brew install cmake ninja`.

### 1.4 Check that it works

In the repository folder:

```
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

All three must pass before a change is handed in. A first `cargo test --all` takes a few minutes (it builds the window toolkit); later runs are fast.

## 2. Building with CMake

`CMakeLists.txt` is a front end for the Rust workspace: it calls `cargo` and compiles nothing itself (the project has no language, so no C or C++ compiler is looked for). It gives the same commands on every system and produces the files of a release.

```
cmake -S . -B build
cmake --build build --config Release --target usage-cockpit   # cargo build --release
cmake --build build --target check                            # cargo fmt --check, cargo clippy -D warnings
ctest --test-dir build -C Release                             # fmt, clippy and the tests of the workspace
cmake --build build --config Release --target dist            # build/dist/usage-cockpit-<version>-<system>[.exe] and SHA256SUMS
```

| Target / command | What it does |
|---|---|
| `usage-cockpit` (the default) | `cargo build --release --locked` |
| `check` | `cargo fmt --all --check`, then `cargo clippy --all-targets -D warnings` |
| `ctest` (or the target `test`) | three tests: `cargo-fmt`, `cargo-clippy`, `cargo-test` (`cargo test --all`) |
| `dist` | builds, copies the program to `build/dist/usage-cockpit-<version>-<system>[.exe]` and records its SHA-256 in `build/dist/SHA256SUMS` (lines of other systems built into the same folder are kept) |

- **Version and system name.** The version is the one of `Cargo.toml` (`[workspace.package]`). The system name follows from the Rust target: `windows-x64` (`x86_64-pc-windows-msvc`), `linux-x64` (`x86_64-unknown-linux-gnu`), `linux-arm64` (`aarch64-unknown-linux-gnu`), `macos-arm64` (`aarch64-apple-darwin`). Another target is refused with a message.
- **Options.** `-DCOCKPIT_TARGET=<triple>` builds for another target than the host of `rustc`; `-DCOCKPIT_CARGO=<path>` uses a particular `cargo`. Without the options the `cargo` of the cargo bin folder of rustup is used (its `cargo` honours the toolchain of `rust-toolchain.toml`; the commands are run in the repository folder), else the one on the `PATH`. Configuring runs `rustc -vV`, which can make rustup download the pinned toolchain the first time.
- **A target that is not the host.** `-DCOCKPIT_TARGET=<triple>` needs that target installed (`rustup target add <triple>`) and a linker for it. The tests of such a target cannot run on this computer: `cargo-test` then only compiles them (`--no-run`) and says so when configuring.
- **Where things go.** Everything cargo builds goes to `build/cargo-target`, so a CMake build never mixes with the `target/` folder of plain `cargo` runs. `build/` is ignored by git.
- **Not inside the source tree.** CMake refuses a build folder that is the source folder (CMake has written its `CMakeCache.txt` and `CMakeFiles` there by then: delete them).
- **Generators.** Any generator is meant to work. Tried on Windows: Visual Studio 17 2022 (with `--config Release`) and Ninja (`cmake -S . -B build -G Ninja`, then `cmake --build build --target dist`, which worked with the same result). NMake and Make were not tried.
- **Windows test.** `scripts\tests\cmake-build.tests.ps1` configures into a temporary folder, builds `dist`, checks the file name, the checksum, that the program in `dist` starts, that a second run keeps one line per file and the lines of other systems, that an unsupported target and an in-source build are refused, and that `target\` was not touched (`-RunTests` also runs `ctest`). The first run builds the whole program and takes a few minutes.
- **Linux and macOS.** The same files are meant to work there (#155, #156, #157); they were **not tried** on those systems yet.
- **Releases.** The release workflow (#57) is separate (it changes workflow files and needs the owner's go-ahead); it can use these files later.

## 3. Getting around

**What to read first**, in this order:

1. [README.md](../README.md) and the [user guide](user-guide.md): what the program does and how it looks. Use it for ten minutes before you read code.
2. [technical-documentation.md](technical-documentation.md): how it works as built. Section 1 (data flow) and section 2 (the modules) are the map.
3. [concept.md](concept.md): the design the code grew from, with the formulas and the reasons.
4. [requirements.md](requirements.md) and [traceability.md](traceability.md): what must hold, and which issue, pull request and test belongs to each requirement.

**The way of the data in short.** Claude Code starts `usage-cockpit bridge` after each response and hands it a JSON record on standard input. The bridge stores `latest.json` and appends to `history-v1.jsonl` in the data folder and prints a short text. The window polls `latest.json`, reads the history once at start, and builds a *view model* (`cockpit-core::viewmodel::build`): every string, number and state that is drawn. The views only lay the view model out. The token statistics come from a second source, the transcript files of Claude Code, scanned in a thread. Details and files: [technical documentation](technical-documentation.md), sections 1 and 3 to 6.

**Where things are.**

```
crates/core/src/        calculations, file formats, view model: no window, testable everywhere
crates/core/tests/      tests of those (one file per topic)
crates/app/src/         command line, bridge, setup, autostart, uninstall, the window (gui/)
crates/app/tests/       tests with the real executable
docs/                   requirements, concept, user guide, technical documentation, this guide
scripts/                measurement scripts, setup scripts and their tests
cmake/, CMakeLists.txt  the CMake front end
```

**A rule that shapes the code:** everything that decides something lives in `cockpit-core` as plain functions with the time and the files passed in, so it can be tested without a window. The window code (`gui/`) lays out what the view model says and nothing more. The bridge never starts any window code.

## 4. Making a change, step by step

1. **An issue.** One issue describes one thing that can be closed. Use the issue form (goal, acceptance criteria as a testable list, requirement ids, estimate). Work for several systems becomes one issue per system.
2. **A branch.** `feature/<issue>-<short-name>`, `fix/<issue>-<short-name>` or `docs/<issue>-<short-name>`, made from `develop`. Never work on `main`.
3. **A requirement, if the behaviour is new.** Add `### REQ-nnn Title` to `docs/requirements.md` with a statement, an acceptance test, a type and `Status: draft`. The next free number is used; numbers are never reused; only the project owner sets `approved`, and changing an approved requirement is a decision of the owner.
4. **The test first.** Write the test that fails without your change. Its name contains the requirement id (`req_024_...`), which is how [traceability.md](traceability.md) is kept. Calculations get unit tests with a worked number (see the examples in `docs/concept.md`), files and the window get tests with temporary folders.
5. **The code.** Put what decides into `cockpit-core`, what lays out into `gui/`. Keep to the style of the neighbours: short functions, the doc comment says why, errors are reported in words.
6. **The documents.** The [user guide](user-guide.md) (a test breaks if a message or a label of the window is not explained there), the [technical documentation](technical-documentation.md) if the way something works changes, [concept.md](concept.md) if the design changes, [CHANGELOG.md](../CHANGELOG.md) for everything a person can see, [traceability.md](traceability.md) for the requirement.
7. **The checks** of section 6, then a commit with a message in the form of [Conventional Commits](https://www.conventionalcommits.org/) and the footer `Refs #<issue>`, then the pull request.

### 4.1 Worked example: a value in the window

Take the line *Unused at reset* of the detailed view and follow it through the code:

| Step | Where |
|---|---|
| The calculation, a pure function | `metrics::unused_at_reset` in `crates/core/src/metrics.rs`; its tests in `crates/core/tests/metrics_forecast.rs` (a worked number: 40 % used, rate 5 %/h, 4 h left gives 40 % unused) |
| The text, one function | `format::unused` in `crates/core/src/format.rs`; tests in `crates/core/tests/format.rs` |
| Putting it in the view model | `window_view` in `crates/core/src/viewmodel.rs` calls both and stores the text as `WindowData.unused_text`; tests in `crates/core/tests/viewmodel_window.rs` |
| Drawing it | `data_grid` in `crates/app/src/gui/detailed.rs`: `row(ui, "Unused at reset", &data.unused_text)` |
| Explaining it | a row in the table of values of `docs/user-guide.md`, with an example text produced by `format::unused`; the tests `crates/core/tests/user_guide.rs` (the example) and `crates/app/tests/user_guide_labels.rs` (the label) fail if it is missing |

A new value goes the same way, from the inside out: the pure function with its test, the text, the field in `WindowData`, the row, the sentence in the guide.

### 4.2 Worked example: a new setting

Take the *rate period* (how many seconds the usage rate is measured over):

| Step | Where |
|---|---|
| The field, its default and its allowed range | `Settings.rate_period_s`, `RATE_PERIOD_RANGE` and the loading code with `int_in(...)` in `crates/core/src/settings.rs` (a missing, wrongly typed or out-of-range value falls back to the default); tests in `crates/core/tests/settings.rs` |
| Using it | `window_view` in `viewmodel.rs` passes `settings.rate_period_s` to `metrics::rate`; tests in `crates/core/tests/metrics_rate.rs` |
| Editing it | `Draft`, `Values::apply_to`, `Draft::validate` and the row in `show` in `crates/app/src/gui/settings_view.rs`; its tests check the ranges and the message |
| Applying it at once | `apply_values` in `crates/app/src/gui/mod.rs` writes the file and gives the new settings to the state, which rebuilds the view model |
| Documenting it | the table of settings in `docs/user-guide.md` (range and default), the settings block in `docs/concept.md` section 9, the requirement REQ-024 |

## 5. Trying a change without touching real data

Two environment variables send everything to folders you choose, so your real data and your real Claude Code settings are never touched:

- `USAGE_COCKPIT_HOME` for the data and configuration folders of the program (`<home>/data`, `<home>/config`; the start entry of the system gets another name when it is set);
- `CLAUDE_CONFIG_DIR` for the folder with the Claude Code settings and transcripts.

**Feed the bridge a record by hand** (Windows PowerShell; the times must lie in the future):

```
$env:USAGE_COCKPIT_HOME = "$env:TEMP\cockpit-try"
$env:CLAUDE_CONFIG_DIR  = "$env:TEMP\cockpit-try\claude"
$now = [DateTimeOffset]::UtcNow
$record = '{"session_id":"s","rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":' + $now.AddHours(3).ToUnixTimeSeconds() + '},"seven_day":{"used_percentage":41.2,"resets_at":' + $now.AddDays(3).ToUnixTimeSeconds() + '}}}'
$record | cargo run -q -- bridge        # prints: 5h 23.5% · 7d 41.2%
cargo run -q                            # the window shows these numbers
```

On Linux and macOS (not tried) the same with `export` and `printf '%s' '...' | cargo run -q -- bridge`; the future times come from `date -d '+3 hours' +%s` (Linux) or `date -v+3H +%s` (macOS). Feed several records with rising percentages a few minutes apart to see the usage rate, the forecast and the chart.

**Try the setup commands safely:** `cargo run -q -- setup-bridge --yes` with `CLAUDE_CONFIG_DIR` set writes into the folder you named, makes a backup, and `remove-bridge --yes` puts it back. Never try them without `CLAUDE_CONFIG_DIR` unless you mean your real settings.

**Measure:** `scripts/measure-bridge.ps1`, `measure-startup.ps1` and `measure-idle.ps1` (see [measurements.md](measurements.md)) use temporary folders as well; they measure the release build (`cargo build --release`) and, if there is none, the debug build with a warning.

## 6. Checks and handing a change in

Before every push:

```
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

(or `ctest --test-dir build -C Release` after a CMake configure). Fix the cause of a failure; do not weaken a check or an allowed lint to make it pass.

- **Commit message:** `<type>: <what>` in English, with the footer `Refs #<issue>`; types as in Conventional Commits (`feat`, `fix`, `docs`, `test`, `refactor`, `build`, `chore`).
- **Pull request:** use the template (what and why, the requirements, the evidence as real commands with real output, a self-review from the reviewer's point of view, an independent review, follow-ups). Write `Refs #<issue>`, not `Closes`. State what you did **not** try.
- **Review:** someone who did not write the change reads the diff against the requirement. Findings are fixed, not only noted.
- **Merge:** into `develop` with a merge commit, after the required CI check is green; branches are kept. Changes to `main`, tags and releases are the owner's decision.
- **After the merge:** a closing note on the issue with the pull request, the requirements' state and the effort.

If you used an AI tool anywhere, say so in the pull request (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## 7. Rules of the repository

- **Everything pushed is public**, from the first character. Never write names of people or computers, internal addresses, paths that contain a user name, credentials or their places. Test data uses neutral paths such as `/opt/tools/...` and `C:/Tools/...`.
- **English** for code, comments, commits, issues, pull requests and documents.
- **Licences.** The project is Apache-2.0. Before a dependency is added, look at its licence and at what it pulls in; the inventory is issue #58, the notices file for the release is #64.
- **No network.** The program makes no connection of its own. A change that adds one is a decision of the owner.
- **Requirements** are the owner's. A proposal is an issue or a `draft` requirement; an approved requirement is changed only with the owner's decision.
- **Documents move with the code:** the guide, the technical documentation, the changelog and the traceability are part of the change.

## 8. Debugging

- **Read the log first.** `log.txt` in the data folder (section "Where your data is" of the user guide) has one line per event of the bridge and the window, at level info and above, without the content of what Claude Code sent.
- **The window does not appear.** Run it from a terminal with `cargo run` and read what it prints; check `log.txt`; a second window shows only *usage-cockpit is already running.* (a first one runs: its lock is `cockpit.lock` in the data folder); on Linux check that the X11 or Wayland libraries are installed.
- **The bridge prints nothing or the wrong text.** Run it by hand as in section 5 and read what it prints; remember that it must exit with 0 and answer even for broken input (`echo not json | cargo run -q -- bridge` prints `usage-cockpit: no data` and writes `last_error.json`). A *kept* status line runs through `shell.rs`: its output replaces the bridge's own text, and a failure or a timeout of one second brings the own text back.
- **A value is wrong.** Find the pure function in `cockpit-core` and write the failing test with the numbers you see; the functions take the time as a parameter, so you can reproduce any moment.
- **The start entry on Windows.** The value `UsageCockpit` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` and, for "switched off in the Windows list of startup apps", the value of the same name under `...\Explorer\StartupApproved\Run`; look at both with `Get-ItemProperty`. With `USAGE_COCKPIT_HOME` set the name is `UsageCockpitTestHome`.
- **Locks and temporary files.** `history.lock`, `log.lock` and `cockpit.lock` are empty files that are locked while in use; a leftover is harmless. `*.tmp` files are leftovers of an interrupted atomic write and are removed after a minute.
- **Looking at the window for real.** On Windows, a script can start the program with temporary folders, find its window and take a picture of it; the pictures show the desktop, so do not put them in a pull request.

## 9. What is unfinished or untried

- **Not tried:** Linux and macOS (build, window, setup, autostart, uninstall: issues #129, #134), a Windows without development tools (#124), the light theme, display scaling other than 100 %, several monitors.
- **Planned scripts and builds:** the setup scripts for Linux and macOS (#152, #153), the CMake builds for them (#155, #156, #157).
- **Open decisions of the owner:** the setting *Start view* is overwritten by the view that was shown last (#147), signing (#6), the release (#71).
- **CI:** the only workflow checks that required files exist and that requirement ids are unique; building and testing on the four targets (#23) and the release workflow (#57) need the owner's go-ahead because they change workflow files.
- **Documents still to do before a release:** the licence inventory (#58), the legal files (#64).
