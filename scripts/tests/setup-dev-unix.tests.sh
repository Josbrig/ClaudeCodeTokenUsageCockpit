#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
#
# Tests of scripts/setup-dev-linux.sh (and, when it exists, scripts/setup-dev-macos.sh) with
# stand-in programs: nothing is installed, no network is used, the real home folder is not
# touched. Run with a POSIX shell, for example
#   sh scripts/tests/setup-dev-unix.tests.sh
# On Windows, Git for Windows bash can run it (`bash scripts/tests/setup-dev-unix.tests.sh`); the
# scripts are then run with `dash` if it is there, else with `sh`.

set -u
HERE=$(cd "$(dirname "$0")" && pwd)
SCRIPTS=$(dirname "$HERE")
REPO=$(dirname "$SCRIPTS")
if command -v dash >/dev/null 2>&1; then SHELL_UNDER_TEST=dash; else SHELL_UNDER_TEST=sh; fi
failed=0
total=0

pass() { total=$((total + 1)); echo "ok    $1"; }
fail() { total=$((total + 1)); failed=$((failed + 1)); echo "FAIL  $1"; [ -n "${2:-}" ] && echo "      $2"; }
# expect NAME ACTUAL EXPECTED
expect() { if [ "$2" = "$3" ]; then pass "$1"; else fail "$1" "expected: $3   actual: $2"; fi; }
# expect_contains NAME TEXT NEEDLE
expect_contains() {
    case "$2" in *"$3"*) pass "$1" ;; *) fail "$1" "missing: $3 in: $(printf '%s' "$2" | head -c 400)" ;; esac
}
expect_not_contains() {
    case "$2" in *"$3"*) fail "$1" "unexpected: $3" ;; *) pass "$1" ;; esac
}

TMP=$(mktemp -d 2>/dev/null || mktemp -d -t cockpit-setup)
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT INT TERM

CHANNEL=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$REPO/rust-toolchain.toml" | head -n 1)

# ---------------------------------------------------------------- a fake computer
# fake_world DIR: an empty bin folder, a home and a log file; prints nothing.
new_world() {
    WORLD="$TMP/world-$1"
    rm -rf "$WORLD"
    mkdir -p "$WORLD/bin" "$WORLD/home"
    : > "$WORLD/log"
    WORLD_BIN="$WORLD/bin"
}

# program NAME BODY: a stand-in program in the bin folder of the world.
program() {
    printf '#!/bin/sh\n%s\n' "$2" > "$WORLD_BIN/$1"
    chmod +x "$WORLD_BIN/$1"
}

# A Linux computer that has everything.
full_linux() {
    new_world "$1"
    program uname 'if [ "${1:-}" = -s ]; then echo Linux; else echo x86_64; fi'
    program cc 'echo "cc (fake) 12.0"'
    program pkg-config "case \"\$1\" in --exists) exit 0 ;; --modversion) echo 1.0.0 ;; esac"
    program git 'echo "git version 2.0.0"'
    program curl "if [ \"\${1:-}\" = --version ]; then echo 'curl 8.0.0'; else echo \"curl \$*\" >> '$WORLD/log'; fi"
    program cmake 'echo "cmake version 3.30.0"'
    program ninja 'echo 1.12.0'
    program apt-get "echo \"apt-get \$*\" >> '$WORLD/log'"
    program sudo "echo \"sudo \$*\" >> '$WORLD/log'"
    program cargo "echo \"cargo \$*\" >> '$WORLD/log'; echo cargo 1.0.0"
    program rustup "case \"\$1\" in --version) echo 'rustup 1.29.0';; toolchain) case \"\$2\" in list) echo '$CHANNEL-x86_64-unknown-linux-gnu (default)';; install) echo \"rustup \$*\" >> '$WORLD/log';; esac;; component) echo 'rustfmt-x86_64-unknown-linux-gnu'; echo 'clippy-x86_64-unknown-linux-gnu';; esac"
}

# run_script SCRIPT ARGS...: runs it in the world (stdin is not a terminal) and sets OUT and CODE.
run_script() {
    script=$1
    shift
    OUT=$(PATH="$WORLD_BIN:/usr/bin:/bin" HOME="$WORLD/home" CARGO_HOME="$WORLD/home/.cargo" \
        "$SHELL_UNDER_TEST" "$script" "$@" </dev/null 2>&1)
    CODE=$?
}

