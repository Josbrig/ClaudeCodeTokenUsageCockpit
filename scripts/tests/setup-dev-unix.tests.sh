#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
#
# Tests of scripts/setup-dev-linux.sh (and, when it exists, scripts/setup-dev-macos.sh) with
# stand-in programs: nothing is installed, no network is used, the real home folder is not
# touched. Run with a POSIX shell, for example
#   sh scripts/tests/setup-dev-unix.tests.sh
# On Windows, Git for Windows bash can run it (`bash scripts/tests/setup-dev-unix.tests.sh`); the
# scripts are then run with `dash` if it is there, else with `sh`.
#
# The tests are cut off from the computer they run on: the scripts under test get a PATH that holds
# only the stand-in programs of the scenario and a few wrappers of basic tools (sed, cat, ...), so
# a real `ninja`, `apt-get`, `cargo` or `sudo` on this computer cannot change a result, and `id`
# is a stand-in too.

set -u
HERE=$(cd "$(dirname "$0")" && pwd)
SCRIPTS=$(dirname "$HERE")
REPO=$(dirname "$SCRIPTS")
if command -v dash >/dev/null 2>&1; then
    SHELL_PATH=$(command -v dash)
else
    SHELL_PATH=$(command -v sh)
fi
SHELL_NAME=$(basename "$SHELL_PATH")
failed=0
total=0

pass() { total=$((total + 1)); echo "ok    $1"; }
fail() { total=$((total + 1)); failed=$((failed + 1)); echo "FAIL  $1"; [ -n "${2:-}" ] && echo "      $2"; }
# expect NAME ACTUAL EXPECTED
expect() { if [ "$2" = "$3" ]; then pass "$1"; else fail "$1" "expected: $3   actual: $2"; fi; }
# expect_contains NAME TEXT NEEDLE
expect_contains() {
    case "$2" in *"$3"*) pass "$1" ;; *) fail "$1" "missing: $3 in: $(printf '%s' "$2" | head -c 500)" ;; esac
}
expect_not_contains() {
    case "$2" in *"$3"*) fail "$1" "unexpected: $3" ;; *) pass "$1" ;; esac
}

TMP=$(mktemp -d 2>/dev/null || mktemp -d -t cockpit-setup)
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM

CHANNEL=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$REPO/rust-toolchain.toml" | head -n 1)

# The real basic tools, found before the PATH is replaced; the world gets wrappers that call them.
BASIC_TOOLS="sed head cat dirname basename mkdir chmod rm mktemp tr"
REAL_DIR="$TMP/real"
mkdir -p "$REAL_DIR"
for tool in $BASIC_TOOLS; do
    real=$(command -v "$tool") || { echo "the test needs the tool $tool"; exit 2; }
    printf '#!/bin/sh\nexec "%s" "$@"\n' "$real" > "$REAL_DIR/$tool"
    chmod +x "$REAL_DIR/$tool"
done
printf '#!/bin/sh\nexec "%s" "$@"\n' "$SHELL_PATH" > "$REAL_DIR/sh"
chmod +x "$REAL_DIR/sh"

