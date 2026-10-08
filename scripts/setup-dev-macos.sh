#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
#
# Checks, and on request installs, the tools needed to develop usage-cockpit on a Mac with Apple
# Silicon.
#
#   sh scripts/setup-dev-macos.sh --check    # report only, installs nothing
#   sh scripts/setup-dev-macos.sh            # report, ask, install what is missing, build once
#
# What is needed:
#   - the Xcode Command Line Tools (compiler, linker and Git), installed with `xcode-select
#     --install`: macOS opens a window where you agree; the script waits until it is done.
#   - Rust: rustup and the toolchain pinned in rust-toolchain.toml, with rustfmt and clippy.
#   - CMake and Ninja (for the CMake builds), installed with Homebrew.
#
# Homebrew itself is never installed by this script: if it is missing and CMake or Ninja are
# needed, the script first installs everything else it can, prints the official command and
# then ends with exit code 3 (with --check: exit code 1 and the command). No step needs
# administrator rights (the Command Line Tools ask you in their own window).
#
# It runs with sh (and with zsh, which is switched to the sh word splitting below).
#
# It installs only what is missing and can be run again. It contains no secrets. The official
# rustup installer adds ~/.cargo/bin to the PATH in your shell profile; nothing else outside the
# places of the tools is changed. Options and exit codes: see --help.

set -u
# zsh does not split unquoted variables into words unless told to; this script relies on it
if [ -n "${ZSH_VERSION:-}" ]; then
    emulate sh
fi

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
# shellcheck source=lib/setup-dev-common.sh
. "$SCRIPT_DIR/lib/setup-dev-common.sh"

# Where Homebrew lives (Apple Silicon, Intel); the PATH of a non-login shell often lacks it.
BREW_DIRS=${COCKPIT_SETUP_BREW_DIRS-/opt/homebrew/bin /usr/local/bin}
# How long and how often to look whether the Command Line Tools are installed (seconds).
CLT_WAIT=${COCKPIT_SETUP_WAIT_SECONDS:-900}
CLT_POLL=${COCKPIT_SETUP_POLL_SECONDS:-5}
# a value that is not a whole number would make the wait loop run forever
case "$CLT_WAIT" in '' | *[!0-9]*) CLT_WAIT=900 ;; esac
case "$CLT_POLL" in '' | *[!0-9]*) CLT_POLL=5 ;; esac

BREW_INSTALL_COMMAND='/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'

# The first folder that has `brew` wins (on Apple Silicon /opt/homebrew, before an old Intel
# Homebrew in /usr/local).
use_brew_bin() {
    for brew_dir in $BREW_DIRS; do
        if [ -x "$brew_dir/brew" ]; then
            PATH="$brew_dir:$PATH"
            export PATH
            return 0
        fi
    done
}

# The Command Line Tools (or a full Xcode) are selected and the folder exists.
clt_present() {
    clt_path=$(xcode-select -p 2>/dev/null) || return 1
    [ -d "$clt_path" ]
}

check_tools() {
    channel=$(toolchain_channel "$REPO_ROOT/rust-toolchain.toml")
    use_cargo_bin
    use_brew_bin
    if clt_present; then
        state clt 'Xcode Command Line Tools' 1 "$clt_path"
        state_program git git Git 'comes with the Command Line Tools'
        state_program cc cc 'C compiler (cc)' 'comes with the Command Line Tools'
    else
        state clt 'Xcode Command Line Tools' 0 'install with xcode-select --install'
        # /usr/bin/git and /usr/bin/cc are stubs that would open the installation window if they
        # were run: they are not run until the tools are there
        state git Git 0 'comes with the Command Line Tools'
        state cc 'C compiler (cc)' 0 'comes with the Command Line Tools'
    fi
    state_program rustup rustup rustup 'install with the official installer (curl | sh), without a default toolchain'
    state_toolchain "$channel"
    state_program cmake cmake CMake 'brew install cmake'
    state_program ninja ninja Ninja 'brew install ninja'
    if is_missing rustup; then
        state_program curl curl 'curl (to install rustup)' 'comes with macOS'
    fi
}

brew_packages() {
    brew_list=""
    is_missing cmake && brew_list="$brew_list cmake"
    is_missing ninja && brew_list="$brew_list ninja"
    printf '%s' "$brew_list"
}