log() { cat "$WORLD/log"; }

# ---------------------------------------------------------------- the common helpers
helper() {
    # run a snippet with the helpers loaded
    "$SHELL_UNDER_TEST" -c ". '$SCRIPTS/lib/setup-dev-common.sh'; $1"
}

printf '%s\n' '[toolchain]' 'channel = "1.99.0"' 'profile = "minimal"' > "$TMP/rt.toml"
expect 'the channel is read from rust-toolchain.toml' "$(helper "toolchain_channel '$TMP/rt.toml'")" '1.99.0'
printf '%s\n' '[toolchain]' '# channel = "nightly"' > "$TMP/rt2.toml"
expect 'a commented channel is not read' "$(helper "toolchain_channel '$TMP/rt2.toml'")" ''
expect 'a missing file gives no channel' "$(helper "toolchain_channel '$TMP/none.toml'")" ''
expect 'the real rust-toolchain.toml names a channel' "$([ -n "$CHANNEL" ] && echo yes)" 'yes'
expect 'the first non-empty line is the version' "$(helper "version_line printf '\\n  git version 2.0  \\nx\\n'")" 'git version 2.0'
expect 'missing ids are collected' "$(helper "state a A 1 x; state b B 0 hint; state c C 0 hint; echo \"\$MISSING_COUNT\$MISSING_IDS\"")" '2 b c'
expect 'is_missing finds only whole ids' "$(helper "state ab AB 0 h; is_missing a && echo yes || echo no")" 'no'
expect 'a missing tool shows its hint' "$(helper "state b B 0 'install b'; print_states" | tail -n 1 | sed 's/  */ /g')" 'MISSING B install b'

# ---------------------------------------------------------------- Linux
LINUX="$SCRIPTS/setup-dev-linux.sh"

full_linux all
run_script "$LINUX" --check --skip-build
expect 'linux: everything there: exit code 0' "$CODE" 0
expect_contains 'linux: everything there: says so' "$OUT" 'Everything is there.'
expect_contains 'linux: the toolchain row names the channel' "$OUT" "$CHANNEL-x86_64-unknown-linux-gnu with rustfmt and clippy"
expect 'linux: check installs nothing' "$(log)" ''

full_linux ninja
rm "$WORLD_BIN/ninja"
run_script "$LINUX" --check --skip-build
expect 'linux: check with a missing tool: exit code 1' "$CODE" 1
expect_contains 'linux: the missing tool is listed with the apt package' "$OUT" 'MISSING  Ninja'
expect_contains 'linux: the package name is shown' "$OUT" 'apt: ninja-build'
expect 'linux: check with a missing tool installs nothing' "$(log)" ''

run_script "$LINUX" --skip-build
expect 'linux: no terminal and no --yes: exit code 1' "$CODE" 1
expect_contains 'linux: no terminal: says that no question can be asked' "$OUT" 'No question can be asked here'
expect 'linux: not agreed installs nothing' "$(log)" ''

run_script "$LINUX" --yes --skip-build
expect 'linux: --yes installs the missing package: exit code 0' "$CODE" 0
expect_contains 'linux: the step is announced before it runs' "$OUT" "this step runs 'sudo apt-get install'"
expect_contains 'linux: the packages are named in the plan' "$OUT" 'system packages with apt (sudo): ninja-build'
LOGTEXT=$(log)
expect_contains 'linux: sudo is used for apt only' "$LOGTEXT" 'sudo apt-get update'
expect_contains 'linux: only the missing package is installed' "$LOGTEXT" 'sudo apt-get install -y ninja-build'
expect_not_contains 'linux: nothing else is installed' "$LOGTEXT" 'libx11'
expect_not_contains 'linux: rustup is not touched when it is there' "$LOGTEXT" 'rustup toolchain install'

# a library that pkg-config does not find
full_linux lib
program pkg-config "case \"\$1\" in --exists) [ \"\$2\" = xcursor ] && exit 1; exit 0 ;; --modversion) echo 1.0.0 ;; esac"
run_script "$LINUX" --check --skip-build
expect_contains 'linux: a missing library is listed' "$OUT" 'MISSING  library xcursor'
expect_contains 'linux: with its apt package' "$OUT" 'apt: libxcursor-dev'
run_script "$LINUX" --yes --skip-build
expect_contains 'linux: the library package is installed' "$(log)" 'sudo apt-get install -y libxcursor-dev'

