#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
#
# Checks, and on request installs, the tools needed to develop usage-cockpit on Linux (x64 and
# arm64, including Raspberry Pi OS).
#
#   sh scripts/setup-dev-linux.sh --check    # report only, installs nothing
#   sh scripts/setup-dev-linux.sh            # report, ask, install what is missing, build once
#
# Needed: Rust (the toolchain pinned in rust-toolchain.toml, with rustfmt and clippy) through
# rustup, a C compiler and linker, pkg-config, Git, curl, CMake and Ninja (for the CMake builds)
# and the development packages of the window toolkit (X11 and Wayland, xkbcommon, OpenGL).
#
# Debian, Ubuntu and Raspberry Pi OS (apt): the missing packages are installed with `sudo
# apt-get install`; sudo is used for that step only, and the script says so before it asks. Other
# families (Fedora, Arch, openSUSE): the script checks, lists the missing things with the package
# names, installs only rustup and the toolchain (they need no root rights) and stops with exit
# code 3 if system packages are missing.
#
# It installs only what is missing and can be run again. It contains no secrets and changes
# nothing outside the places of the tools. Options and exit codes: see --help.

set -u

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
# shellcheck source=lib/setup-dev-common.sh
. "$SCRIPT_DIR/lib/setup-dev-common.sh"

# The libraries of the window toolkit, as pkg-config knows them: "module:apt package".
LIBRARIES="xkbcommon:libxkbcommon-dev wayland-client:libwayland-dev x11:libx11-dev xcursor:libxcursor-dev xi:libxi-dev xrandr:libxrandr-dev gl:libgl1-mesa-dev"

# The packages for the other families, only printed.
OTHER_FAMILIES='Fedora:  sudo dnf install gcc gcc-c++ make pkgconf-pkg-config cmake ninja-build git curl libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel mesa-libGL-devel
Arch:    sudo pacman -S base-devel pkgconf cmake ninja git curl libxkbcommon wayland libx11 libxcursor libxi libxrandr mesa
openSUSE: sudo zypper install gcc gcc-c++ make pkg-config cmake ninja git curl libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel Mesa-libGL-devel'

APT_PACKAGES=""

add_apt_package() {
    case " $APT_PACKAGES " in *" $1 "*) ;; *) APT_PACKAGES="$APT_PACKAGES $1" ;; esac
}

check_tools() {
    channel=$(toolchain_channel "$REPO_ROOT/rust-toolchain.toml")
    use_cargo_bin
    state_program rustup rustup rustup 'install with the official installer (curl | sh), without a default toolchain'
    state_toolchain "$channel"
    state_program cc cc 'C compiler (cc)' 'apt: build-essential'
    state_program pkgconfig pkg-config pkg-config 'apt: pkg-config'
    state_program git git Git 'apt: git'
    state_program curl curl curl 'apt: curl'
    state_program cmake cmake CMake 'apt: cmake'
    state_program ninja ninja Ninja 'apt: ninja-build'
    is_missing cc && add_apt_package build-essential
    is_missing pkgconfig && add_apt_package pkg-config
    is_missing git && add_apt_package git
    is_missing curl && add_apt_package curl
    is_missing cmake && add_apt_package cmake
    is_missing ninja && add_apt_package ninja-build
    if have pkg-config; then
        for entry in $LIBRARIES; do
            module=${entry%%:*}
            package=${entry#*:}
            if pkg-config --exists "$module" 2>/dev/null; then
                state "lib-$module" "library $module" 1 "$(pkg-config --modversion "$module" 2>/dev/null)"
            else
                state "lib-$module" "library $module" 0 "apt: $package"
                add_apt_package "$package"
            fi
        done
    else
        state libraries 'window libraries' 0 'cannot be checked without pkg-config'
        for entry in $LIBRARIES; do add_apt_package "${entry#*:}"; done
    fi
}

system_missing() {
    [ -n "$APT_PACKAGES" ]
}

install_system_packages() {
    echo
    echo "The system packages need administrator rights: this step runs 'sudo apt-get install'."
    if [ "$(id -u)" = 0 ]; then
        apt-get update && apt-get install -y $APT_PACKAGES
    elif have sudo; then
        sudo apt-get update && sudo apt-get install -y $APT_PACKAGES
    else
        echo 'Neither root rights nor sudo: install these packages by hand:' >&2
        echo "  apt-get install$APT_PACKAGES" >&2
        return 1
    fi
}

main() {
    parse_args "$@"
    if [ "$(uname -s)" != Linux ]; then
        echo "This script is for Linux; this system is $(uname -s). See docs/development-guide.md for the other systems." >&2
        exit 3
    fi
    check_tools
    print_states
    if [ "$MISSING_COUNT" = 0 ]; then
        echo 'Everything is there.'
    else
        echo
        echo "$MISSING_COUNT missing."
        if [ "$CHECK_ONLY" = 1 ]; then
            echo 'Nothing was installed (--check). Run without --check to install.'
            exit 1
        fi
        if system_missing && ! have apt-get; then
            echo 'The system packages that are missing cannot be installed by this script on this system.'
            echo 'Install them with your package manager (names for the common families):'
            printf '%s\n' "$OTHER_FAMILIES"
            echo 'Rust itself can still be installed by this script (it needs no root rights).'
        fi
        installable=0
        is_missing rustup && installable=1
        is_missing toolchain && installable=1
        have apt-get && system_missing && installable=1
        if [ "$installable" = 0 ]; then
            echo
            echo 'There is nothing in the list that this script can install here; install the system packages above by hand and run it again.'
            exit 3
        fi
        echo
        echo 'This would be installed:'
        is_missing rustup && echo '  - rustup (official installer, without a default toolchain)'
        { is_missing rustup || is_missing toolchain; } && echo "  - the Rust toolchain ${channel:-stable} with rustfmt and clippy"
        if have apt-get && system_missing; then
            echo "  - system packages with apt (sudo):$APT_PACKAGES"
        fi
        confirm 'Install the missing tools?' || { echo 'Nothing was installed.'; exit 1; }
        if have apt-get && system_missing; then
            install_system_packages || exit 2
        fi
        if is_missing rustup; then
            install_rustup || exit 2
        fi
        if is_missing rustup || is_missing toolchain; then
            install_toolchain "$channel" || exit 2
        fi
        echo
        echo 'Done. Open a new terminal so that the PATH of the new tools is in effect.'
        if system_missing && ! have apt-get; then
            echo 'The system packages above are still missing.'
            exit 3
        fi
    fi
    if [ "$SKIP_BUILD" = 0 ] && [ "$CHECK_ONLY" = 0 ]; then
        build_once || { echo 'cargo build failed.' >&2; exit 2; }
    fi
    print_commands
    exit 0
}

# Run only when the script is started, not when the tests read it with `.`.
case "${0##*/}" in
    setup-dev-linux.sh) main "$@" ;;
esac
