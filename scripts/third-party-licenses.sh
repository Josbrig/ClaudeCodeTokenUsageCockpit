#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
#
# Makes THIRD_PARTY_LICENSES.md (REQ-107) with cargo about; with --check it only checks: cargo deny
# must accept every licence and the committed file must be the one that would be written now.
#
#   sh scripts/third-party-licenses.sh           # write the file
#   sh scripts/third-party-licenses.sh --check   # exit code 1 if a licence is not allowed or the file is old
#
# Needs the tools: cargo install cargo-about cargo-deny --locked

set -u
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT" || exit 2

for tool in cargo; do
    command -v "$tool" >/dev/null 2>&1 || { echo "$tool is not installed." >&2; exit 2; }
done
cargo about --version >/dev/null 2>&1 || { echo 'cargo-about is not installed: cargo install cargo-about cargo-deny --locked' >&2; exit 2; }

fresh=$(mktemp) || exit 2
trap 'rm -f "$fresh"' EXIT
cargo about generate --workspace --output-file "$fresh" about.hbs || { echo 'cargo about failed.' >&2; exit 2; }
# the same shape as the Windows script writes: one final line end
body=$(tr -d '\r' < "$fresh")
printf '%s\n' "$body" > "$fresh"

if [ "${1:-}" = --check ]; then
    cargo deny check licenses || { echo 'A licence is not allowed (see above).'; exit 1; }
    if ! cmp -s "$fresh" THIRD_PARTY_LICENSES.md; then
        echo 'THIRD_PARTY_LICENSES.md is out of date. Run scripts/third-party-licenses.sh and commit the file.'
        exit 1
    fi
    echo 'Licences allowed and THIRD_PARTY_LICENSES.md is up to date.'
else
    cp "$fresh" THIRD_PARTY_LICENSES.md
    echo "Wrote $REPO_ROOT/THIRD_PARTY_LICENSES.md"
fi
