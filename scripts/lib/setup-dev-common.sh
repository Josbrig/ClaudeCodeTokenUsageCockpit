# SPDX-License-Identifier: Apache-2.0
#
# Helpers of setup-dev-linux.sh and setup-dev-macos.sh. This file is read with `.` by those
# scripts (it is not run on its own) and by the tests. POSIX sh only: no bash features.
#
# What the scripts that use it promise:
#   --check       report only: install nothing, exit 1 if something is missing
#   --yes         do not ask before installing
#   --skip-build  do not run `cargo build` at the end
# Exit codes: 0 everything is there or was installed, 1 something is missing (--check) or the
# question was declined or could not be asked, 2 a step failed, 3 this system or package manager
# is not supported by the script (it says what to install by hand).

CHECK_ONLY=0
ASSUME_YES=0
SKIP_BUILD=0

# One row per tool: "ok      name   detail" or "MISSING name   hint"; the ids of the missing ones.
STATE_ROWS=""
MISSING_IDS=""
MISSING_COUNT=0

usage() {
    cat <<'EOF'
Usage: setup-dev-SYSTEM.sh [--check] [--yes] [--skip-build]

  --check       report what is there and what is missing; install nothing
  --yes         install what is missing without asking
  --skip-build  do not run `cargo build` at the end
  -h, --help    this text

Exit codes: 0 all there or installed, 1 something is missing / not agreed, 2 a step failed,
3 system or package manager not supported (the script says what to install by hand).
EOF
}

parse_args() {
    for setup_arg in "$@"; do
        case "$setup_arg" in
            --check) CHECK_ONLY=1 ;;
            --yes | -y) ASSUME_YES=1 ;;
            --skip-build) SKIP_BUILD=1 ;;
            -h | --help) usage; exit 0 ;;
            *) echo "unknown option: $setup_arg" >&2; usage >&2; exit 2 ;;
        esac
    done
}

# The channel named in rust-toolchain.toml (for example 1.99.0); nothing if there is none.
toolchain_channel() {
    [ -f "$1" ] || return 0
    sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -n 1
}

# The first non-empty line of what a command prints, trimmed.
version_line() {
    "$@" 2>&1 | sed -n '/[^[:space:]]/{s/^[[:space:]]*//;s/[[:space:]]*$//;p;q;}'
}

have() {
    command -v "$1" >/dev/null 2>&1
}

# The folder where rustup puts its programs (the PATH of this terminal may not have it yet).
cargo_bin() {
    printf '%s\n' "${CARGO_HOME:-$HOME/.cargo}/bin"
}

use_cargo_bin() {
    if [ -d "$(cargo_bin)" ]; then
        PATH="$(cargo_bin):$PATH"
        export PATH
    fi
}

# state ID NAME PRESENT(1/0) DETAIL-OR-HINT
state() {
    if [ "$3" = 1 ]; then
        state_mark='ok     '
    else
        state_mark='MISSING'
        MISSING_IDS="$MISSING_IDS $1"
        MISSING_COUNT=$((MISSING_COUNT + 1))
    fi
    state_row=$(printf '%s  %-28s %s' "$state_mark" "$2" "$4")
    STATE_ROWS="${STATE_ROWS}${state_row}
"
}

# Does the program NAME exist? Fills one row (the version if --version works).
state_program() {
    if have "$2"; then
        state "$1" "$3" 1 "$(version_line "$2" --version)"
    else
        state "$1" "$3" 0 "$4"
    fi
}

is_missing() {
    case " $MISSING_IDS " in *" $1 "*) return 0 ;; *) return 1 ;; esac
}

# The Rust toolchain of rust-toolchain.toml with rustfmt and clippy.
state_toolchain() {
    toolchain_name="Rust toolchain ${1:-stable}"
    if ! have rustup; then
        state toolchain "$toolchain_name" 0 'install with rustup (with rustfmt and clippy)'
        return
    fi
    toolchain_list=$(rustup toolchain list 2>/dev/null || true)
    toolchain_found=$(printf '%s\n' "$toolchain_list" | sed -n "s/^\\(${1:-stable}-[^ ]*\\).*/\\1/p" | head -n 1)
    if [ -z "$toolchain_found" ]; then
        state toolchain "$toolchain_name" 0 'install with rustup (with rustfmt and clippy)'
        return
    fi
    toolchain_components=$(rustup component list --toolchain "${1:-stable}" --installed 2>/dev/null || true)
    case "$toolchain_components" in
        *rustfmt*) toolchain_fmt=1 ;;
        *) toolchain_fmt=0 ;;
    esac
    case "$toolchain_components" in
        *clippy*) toolchain_clippy=1 ;;
        *) toolchain_clippy=0 ;;
    esac
    if [ "$toolchain_fmt" = 1 ] && [ "$toolchain_clippy" = 1 ]; then
        state toolchain "$toolchain_name" 1 "$toolchain_found with rustfmt and clippy"
    else
        state toolchain "$toolchain_name" 0 'add rustfmt and clippy with rustup'
    fi
}

print_states() {
    printf 'Development tools for usage-cockpit\n'
    printf '%s' "$STATE_ROWS"
}

# confirm QUESTION: yes with --yes, else asks on the terminal; no question possible means no.
confirm() {
    [ "$ASSUME_YES" = 1 ] && return 0
    if [ ! -t 0 ]; then
        echo 'No question can be asked here; run the script in a terminal or add --yes.'
        return 1
    fi
    printf '%s [y/N] ' "$1"
    read -r confirm_answer || return 1
    case "$confirm_answer" in y | Y | yes | YES | Yes) return 0 ;; *) return 1 ;; esac
}

# rustup without a default toolchain; the pinned one is installed after.
install_rustup() {
    have curl || { echo 'curl is needed to install rustup.' >&2; return 1; }
    echo 'Installing rustup (https://rustup.rs) without a default toolchain ...'
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
}

install_toolchain() {
    install_channel=${1:-stable}
    use_cargo_bin
    have rustup || { echo 'rustup was not found after installing it.' >&2; return 1; }
    echo "rustup toolchain install $install_channel --profile minimal -c rustfmt -c clippy"
    rustup toolchain install "$install_channel" --profile minimal -c rustfmt -c clippy
}

# cargo build once in REPO_ROOT, to prove that the setup works.
build_once() {
    use_cargo_bin
    if ! have cargo; then
        echo
        echo 'cargo was not found on this PATH, so the build was skipped. Open a new terminal and run the script again to check it.'
        return 0
    fi
    echo
    echo 'Building once to prove that the setup works (cargo build) ...'
    (cd "$REPO_ROOT" && cargo build) || return 1
    echo 'The build works.'
}

print_commands() {
    echo
    echo 'Build and test:'
    echo '  cargo fmt --all --check'
    echo '  cargo clippy --all-targets -- -D warnings'
    echo '  cargo test --all'
}