# ---------------------------------------------------------------- a fake computer
# new_world NAME: an empty bin folder (the scenario's programs), a home and a log file.
new_world() {
    WORLD="$TMP/world-$1"
    rm -rf "$WORLD"
    mkdir -p "$WORLD/bin" "$WORLD/home"
    : > "$WORLD/log"
    WORLD_BIN="$WORLD/bin"
    # a user who is not root, unless FAKE_UID says otherwise
    program id 'echo "${FAKE_UID:-1000}"'
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
    program git 'echo "git version 2.0.0"'
    program curl "if [ \"\${1:-}\" = --version ]; then echo 'curl 8.0.0'; else echo \"curl \$*\" >> '$WORLD/log'; fi"
    program cmake 'echo "cmake version 3.30.0"'
    program ninja 'echo 1.12.0'
    program ldconfig "cat '$WORLD/ldconfig.txt'"
    ldconfig_with libxkbcommon.so.0 libxkbcommon-x11.so.0 libwayland-client.so.0 libX11.so.6 libXcursor.so.1 libXi.so.6 libXrandr.so.2 libxcb.so.1 libGL.so.1 libEGL.so.1
    program apt-get "echo \"apt-get \$*\" >> '$WORLD/log'"
    program sudo "echo \"sudo \$*\" >> '$WORLD/log'"
    program cargo "echo \"cargo \$*\" >> '$WORLD/log'; echo cargo 1.0.0"
    program rustup "case \"\$1\" in --version) echo 'rustup 1.29.0';; toolchain) case \"\$2\" in list) echo '$CHANNEL-x86_64-unknown-linux-gnu (default)';; install) echo \"rustup \$*\" >> '$WORLD/log';; esac;; component) echo 'rustfmt-x86_64-unknown-linux-gnu'; echo 'clippy-x86_64-unknown-linux-gnu';; esac"
}

# ldconfig_with SONAME...: what `ldconfig -p` prints on this fake computer.
ldconfig_with() {
    {
        echo "$# libs found in cache"
        for soname in "$@"; do
            printf '\t%s (libc6,x86-64) => /usr/lib/x86_64-linux-gnu/%s\n' "$soname" "$soname"
        done
    } > "$WORLD/ldconfig.txt"
}

# run_script SCRIPT ARGS...: runs it in the world (stdin is not a terminal); sets OUT and CODE.
run_script() {
    script=$1
    shift
    OUT=$(PATH="$WORLD_BIN:$REAL_DIR" HOME="$WORLD/home" CARGO_HOME="$WORLD/home/.cargo" \
        "$SHELL_PATH" "$script" "$@" </dev/null 2>&1)
    CODE=$?
}

log() { cat "$WORLD/log"; }