# Waits until the Command Line Tools are installed (the installation window is the person's).
wait_for_clt() {
    echo 'A window of macOS asks you to install the Command Line Tools: agree there. Waiting ...'
    waited=0
    while ! clt_present; do
        if [ "$waited" -ge "$CLT_WAIT" ]; then
            echo "The Command Line Tools are still not installed after $CLT_WAIT seconds; run the script again when the installation is done." >&2
            return 1
        fi
        sleep "$CLT_POLL"
        # every round counts at least one second, so the loop ends even for a poll time of 0
        if [ "$CLT_POLL" -ge 1 ]; then waited=$((waited + CLT_POLL)); else waited=$((waited + 1)); fi
    done
    echo 'The Command Line Tools are installed.'
}

main() {
    parse_args "$@"
    if [ "$(uname -s)" != Darwin ]; then
        echo "This script is for macOS; this system is $(uname -s). See docs/development-guide.md for the other systems." >&2
        exit 3
    fi
    if [ "$(uname -m)" != arm64 ]; then
        if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
            echo 'Note: this terminal runs under Rosetta (x86_64) on an Apple Silicon Mac; rustup would install the Intel toolchain. Open a terminal that does not run under Rosetta.'
        else
            echo "Note: the supported Mac is the one with Apple Silicon (arm64); this one reports $(uname -m)."
        fi
    fi
    check_tools
    print_states
    if [ "$MISSING_COUNT" = 0 ]; then
        echo 'Everything is there.'
    else
        echo
        echo "$MISSING_COUNT missing."
        brew_needed=$(brew_packages)
        no_brew=0
        if [ -n "$brew_needed" ] && ! have brew; then
            no_brew=1
            echo
            echo 'Homebrew is needed for:'"$brew_needed"' and is not installed. This script does not install it. Install it with the official command, then run this script again:'
            echo "  $BREW_INSTALL_COMMAND"
        fi
        if [ "$CHECK_ONLY" = 1 ]; then
            echo
            echo 'Nothing was installed (--check). Run without --check to install.'
            exit 1
        fi
        installable=0
        is_missing clt && installable=1
        is_missing rustup && installable=1
        is_missing toolchain && installable=1
        [ -n "$brew_needed" ] && [ "$no_brew" = 0 ] && installable=1
        if [ "$installable" = 0 ]; then
            echo
            echo 'There is nothing in the list that this script can install here; install Homebrew as described above and run it again.'
            exit 3
        fi
        echo
        echo 'This would be installed:'
        is_missing clt && echo '  - the Xcode Command Line Tools (xcode-select --install; macOS asks you in its own window)'
        is_missing rustup && echo '  - rustup (official installer, without a default toolchain)'
        { is_missing rustup || is_missing toolchain; } && echo "  - the Rust toolchain ${channel:-stable} with rustfmt and clippy"
        [ -n "$brew_needed" ] && [ "$no_brew" = 0 ] && echo "  - with Homebrew:$brew_needed"
        confirm 'Install the missing tools?' || { echo 'Nothing was installed.'; exit 1; }
        if is_missing clt; then
            # it fails when the tools are already installed or no installer can be started (an SSH
            # session without a screen): look again before waiting
            xcode-select --install || true
            clt_present || wait_for_clt || exit 2
        fi
        if is_missing rustup; then
            install_rustup || exit 2
        fi
        if is_missing rustup || is_missing toolchain; then
            install_toolchain "$channel" || exit 2
        fi
        if [ -n "$brew_needed" ] && [ "$no_brew" = 0 ]; then
            echo "brew install$brew_needed"
            # shellcheck disable=SC2086 # the names are a fixed list without glob characters
            brew install $brew_needed || exit 2
        fi
        echo
        echo 'Done. Open a new terminal so that the PATH of the new tools is in effect.'
        if [ "$no_brew" = 1 ]; then
            echo 'CMake and Ninja are still missing (Homebrew is not installed).'
            exit 3
        fi
    fi
    if [ "$SKIP_BUILD" = 0 ] && [ "$CHECK_ONLY" = 0 ]; then
        build_once || { echo 'cargo build failed.' >&2; exit 2; }
    fi
    print_commands
    exit 0
}

main "$@"
