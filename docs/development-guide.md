# Development guide

How to develop the program by hand, without an AI assistant: set up the tools, build, test and change the code, and hand a change in.

> **Status.** This guide is written in parts. This version has the part on **setting up the tools** (the Windows script exists; the Linux and macOS scripts are planned in #152 and #153). The rest (getting around, making a change step by step, trying a change, handing it in, debugging) is issue #158. Until then, read the [technical documentation](technical-documentation.md) for how the program works and [CONTRIBUTING.md](../CONTRIBUTING.md) for how work is organised here.

## 1. Setting up the tools

### 1.1 What is needed

| Tool | Why |
|---|---|
| **Rust**, the toolchain pinned in `rust-toolchain.toml`, with `rustfmt` and `clippy` | builds and checks the program (installed with `rustup`) |
| A **C++ toolchain** that the Rust linker uses | on Windows the Microsoft C++ Build Tools (MSVC) |
| **Git** | the repository; on Windows its `bash` is also used by some tests |
| **CMake** and **Ninja** | the CMake builds (#154 to #157); not needed for plain `cargo` |

### 1.2 Windows

The script `scripts/setup-dev-windows.ps1` looks at what is there, says what is missing, and installs only that, after asking:

```
powershell -File scripts\setup-dev-windows.ps1 -CheckOnly   # report only, installs nothing
powershell -File scripts\setup-dev-windows.ps1              # report, ask, install what is missing, build once
```

- `-CheckOnly` installs nothing and exits with 1 if something is missing. The exit code is also 1 when you decline the question or no question can be asked, 2 when an installation fails, and 0 when everything is there or was installed.
- Without it, the script lists what it would install and asks `Install the missing tools? [y/N]`. `-Yes` skips the question (for use in a script); in a place where no question can be asked (no terminal) and without `-Yes` it installs nothing.
- Programs are installed **for the current user** where the package allows it (`--scope user` is tried first). The **C++ Build Tools can only be installed for the whole computer**: a Windows prompt asks for administrator consent. rustup is installed without a default toolchain; the script then installs only the pinned one.
- It uses `winget` for the programs (Git: `Git.Git`; CMake: `Kitware.CMake`; Ninja: `Ninja-build.Ninja`; the C++ Build Tools: `Microsoft.VisualStudio.2022.BuildTools` with the workload *Desktop development with C++*; rustup: `Rustlang.Rustup`) and `rustup` for the toolchain. If `winget` is missing it says which program to install by hand.
- It can be run again at any time; what is present is not touched.
- At the end it runs `cargo build` once to prove that the setup works (`-SkipBuild` leaves that out) and prints the commands to build and test.
- After an installation, open a **new terminal** so that the new tools are on the `PATH`.

The pure parts of the script (reading the toolchain channel, the plan, the table, the `winget` arguments) and its flow (exit codes, consent, which tools would be installed, with the installers replaced by fakes) have a test script that installs nothing: `powershell -File scripts\tests\setup-dev-windows.tests.ps1`.

**By hand**, the same: install [rustup](https://rustup.rs) and run `rustup toolchain install <channel from rust-toolchain.toml> --profile minimal -c rustfmt -c clippy`; install the [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/) with the C++ workload; install Git, CMake and Ninja.

### 1.3 Check that it works

In the repository folder:

```
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

All three must pass before a change is handed in.