# ---------------------------------------------------------------- the common helpers
helper() {
    "$SHELL_PATH" -c ". '$SCRIPTS/lib/setup-dev-common.sh'; $1"
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
expect_not_contains 'linux: curl is not asked for when rustup is there' "$OUT" 'curl'
expect 'linux: check installs nothing' "$(log)" ''

full_linux ninja
rm "$WORLD_BIN/ninja"
run_script "$LINUX" --check --skip-build
expect 'linux: check with a missing tool: exit code 1' "$CODE" 1
expect_contains 'linux: the missing tool is listed' "$OUT" 'MISSING  Ninja'
expect_contains 'linux: the package name is shown' "$OUT" 'apt: ninja-build'
expect 'linux: check with a missing tool installs nothing' "$(log)" ''

run_script "$LINUX" --skip-build
expect 'linux: no terminal and no --yes: exit code 1' "$CODE" 1
expect_contains 'linux: no terminal: says that no question can be asked' "$OUT" 'No question can be asked here'
expect_contains 'linux: the plan was shown before' "$OUT" 'system packages with apt (needs sudo): ninja-build'
expect 'linux: not agreed installs nothing' "$(log)" ''

run_script "$LINUX" --yes --skip-build
expect 'linux: --yes installs the missing package: exit code 0' "$CODE" 0
expect_contains 'linux: the step is announced' "$OUT" "this step runs 'sudo apt-get install'"
LOGTEXT=$(log)
expect 'linux: exactly the two apt calls, with sudo' "$LOGTEXT" 'sudo apt-get update
sudo apt-get install -y ninja-build'

# root needs no sudo
full_linux root
rm "$WORLD_BIN/ninja" "$WORLD_BIN/sudo"
FAKE_UID=0 run_script "$LINUX" --yes --skip-build
expect 'linux: as root: exit code 0' "$CODE" 0
expect 'linux: as root apt-get is called directly' "$(log)" 'apt-get update
apt-get install -y ninja-build'

# no root and no sudo: the names are printed, nothing installed
full_linux nosudo
rm "$WORLD_BIN/ninja" "$WORLD_BIN/sudo"
run_script "$LINUX" --yes --skip-build
expect 'linux: no sudo and not root: exit code 3' "$CODE" 3
expect_contains 'linux: says what to install as administrator' "$OUT" 'apt-get install ninja-build'
expect 'linux: nothing was installed' "$(log)" ''

# a window library that ldconfig does not list
full_linux lib
ldconfig_with libxkbcommon.so.0 libxkbcommon-x11.so.0 libwayland-client.so.0 libX11.so.6 libXi.so.6 libXrandr.so.2 libxcb.so.1 libGL.so.1 libEGL.so.1
run_script "$LINUX" --check --skip-build
expect_contains 'linux: a missing library is listed' "$OUT" 'MISSING  library libXcursor.so.1'
expect_contains 'linux: with its apt package' "$OUT" 'apt: libxcursor1'
run_script "$LINUX" --yes --skip-build
expect_contains 'linux: the library package is installed' "$(log)" 'sudo apt-get install -y libxcursor1'

# without ldconfig the libraries cannot be checked, which is said and is no failure
full_linux noldconfig
rm "$WORLD_BIN/ldconfig"
run_script "$LINUX" --check --skip-build
expect 'linux: no ldconfig: exit code 0' "$CODE" 0
expect_contains 'linux: no ldconfig: says it was not checked' "$OUT" 'not checked (ldconfig was not found)'

# nothing but the C compiler is needed to build: no development packages are asked for
full_linux nodev
if grep -Eq 'lib[a-z0-9.]+-dev([^a-z-]|$)' "$LINUX"; then fail 'linux: no development package is asked for'; else pass 'linux: no development package is asked for'; fi

# curl is only asked for when rustup has to be installed
full_linux curl
rm "$WORLD_BIN/curl"
run_script "$LINUX" --check --skip-build
expect 'linux: curl missing but rustup there: exit code 0' "$CODE" 0

# rustup and the toolchain are missing: the official installer is used without a default toolchain
full_linux rustup
rm "$WORLD_BIN/rustup" "$WORLD_BIN/cargo"
program curl "if [ \"\${1:-}\" = --version ]; then echo 'curl 8.0.0'; exit 0; fi
echo \"curl \$*\" >> '$WORLD/log'
while [ \$# -gt 0 ]; do [ \"\$1\" = -o ] && out=\$2; shift; done
cat > \"\$out\" <<'INSTALLER'
echo \"installer args: \$*\" >> '$WORLD/log'
mkdir -p \"\$HOME/.cargo/bin\"
printf '#!/bin/sh\necho \"rustup \$*\" >> $WORLD/log\ncase \"\$1\" in --version) echo rustup 1.29.0;; toolchain) if [ \"\$2\" = list ]; then echo \"$CHANNEL-x86_64-unknown-linux-gnu (default)\"; fi;; component) echo rustfmt; echo clippy;; esac\n' > \"\$HOME/.cargo/bin/rustup\"
chmod +x \"\$HOME/.cargo/bin/rustup\"
INSTALLER"
run_script "$LINUX" --check --skip-build
expect_contains 'linux: missing rustup is listed' "$OUT" 'MISSING  rustup'
expect_contains 'linux: and the toolchain' "$OUT" "MISSING  Rust toolchain $CHANNEL"
expect_contains 'linux: and curl, which is needed to install it' "$OUT" 'curl (to install rustup)'
run_script "$LINUX" --yes --skip-build
expect 'linux: rustup installation: exit code 0' "$CODE" 0
LOGTEXT=$(log)
expect_contains 'linux: the installer is downloaded to a file with failure checking' "$LOGTEXT" 'curl --proto =https --tlsv1.2 -sSf -o'
expect_contains 'linux: the installer is called without a default toolchain' "$LOGTEXT" 'installer args: -y --default-toolchain none --profile minimal'
expect_contains 'linux: then only the pinned toolchain is installed' "$LOGTEXT" "rustup toolchain install $CHANNEL --profile minimal -c rustfmt -c clippy"
expect_not_contains 'linux: no system package is installed for that' "$LOGTEXT" 'apt-get install'

# a failed download is an error, not a success (a fresh computer without rustup)
full_linux download
rm "$WORLD_BIN/rustup" "$WORLD_BIN/cargo"
program curl "if [ \"\${1:-}\" = --version ]; then echo 'curl 8.0.0'; exit 0; fi; echo 'curl: (6) Could not resolve host' >&2; exit 6"
run_script "$LINUX" --yes --skip-build
expect 'linux: a failed download: exit code 2' "$CODE" 2
expect_contains 'linux: says that the installer could not be downloaded' "$OUT" 'could not be downloaded'

# the toolchain is there but without clippy
full_linux components
program rustup "case \"\$1\" in --version) echo 'rustup 1.29.0';; toolchain) case \"\$2\" in list) echo '$CHANNEL-x86_64-unknown-linux-gnu (default)';; install) echo \"rustup \$*\" >> '$WORLD/log';; esac;; component) echo 'rustfmt-x86_64-unknown-linux-gnu';; esac"
run_script "$LINUX" --check --skip-build
expect_contains 'linux: a toolchain without clippy counts as missing' "$OUT" "MISSING  Rust toolchain $CHANNEL"
expect_contains 'linux: and says what to add' "$OUT" 'add rustfmt and clippy with rustup'
run_script "$LINUX" --yes --skip-build
expect_contains 'linux: the components are added by the toolchain installation' "$(log)" "rustup toolchain install $CHANNEL --profile minimal -c rustfmt -c clippy"

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
expect_contains 'other family: check lists the package names too' "$OUT" 'sudo dnf install'
run_script "$LINUX" --yes --skip-build
expect 'other family: only system packages missing: exit code 3' "$CODE" 3
expect_contains 'other family: the package names are printed' "$OUT" 'sudo dnf install'
expect_contains 'other family: for several families' "$OUT" 'sudo pacman -S'
expect 'other family: nothing was installed' "$(log)" ''

# arm64 (Raspberry Pi): the same code path
full_linux arm
program uname 'if [ "${1:-}" = -s ]; then echo Linux; else echo aarch64; fi'
run_script "$LINUX" --check --skip-build
expect 'linux arm64: exit code 0' "$CODE" 0

# started from another folder and through a link-free relative path
full_linux cwd
OUT=$(cd "$SCRIPTS" && PATH="$WORLD_BIN:$REAL_DIR" HOME="$WORLD/home" "$SHELL_PATH" setup-dev-linux.sh --check --skip-build </dev/null 2>&1)
expect 'linux: started from the scripts folder with a bare name' "$?" 0

# options
run_script "$LINUX" --help
expect 'help: exit code 0' "$CODE" 0
expect_contains 'help: shows the usage' "$OUT" 'Usage:'
run_script "$LINUX" -y --check --skip-build
expect 'the short -y is accepted' "$CODE" 0
run_script "$LINUX" --frobnicate
expect 'an unknown option: exit code 2' "$CODE" 2

# ---------------------------------------------------------------- syntax with the strictest shell
for script in "$SCRIPTS/lib/setup-dev-common.sh" "$LINUX" "$SCRIPTS/setup-dev-macos.sh"; do
    [ -f "$script" ] || continue
    if "$SHELL_PATH" -n "$script" 2>/dev/null; then pass "syntax: $(basename "$script") parses with $SHELL_NAME"; else fail "syntax: $(basename "$script")"; fi
done

echo
if [ "$failed" -gt 0 ]; then echo "$failed of $total check(s) failed"; exit 1; fi
echo "all $total checks passed"