# rustup and the toolchain are missing: the official installer is used without a default toolchain
full_linux rustup
rm "$WORLD_BIN/rustup" "$WORLD_BIN/cargo"
program curl "if [ \"\${1:-}\" = --version ]; then echo 'curl 8.0.0'; exit 0; fi; echo \"curl \$*\" >> '$WORLD/log'; cat <<'INSTALLER'
echo \"installer args: \$*\" >> '$WORLD/log'
mkdir -p \"\$HOME/.cargo/bin\"
printf '#!/bin/sh\necho \"rustup \$*\" >> $WORLD/log\ncase \"\$1\" in --version) echo rustup 1.29.0;; toolchain) if [ \"\$2\" = list ]; then echo \"$CHANNEL-x86_64-unknown-linux-gnu (default)\"; fi;; component) echo rustfmt; echo clippy;; esac\n' > \"\$HOME/.cargo/bin/rustup\"
chmod +x \"\$HOME/.cargo/bin/rustup\"
INSTALLER"
run_script "$LINUX" --check --skip-build
expect_contains 'linux: missing rustup is listed' "$OUT" 'MISSING  rustup'
expect_contains 'linux: and the toolchain' "$OUT" "MISSING  Rust toolchain $CHANNEL"
run_script "$LINUX" --yes --skip-build
expect 'linux: rustup installation: exit code 0' "$CODE" 0
LOGTEXT=$(log)
expect_contains 'linux: the installer is called without a default toolchain' "$LOGTEXT" 'installer args: -y --default-toolchain none --profile minimal'
expect_contains 'linux: then only the pinned toolchain is installed' "$LOGTEXT" "rustup toolchain install $CHANNEL --profile minimal -c rustfmt -c clippy"
expect_not_contains 'linux: no system package is installed for that' "$LOGTEXT" 'apt-get install'

# the build at the end
full_linux build
run_script "$LINUX" --yes
expect 'linux: the build runs at the end: exit code 0' "$CODE" 0
expect_contains 'linux: cargo build was run' "$(log)" 'cargo build'
program cargo "echo \"cargo \$*\" >> '$WORLD/log'; exit 1"
run_script "$LINUX" --yes
expect 'linux: a failing build: exit code 2' "$CODE" 2
run_script "$LINUX" --yes --skip-build
expect 'linux: --skip-build skips it' "$CODE" 0

# another system, another package manager
full_linux darwin
program uname 'if [ "${1:-}" = -s ]; then echo Darwin; else echo arm64; fi'
run_script "$LINUX"
expect 'linux script on another system: exit code 3' "$CODE" 3
expect_contains 'linux script on another system: says so' "$OUT" 'This script is for Linux'

full_linux family
rm "$WORLD_BIN/apt-get" "$WORLD_BIN/ninja"
run_script "$LINUX" --check
expect 'other family: check: exit code 1' "$CODE" 1
run_script "$LINUX" --yes --skip-build
expect 'other family: only system packages missing: exit code 3' "$CODE" 3
expect_contains 'other family: the package names are printed' "$OUT" 'sudo dnf install'
expect_contains 'other family: for several families' "$OUT" 'sudo pacman -S'
expect 'other family: nothing was installed' "$(log)" ''

# options
run_script "$LINUX" --help
expect 'help: exit code 0' "$CODE" 0
expect_contains 'help: shows the usage' "$OUT" 'Usage:'
run_script "$LINUX" --frobnicate
expect 'an unknown option: exit code 2' "$CODE" 2

# ---------------------------------------------------------------- syntax with the strictest shell
for script in "$SCRIPTS/lib/setup-dev-common.sh" "$LINUX" "$SCRIPTS/setup-dev-macos.sh"; do
    [ -f "$script" ] || continue
    if "$SHELL_UNDER_TEST" -n "$script" 2>/dev/null; then pass "syntax: $(basename "$script") parses with $SHELL_UNDER_TEST"; else fail "syntax: $(basename "$script")"; fi
done

echo
if [ "$failed" -gt 0 ]; then echo "$failed of $total check(s) failed"; exit 1; fi
echo "all $total checks passed"
