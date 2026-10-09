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

### 1.0 Get the repository

```
git clone https://github.com/Josbrig/ClaudeCodeTokenUsageCockpit.git
cd ClaudeCodeTokenUsageCockpit
git switch develop
```

`develop` is the branch that work is based on and that pull requests go to (`main` holds releases). If you do not have the right to push to the repository, fork it on GitHub, push your branch to your fork and open the pull request from there.

### 1.1 What is needed

| Tool | Why |
|---|---|
| **Rust**, the toolchain pinned in `rust-toolchain.toml`, with `rustfmt` and `clippy` | builds and checks the program (installed with `rustup`) |
| A **C++ toolchain** that the Rust linker uses | on Windows the Microsoft C++ Build Tools (MSVC); on Linux `build-essential`; on macOS the Xcode Command Line Tools |
| **Git** | the repository; on Windows its `bash` is also used by some tests |
| **CMake** and **Ninja** | the CMake builds (section 2); not needed for plain `cargo` |
| On Linux: the run-time libraries of the window (X11, Wayland, xkbcommon, OpenGL/EGL) | the window loads them when it runs; nothing of them is needed to build it |

### 1.2 Windows

The script `scripts/setup-dev-windows.ps1` looks at what is there, says what is missing, and installs only that, after asking:

```
powershell -ExecutionPolicy Bypass -File scripts\setup-dev-windows.ps1 -CheckOnly   # report only, installs nothing
powershell -ExecutionPolicy Bypass -File scripts\setup-dev-windows.ps1              # report, ask, install what is missing, build once
```

`-ExecutionPolicy Bypass` is needed on a Windows whose policy refuses scripts that are not signed (the default of a client Windows); it applies to this one call only and changes no setting. If you downloaded the file as a zip, `Unblock-File scripts\*.ps1` does the same for good.

- `-CheckOnly` installs nothing and exits with 1 if something is missing. The exit code is also 1 when you decline the question or no question can be asked, 2 when an installation fails, and 0 when everything is there or was installed.
- Without it, the script lists what it would install and asks `Install the missing tools? [y/N]`. `-Yes` skips the question (for use in a script); in a place where no question can be asked (no terminal) and without `-Yes` it installs nothing.
- Programs are installed **for the current user** where the package allows it (`--scope user` is tried first). The **C++ Build Tools can only be installed for the whole computer**: a Windows prompt asks for administrator consent. rustup is installed without a default toolchain; the script then installs only the pinned one (this part was not tried: rustup was already installed on the machine where the script was tried).
- It uses `winget` for the programs (Git: `Git.Git`; CMake: `Kitware.CMake`; Ninja: `Ninja-build.Ninja`; the C++ Build Tools: `Microsoft.VisualStudio.2022.BuildTools` with the workload *Desktop development with C++*; rustup: `Rustlang.Rustup`) and `rustup` for the toolchain. If `winget` is missing it says which program to install by hand.
- It can be run again at any time; what is present is not touched.
- At the end it runs `cargo build` once to prove that the setup works (`-SkipBuild` leaves that out) and prints the commands to build and test.
- After an installation, open a **new terminal** so that the new tools are on the `PATH` of the programs you start. The script itself also looks at the `PATH` that Windows has stored, so running it again in the same terminal does not install a tool twice.

The pure parts of the script (reading the toolchain channel, the plan, the table, the `winget` arguments, the stored `PATH`) and its flow (exit codes, consent, which tools would be installed, with the installers replaced by fakes) have a test script that installs nothing: `powershell -ExecutionPolicy Bypass -File scripts\tests\setup-dev-windows.tests.ps1`. Tried for real: Ninja was installed by the script (for the user, exit code 0).

