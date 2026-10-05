# 0002 Technology: Rust with egui/eframe

- Date: 2026-10-05
- Status: accepted (confirmed by the owner on 2026-10-05)

## Context

The cockpit must be one executable per platform without an installer or a separately installed runtime (REQ-101), run on Windows x64, macOS arm64, Linux x64 and Linux arm64 (REQ-102), show a small always-on-top window with custom graphics (REQ-018, REQ-029, REQ-030), stay light (REQ-105, REQ-116) and use only Apache-2.0-compatible components (REQ-107). The bridge must start and finish within 100 ms (REQ-109). Most of the code will be written by an AI coding agent, so a mainstream language with a strong compiler, a standard test runner and good documentation is an advantage.

## Candidates

| Criterion | Rust + egui/eframe | Go + Fyne | C# .NET Native AOT + Avalonia | Tauri (Rust + system web view) | C++ + Dear ImGui + GLFW |
|---|---|---|---|---|---|
| Single executable, no runtime (REQ-101) | yes, statically linked Rust; OS graphics libraries only | yes, but needs cgo | yes with Native AOT | **no on Linux**: requires the WebKitGTK system library at run time | yes |
| Four targets on GitHub-hosted runners (REQ-102) | yes, native build per runner | yes, but C toolchain per target (cgo) | yes, native build per runner (no cross-OS compile) | yes | yes |
| Always-on-top, custom drawing (REQ-018, REQ-030) | yes (`with_always_on_top`, immediate-mode painter, `egui_plot`) | yes | yes | yes (HTML/CSS) | yes |
| Licences (REQ-107) | MIT OR Apache-2.0 | BSD-3-Clause | MIT | MIT OR Apache-2.0 | MIT (Dear ImGui), zlib (GLFW) |
| Idle resources (REQ-105) | low; repaints only on change or timer | low to medium | medium (runtime in binary, larger files) | medium (web view process) | low |
| Bridge start time (REQ-109) | milliseconds | milliseconds | tens of milliseconds | n/a (separate binary needed) | milliseconds |
| Testability, agent-friendliness | `cargo test`, `clippy`, `rustfmt` built in; strict compiler catches errors early | good | good | split Rust/web stack | manual build system, no standard test runner, memory safety by discipline |

## Decision

**Rust with egui/eframe**, one executable with two modes:

- `cockpit` starts the window;
- `cockpit bridge` is the status line command: reads one record from standard input, stores it, prints a status line text and exits.

One file therefore serves as both bridge and cockpit, which keeps delivery to a single executable per platform.

Builds run natively on GitHub-hosted runners, no cross-compilation:

| Target | Runner |
|---|---|
| Windows x64 | `windows-latest` |
| macOS arm64 | `macos-latest` (Apple Silicon) |
| Linux x64 | `ubuntu-24.04`, build inside a `debian:bookworm` container |
| Linux arm64 (Raspberry Pi, 64-bit OS) | `ubuntu-24.04-arm`, build inside a `debian:bookworm` container |

Building the Linux executables inside a Debian 12 (bookworm) container fixes the required C library at glibc 2.36, the version of current 64-bit Raspberry Pi OS; the executables then also run on newer distributions. The Ubuntu 22.04 runner images are not used because GitHub retires them (deprecation from 2026-09-17, unsupported from 2027-04-17).

## Consequences

- The Linux executable uses the system's windowing and OpenGL libraries, which every desktop installation provides; this is not a separately installed runtime in the sense of REQ-101. The bridge mode must not need them; this is checked in the first prototype on a system without a display.
- egui redraws only on input or on request, so idle CPU stays low if the cockpit requests repaints at a modest interval (for example once per second for the countdown).
- **Always-on-top and window position on Linux:** under Wayland a client can neither keep itself on top nor set or restore its own position; current Raspberry Pi OS uses Wayland by default. On Linux the cockpit therefore starts through X11 (natively or via XWayland) where available, and documents the remaining limitation. Whether XWayland honours always-on-top on Raspberry Pi OS is verified on real hardware by the owner. Windows and macOS support both directly.
- **Windows console output:** the executable is built as a GUI application so that no console window opens on double-click. Bridge mode writes to the standard output that Claude Code passes in as a pipe, which works for GUI applications; for `--version` typed in a terminal the program attaches to the parent console.
- **macOS:** unsigned, non-notarised downloads are blocked on current macOS until the user allows them once in System Settings → Privacy & Security; the user documentation describes this. Windows shows a SmartScreen warning. Signing is a separate owner decision (#6).
- **Scope:** Linux arm64 means 64-bit Raspberry Pi OS (or another 64-bit distribution); 32-bit Raspberry Pi OS is not a target.
- **Licences:** dependencies are pinned in `Cargo.lock`; a licence inventory is generated in CI (REQ-107). It includes the fonts bundled by egui, which use font licences (SIL Open Font License 1.1, Ubuntu Font Licence) that permit redistribution; the licence check is configured to allow them explicitly.

## Minimal always-on-top proof

The first implementation issue builds a window of 320 × 120 logical pixels with `eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_always_on_top().with_inner_size([320.0, 120.0]), .. }` that draws one bar and a text, and runs it on the CI runners of all four targets (build) and on the owner's machines (visual check).

## Sources

- egui/eframe licence and features: <https://docs.rs/eframe/latest/eframe/>, `ViewportBuilder::with_always_on_top`: <https://docs.rs/egui/latest/egui/viewport/struct.ViewportBuilder.html>
- Tauri Linux run-time dependency on WebKitGTK: <https://v2.tauri.app/start/prerequisites/>
- Fyne needs cgo and a C compiler per target: <https://docs.fyne.io/started/cross-compiling/>
- .NET Native AOT has no cross-OS compilation: <https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/cross-compile>
- GitHub-hosted arm64 runners for public repositories (generally available, labels `ubuntu-24.04-arm`, `ubuntu-22.04-arm`, `windows-11-arm`): <https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/>
- Retirement of the Ubuntu 22.04 runner images: <https://github.com/actions/runner-images/issues/14254>
