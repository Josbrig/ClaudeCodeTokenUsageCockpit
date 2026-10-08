# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-10-08 (pre-release)

First pre-release, Windows x64. The notes for the release, including what it cannot do, are in [docs/release-notes-0.1.0.md](docs/release-notes-0.1.0.md).

### Removed
- The switch *Start with Windows* (and its Linux and macOS forms) in the settings: a window started with the system has no live values to show. *Remove everything* still removes an entry made by an earlier version.

### Changed
- The Windows setup script also looks at the `PATH` that Windows has stored, so it finds a tool it installed earlier in the same terminal.
- The bridge can be set up from a folder whose path contains a space: on Windows with the short folder name or a quoted path, on Linux and macOS (not tried yet) quoted for `sh -c`.
- The Windows x64 executable no longer imports the Visual C++ runtime (the C runtime is linked in; not yet tried on a clean Windows); the bridge starts faster (median 66 ms instead of 101 ms on the development machine).

### Added
- `NOTICE`, `THIRD_PARTY_LICENSES.md` (made with `cargo about`), `deny.toml` (allowed licences, `cargo deny check licenses`) and `scripts/third-party-licenses.ps1` / `.sh`.
- `docs/current-state.md` (data sources, what works when, concept problems) and `docs/retrospective.md` (what went wrong in the project and what has to change).
- Screenshots of both views in the README and the user guide.
- When a window has no current percentage, it shows the tokens of the last 5 hours or 7 days from the transcript files.
- `scripts/tests/cmake-build.tests.ps1 -OtherTargets`: configures the CMake build for Linux x64, Linux arm64 and macOS arm64 from Windows and runs the check target for each (the code and test code of those targets pass clippy; nothing is built into a program, linked or run).
- `scripts/setup-dev-macos.sh`: checks and, on request, installs the tools needed to develop the program on a Mac with Apple Silicon (Command Line Tools, Rust, CMake and Ninja through Homebrew); Homebrew itself is never installed by it.
- `scripts/setup-dev-linux.sh`: checks and, on request, installs the tools needed to develop the program on Linux (Rust, C compiler, Git, CMake, Ninja and the run-time libraries of the window; apt-based systems), with tests that use stand-in programs and are cut off from the computer they run on.
- Linux and macOS: *Remove everything* waits for the closing window with `kill -0`, and cleans the one folder of data and configuration on macOS once (compiled and linted, not tried on those systems).
- Development guide: how to compile and lint the code for Linux and macOS from any computer; checked (compile and lint only) for Linux x64, Linux arm64 and macOS arm64.
- `docs/development-guide.md`: how to set up the tools, build with CMake, find one's way in the code, make a change step by step with two worked examples, try it without touching real data, hand it in and debug, for working without an AI assistant.
- `CMakeLists.txt`: a CMake front end for the Rust workspace (targets `usage-cockpit`, `check`, `dist`, tests with `ctest`); `dist` writes `usage-cockpit-<version>-<system>` and `SHA256SUMS`. Tried on Windows.
- `scripts/setup-dev-windows.ps1`: checks and, on request, installs the tools needed to develop the program on Windows (Rust, C++ Build Tools, Git, CMake, Ninja); first part of `docs/development-guide.md`.
- User guide reworked into a complete manual (quick start, installing the portable program, setup and removal, operation, reading every value and message, glossary, troubleshooting); tests check that every message and label of the window is explained.
- Technical documentation of the program as built (`docs/technical-documentation.md`).
- Detailed view: bar charts of the tokens per day and per model above the two tables, with a choice of input, output, cache write or cache read.
- *Remove everything* (button in the detailed view, `usage-cockpit uninstall [--yes] [--remove-data]`): undoes the bridge entry, a start entry made by an earlier version and, on request, the data and configuration folders, so that the portable program can be deleted without leftovers.
- Project brief, project description, research note on data sources and first draft of the requirements.
- Repository setup: issue forms, pull request template, CI check, Dependabot configuration, contribution and security guidelines.
