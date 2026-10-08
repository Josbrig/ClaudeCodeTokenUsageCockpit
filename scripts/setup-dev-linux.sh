#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
#
# Checks, and on request installs, the tools needed to develop usage-cockpit on Linux (x64 and
# arm64, including Raspberry Pi OS).
#
#   sh scripts/setup-dev-linux.sh --check    # report only, installs nothing
#   sh scripts/setup-dev-linux.sh            # report, ask, install what is missing, build once
#
# What is needed:
#   - to build: Rust (the toolchain pinned in rust-toolchain.toml, with rustfmt and clippy)
#     through rustup, and a C compiler and linker (`cc`). The window libraries are loaded when the
#     program runs (dlopen), so no development packages are needed to build it.
#   - to run the window: the libraries of X11, Wayland, xkbcommon and OpenGL/EGL (checked through
#     `ldconfig -p`; a desktop normally has them, a minimal server image does not).
#   - Git, CMake and Ninja (for the CMake builds), and curl (only to install rustup).
#
# Debian, Ubuntu and Raspberry Pi OS (apt): the missing packages are installed with `sudo
# apt-get install`; sudo is used for that step only, and the plan says so before it asks. Other
# families (Fedora, Arch, openSUSE), or no root rights and no sudo: the script lists the package
# names, installs only rustup and the toolchain (no root rights needed) and exits with code 3 if
# system packages are still missing.
#
# It installs only what is missing and can be run again. It contains no secrets. The official
# rustup installer adds ~/.cargo/bin to the PATH in your shell profile; nothing else outside the
# places of the tools is changed. Options and exit codes: see --help.

set -u

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
# shellcheck source=lib/setup-dev-common.sh
. "$SCRIPT_DIR/lib/setup-dev-common.sh"

# The libraries that the window loads when it runs: "soname apt-package".
RUNTIME_LIBS='libxkbcommon.so.0 libxkbcommon0
libxkbcommon-x11.so.0 libxkbcommon-x11-0
libwayland-client.so.0 libwayland-client0
libX11.so.6 libx11-6
libXcursor.so.1 libxcursor1
libXi.so.6 libxi6
libXrandr.so.2 libxrandr2
libxcb.so.1 libxcb1
libGL.so.1 libgl1
libEGL.so.1 libegl1'

# The package names for the other families, only printed.
OTHER_FAMILIES='Fedora:   sudo dnf install gcc make cmake ninja-build git curl libxkbcommon libxkbcommon-x11 libwayland-client libX11 libXcursor libXi libXrandr libxcb mesa-libGL mesa-libEGL
Arch:     sudo pacman -S base-devel cmake ninja git curl libxkbcommon libxkbcommon-x11 wayland libx11 libxcursor libxi libxrandr libxcb libglvnd
openSUSE: sudo zypper install gcc make cmake ninja git curl libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libX11-6 libXcursor1 libXi6 libXrandr2 libxcb1 Mesa-libGL1 Mesa-libEGL1'

APT_PACKAGES=""

add_apt_package() {
    case " $APT_PACKAGES " in *" $1 "*) ;; *) APT_PACKAGES="$APT_PACKAGES $1" ;; esac
}

# ldconfig lives in /sbin or /usr/sbin, which a normal user's PATH often lacks.
find_ldconfig() {
    for candidate in ldconfig /sbin/ldconfig /usr/sbin/ldconfig; do
        if command -v "$candidate" >/dev/null 2>&1; then
            command -v "$candidate"
            return 0
        fi
    done
    return 1
}

check_tools() {
    channel=$(toolchain_channel "$REPO_ROOT/rust-toolchain.toml")
    use_cargo_bin
    state_program rustup rustup rustup 'install with the official installer (curl | sh), without a default toolchain'
    state_toolchain "$channel"
    state_program cc cc 'C compiler (cc)' 'apt: build-essential'
    state_program git git Git 'apt: git'
    state_program cmake cmake CMake 'apt: cmake'
    state_program ninja ninja Ninja 'apt: ninja-build'
    is_missing cc && add_apt_package build-essential
    is_missing git && add_apt_package git
    is_missing cmake && add_apt_package cmake
    is_missing ninja && add_apt_package ninja-build
    # curl is only needed to install rustup
    if is_missing rustup; then
        state_program curl curl 'curl (to install rustup)' 'apt: curl'
        is_missing curl && add_apt_package curl
    fi
    if ldconfig_path=$(find_ldconfig); then
        ldconfig_list=$("$ldconfig_path" -p 2>/dev/null || true)
        old_ifs=$IFS
        IFS='
'
        for entry in $RUNTIME_LIBS; do
            IFS=$old_ifs
            soname=${entry%% *}
            package=${entry#* }
            case "$ldconfig_list" in
                *"$soname ("*) state "lib-$soname" "library $soname" 1 'found' ;;
                *)
                    state "lib-$soname" "library $soname" 0 "apt: $package"
                    add_apt_package "$package"
                    ;;
            esac
            IFS='
'
        done
        IFS=$old_ifs
    else
        state libraries 'window libraries' 1 'not checked (ldconfig was not found)'
    fi
}

system_missing() {
    [ -n "$APT_PACKAGES" ]
}

# Can this script install system packages here: apt-get, and root rights or sudo.
apt_usable() {
    have apt-get && { [ "$(id -u)" = 0 ] || have sudo; }
}

install_system_packages() {
    echo
    echo "The system packages need administrator rights: this step runs 'sudo apt-get install'."
    if [ "$(id -u)" = 0 ]; then
        apt-get update || echo 'apt-get update failed; trying the installation anyway.'
        # shellcheck disable=SC2086 # the names are a fixed list without glob characters
        apt-get install -y $APT_PACKAGES
    else
        sudo apt-get update || echo 'apt-get update failed; trying the installation anyway.'
        # shellcheck disable=SC2086 # the names are a fixed list without glob characters
        sudo apt-get install -y $APT_PACKAGES
    fi
}

print_other_families() {
    echo
    if have apt-get; then
        echo 'apt-get is there, but neither root rights nor sudo: install these packages as administrator:'
        echo "  apt-get install$APT_PACKAGES"
    else
        echo 'This script installs system packages only with apt. Install them with your package manager (names for the common families):'
        printf '%s\n' "$OTHER_FAMILIES"
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
        if system_missing && ! apt_usable; then
            print_other_families
        fi
        if [ "$CHECK_ONLY" = 1 ]; then
            echo
            echo 'Nothing was installed (--check). Run without --check to install.'
            exit 1
        fi
        installable=0
        is_missing rustup && installable=1
        is_missing toolchain && installable=1
        system_missing && apt_usable && installable=1
        if [ "$installable" = 0 ]; then
            echo
            echo 'There is nothing in the list that this script can install here; install the system packages above by hand and run it again.'
            exit 3
        fi
        echo
        echo 'This would be installed:'
        is_missing rustup && echo '  - rustup (official installer, without a default toolchain)'
        { is_missing rustup || is_missing toolchain; } && echo "  - the Rust toolchain ${channel:-stable} with rustfmt and clippy"
        if system_missing && apt_usable; then
            echo "  - system packages with apt (needs sudo):$APT_PACKAGES"
        fi
        confirm 'Install the missing tools?' || { echo 'Nothing was installed.'; exit 1; }
        if system_missing && apt_usable; then
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
        if system_missing && ! apt_usable; then
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

main "$@"