**By hand**, the same: install [rustup](https://rustup.rs) and run `rustup toolchain install <channel from rust-toolchain.toml> --profile minimal -c rustfmt -c clippy`; install the [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/) with the C++ workload; install Git, CMake and Ninja.

### 1.3 Linux

The script `scripts/setup-dev-linux.sh` (POSIX `sh`) does for Linux what the Windows script does: it looks at what is there, says what is missing, and installs only that, after asking:

```
sh scripts/setup-dev-linux.sh --check    # report only, installs nothing
sh scripts/setup-dev-linux.sh            # report, ask, install what is missing, build once
```

- **Options:** `--check` (install nothing; exit code 1 if something is missing), `--yes` or `-y` (do not ask), `--skip-build` (no `cargo build` at the end), `--help`. **Exit codes:** 0 everything is there or was installed, 1 something is missing (`--check`), the question was declined or could not be asked (no terminal and no `--yes`), 2 a step failed, 3 this is not Linux, or system packages are missing that the script cannot install here (another package manager, or no root rights and no `sudo`); it prints their names.
- **What it checks:** rustup and the pinned toolchain with `rustfmt` and `clippy`; a C compiler (`cc`, the linker driver), Git, CMake and Ninja; curl, but only when rustup has to be installed; and the **run-time libraries of the window** (`libxkbcommon`, `libxkbcommon-x11`, `libwayland-client`, `libX11`, `libXcursor`, `libXi`, `libXrandr`, `libxcb`, `libGL`, `libEGL`), looked up with `ldconfig -p`. A desktop has them; a minimal server image does not. **Building needs no development packages**: the window libraries are loaded when the program runs (`dlopen`), so the build needs only the compiler. (This was checked in the dependency tree: no crate links a system library at build time.)
- **What it installs:** on Debian, Ubuntu and Raspberry Pi OS (`apt-get` exists, and you are root or have `sudo`) the missing system packages with `sudo apt-get install -y ...`; **sudo is used for that step only**, and the plan says so before it asks. rustup comes from the official installer, downloaded to a file first (a failed download is an error), run **without a default toolchain**; then only the pinned toolchain is installed. That needs no root rights, but the rustup installer adds `~/.cargo/bin` to the `PATH` in your shell profile. On other families (Fedora, Arch, openSUSE) and without root rights or `sudo`, it installs only rustup and the toolchain and prints the package names (`dnf`, `pacman`, `zypper`, or `apt-get install ...` to run as administrator), also with `--check`.
- It can be run again at any time; what is present is not touched. After an installation open a new terminal (the script itself looks in `~/.cargo/bin` already).
- **Tests:** `sh scripts/tests/setup-dev-unix.tests.sh` runs the scripts with stand-in programs in a fake home: nothing is installed, no network is used, and the scripts see only a `PATH` with the stand-ins and a few wrapped basic tools, so a real `ninja`, `apt-get` or `sudo` on the computer that runs the tests cannot change a result. It covers the helpers (reading the channel, the version line, the list of missing tools), the Linux script (everything there, a missing tool, no question possible, `--yes`, root and no root, no `sudo`, a missing library, no `ldconfig`, a missing rustup with the installer called without a default toolchain, a failed download, a toolchain without clippy, the build and a failing build, another system, another package manager, arm64, a start from another folder, the options) and the syntax with `dash`. It ran on Windows with Git for Windows bash (69 checks).
- **Not tried on a real Linux:** the real `apt-get`, `sudo`, the rustup installer, and `ldconfig -p` on a real distribution (the package names are those of Debian and Ubuntu; check them if a library is still reported missing after the installation). `shellcheck` was not available; the files carry its directives for the two places where it would complain. The [HUMAN] issue for Linux asks for a real run.

### 1.3a macOS

The script `scripts/setup-dev-macos.sh` (POSIX `sh`, same options and exit codes as the Linux script, see section 1.3) does the same for a Mac with Apple Silicon:

```
sh scripts/setup-dev-macos.sh --check    # report only, installs nothing
sh scripts/setup-dev-macos.sh            # report, ask, install what is missing, build once
```

- **What it checks:** the Xcode Command Line Tools (`xcode-select -p`, and the folder it names must exist; they bring the compiler, the linker and Git), rustup and the pinned toolchain with `rustfmt` and `clippy`, CMake and Ninja; curl only when rustup has to be installed. While the Command Line Tools are missing, Git and the compiler are **not run** (their stubs in `/usr/bin` would open the installation window).
- **What it installs:** the Command Line Tools with `xcode-select --install` (macOS opens a window where you agree; the script waits up to 15 minutes, then says to run it again; if the installation cannot be started at all, for example in a session without a screen, it stops at once); rustup from the official installer without a default toolchain, then only the pinned toolchain; CMake and Ninja with `brew install` (only the missing ones). No step needs administrator rights.
- **Homebrew is never installed by the script.** If it is missing and CMake or Ninja are needed, the script installs what it can (the Command Line Tools, rustup, the toolchain), prints the official Homebrew command and ends with exit code 3 (with `--check`: exit code 1 and the command). Install Homebrew, then run the script again.
- On an Intel Mac it works but says that Apple Silicon is the supported target; in a terminal that runs under Rosetta on an Apple Silicon Mac it says so (rustup would install the Intel toolchain there). The script also runs under `zsh`.
- Homebrew is looked for in `/opt/homebrew/bin` first, then `/usr/local/bin`.
- **Tests:** the same test script as for Linux (`sh scripts/tests/setup-dev-unix.tests.sh`, 120 checks in all) covers the macOS script with stand-in programs: everything there, a missing tool (`--check`, no question possible, `--yes`), no Homebrew, missing Command Line Tools (not run, then installed and waited for, the time limit, tools that appear only after a few looks, an installation that cannot start, a selected folder that is gone), no Homebrew after the tools were installed, both packages at once, the first Homebrew folder wins, numbers that are not numbers, a missing rustup, the build, another system, an Intel Mac, Rosetta, help. They ran on Windows with Git for Windows bash.
- **Not tried on a real Mac:** `xcode-select --install`, Homebrew, the rustup installer. The [HUMAN] issue for macOS asks for a real run.

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
| `check` | `cargo fmt --all --check`, then `cargo clippy --all-targets -D warnings`, for the workspace and for `tools/usage-probe` |
| `ctest` (or the target `test`) | six tests: `cargo-fmt`, `cargo-clippy`, `cargo-test` (`cargo test --all`) and the same three for `tools/usage-probe` (`probe-cargo-fmt`, `probe-cargo-clippy`, `probe-cargo-test`) |
| `usage-probe` (also part of the default) | `cargo build --release --locked` of the test program `tools/usage-probe`, into `build/cargo-target-probe` (a cargo project of its own; it is not part of `dist`) |
| `dist` | builds, copies the program to `build/dist/usage-cockpit-<version>-<system>[.exe]` and records its SHA-256 in `build/dist/SHA256SUMS` (lines of other systems built into the same folder are kept) |

- **Version and system name.** The version is the one of `Cargo.toml` (`[workspace.package]`). The system name follows from the Rust target: `windows-x64` (`x86_64-pc-windows-msvc`), `linux-x64` (`x86_64-unknown-linux-gnu`), `linux-arm64` (`aarch64-unknown-linux-gnu`), `macos-arm64` (`aarch64-apple-darwin`). Another target is refused with a message.
- **Options.** `-DCOCKPIT_TARGET=<triple>` builds for another target than the host of `rustc`; `-DCOCKPIT_CARGO=<path>` uses a particular `cargo`. Without the options the `cargo` of the cargo bin folder of rustup is used (its `cargo` honours the toolchain of `rust-toolchain.toml`; the commands are run in the repository folder), else the one on the `PATH`. Configuring runs `rustc -vV`, which can make rustup download the pinned toolchain the first time.
- **A target that is not the host.** `-DCOCKPIT_TARGET=<triple>` needs that target installed (`rustup target add <triple>`) and a linker for it. The tests of such a target cannot run on this computer: `cargo-test` then only compiles them (`--no-run`) and says so when configuring.
- **Where things go.** Everything cargo builds goes to `build/cargo-target`, so a CMake build never mixes with the `target/` folder of plain `cargo` runs. `build/` is ignored by git.
- **Not inside the source tree.** CMake refuses a build folder that is the source folder (CMake has written its `CMakeCache.txt` and `CMakeFiles` there by then: delete them).
- **Generators.** Any generator is meant to work. Tried on Windows: Visual Studio 17 2022 (with `--config Release`) and Ninja (`cmake -S . -B build -G Ninja`, then `cmake --build build --target dist`, which worked with the same result). NMake and Make were not tried.
- **One script that asks.** `powershell -File scripts\build.ps1` makes a build folder (default `build`) and **asks** which build tool CMake shall use (Ninja, the Visual Studio that CMake finds, NMake or Make; only what is installed is offered; Ninja is the recommended one for working in VS Code) and what to build (everything, or a choice of: the program, the test program, the checks, the tests, the release file `dist`). It shows the plan and asks once more before it starts. With `-Yes` it asks nothing (parameters `-Generator`, `-Parts app,probe,check,test,dist`, `-BuildFolder`; `-PlanOnly` shows the plan only). An existing build folder that was made with another tool cannot be changed by CMake: the script names the folder and offers to delete it (only a folder inside the repository that has a `CMakeCache.txt` of this repository; `-ReplaceFolder` answers yes). Tests of the script: `powershell -File scripts\tests\build.tests.ps1` (the questions with injected answers, the plan, the folder checks; nothing is built). The script is for Windows; a twin for Linux and macOS is still to be written.
- **Windows test.** `scripts\tests\cmake-build.tests.ps1` configures into a temporary folder, builds `dist`, checks the file name, the checksum, that the program in `dist` starts, that a second run keeps one line per file and the lines of other systems, that an unsupported target and an in-source build are refused, and that `target\` was not touched (`-RunTests` also runs `ctest`). The first run builds the whole program and takes a few minutes.
- **Linux and macOS.** The same files are meant to work there (#155, #156, #157); a build there was **not tried** yet. From Windows, `scripts\tests\cmake-build.tests.ps1 -OtherTargets` configures for `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin` (needs `rustup target add` for them) and runs the `check` target for each: the system names `linux-x64`, `linux-arm64` and `macos-arm64` come out right, the configuration says that the tests of such a target are only compiled (not run), `cargo fmt --check` passes, and `cargo clippy --all-targets -D warnings` passes for that target, so its code and test code type-check without warnings. Nothing is linked or run, so no program for those systems is built this way.
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
cargo run -q                            # the window shows these numbers; close it to get the terminal back
```

The first `cargo run` builds the whole program and prints nothing for a few minutes (`-q` hides the progress; leave it out to see it). The program is built for the Windows GUI subsystem: if you start `target\debug\usage-cockpit.exe` yourself instead of through `cargo run`, PowerShell does not wait for it, `$LASTEXITCODE` stays empty and the output can come later; use `cargo run`, or `Start-Process -Wait -RedirectStandardInput ... -RedirectStandardOutput ...` as the scripts in `scripts/` do.

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

**Checking the other systems without having them.** Code that is only compiled on Linux or macOS (`#[cfg(unix)]`, `#[cfg(not(windows))]`: the quoting for `sh`, the shell for the kept status line, the start entry as a file) is not compiled by the three commands above on Windows. From any computer you can compile and lint it for the other targets:

```
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin
cargo clippy --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
cargo clippy --target aarch64-unknown-linux-gnu --all-targets -- -D warnings
cargo clippy --target aarch64-apple-darwin --all-targets -- -D warnings
```

This proves that the code **that is compiled for that system, including its tests,** compiles and passes the lints. At the moment all three targets compile the same non-Windows code (there is no code that is specific to Linux or to macOS yet), so a difference between Linux and macOS that is added later is only checked for the targets that select it. It proves nothing else: nothing is linked or run, so the behaviour on that system is not tried (the tests for it have to be run there). It was run on Windows for these three targets and passed without a message. Do it before you hand in a change that touches code behind a `cfg`.

**Handing in:**

- **Commit message:** `<type>: <what>` in English, with the footer `Refs #<issue>`; types as in Conventional Commits (`feat`, `fix`, `docs`, `test`, `refactor`, `build`, `chore`).
- **Pull request:** use the template (what and why, the requirements, the evidence as real commands with real output, a self-review from the reviewer's point of view, an independent review, follow-ups). Write `Refs #<issue>`, not `Closes`. State what you did **not** try.
- **Review:** someone who did not write the change reads the diff against the requirement. Findings are fixed, not only noted.
- **Merge:** into `develop` with a merge commit, after the required CI check is green; branches are kept. Note that this check only looks at the repository files (the required files exist, requirement ids are unique): it does **not** build or test the code yet (section 9), so the three local commands above are the only gate for the code. Changes to `main`, tags and releases are the owner's decision.
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

- **Read the log first.** `log.txt` in the data folder (section "Where your data is" of the user guide) has the events of the bridge and the window at level info and above (a start, a changed Claude Code version, a pruned history, every warning and error), without the content of what Claude Code sent. A normal successful bridge call writes nothing.
- **The window does not appear.** Run it from a terminal with `cargo run` and read what it prints; check `log.txt`; a second window shows only *usage-cockpit is already running.* (a first one runs: its lock is `cockpit.lock` in the data folder); on Linux check that the X11 or Wayland libraries are installed.
- **The bridge prints nothing or the wrong text.** Run it by hand as in section 5 and read what it prints; remember that it must exit with 0 and answer even for broken input (`echo not json | cargo run -q -- bridge` prints `usage-cockpit: no data` and writes `last_error.json`). A *kept* status line runs through `shell.rs`: its output replaces the bridge's own text, and a failure or a timeout of one second brings the own text back.
- **A value is wrong.** Find the pure function in `cockpit-core` and write the failing test with the numbers you see; the functions take the time as a parameter, so you can reproduce any moment.
- **The start entry on Windows.** The value `UsageCockpit` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` and, for "switched off in the Windows list of startup apps", the value of the same name under `...\Explorer\StartupApproved\Run`; look at both with `Get-ItemProperty`. With `USAGE_COCKPIT_HOME` set the name is `UsageCockpitTestHome`.
- **Locks and temporary files.** `history.lock`, `log.lock` and `cockpit.lock` are empty files that are locked while in use; a leftover is harmless. `*.tmp` files are leftovers of an interrupted atomic write and are removed after a minute.
- **Looking at the window for real.** On Windows, a script can start the program with temporary folders, find its window and take a picture of it; the pictures show the desktop, so do not put them in a pull request.

## 9. What is unfinished or untried

- **Not tried:** Linux and macOS (build, window, setup, autostart, uninstall: issues #129, #134), a Windows without development tools (#124), the light theme, display scaling other than 100 %, several monitors.
- **Planned builds:** the CMake builds for the other systems (#155, #156, #157).
- **Open decisions of the owner:** the setting *Start view* is overwritten by the view that was shown last (#147), signing (#6), the release (#71).
- **CI:** the only workflow checks that required files exist and that requirement ids are unique; building and testing on the four targets (#23) and the release workflow (#57) need the owner's go-ahead because they change workflow files.
- **Documents still to do before a release:** the licence inventory (#58), the legal files (#64).
