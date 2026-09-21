#!/usr/bin/env bash
#
# Puts the pinned cargo-nextest in cargo's bin directory, where `cargo nextest`
# looks for it.
#
# A prebuilt release is fetched rather than built with `cargo install`, which
# spent around four minutes compiling a test runner nothing in the workspace
# depends on and was most of what setting a machine up cost. The version and the
# hash it is checked against are both common.sh's, so a machine runs the tests
# through a runner someone looked at rather than whatever was newest that day.
#
# A cargo-nextest that is already on PATH is left exactly as it is, whatever
# version it is: verify only needs one that runs, and replacing the one a
# machine chose for itself is not this script's business.
#
# Usage:
#   install-nextest.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-nextest.sh' >&2
    exit 1
fi

if cargo nextest --version > /dev/null 2>&1; then
    echo "cargo-nextest is already installed: $(cargo nextest --version | head -1)"
    exit 0
fi

case "$(uname -m)" in
    x86_64 | amd64)
        architecture='x86_64'
        sha256="$nextest_sha256_x86_64"
        ;;
    aarch64 | arm64)
        architecture='aarch64'
        sha256="$nextest_sha256_aarch64"
        ;;
    *)
        echo "nextest publishes no release for $(uname -m)." >&2
        echo 'Install it with `cargo install cargo-nextest --locked` instead.' >&2
        exit 1
        ;;
esac

release="cargo-nextest-$nextest_version-$architecture-unknown-linux-musl"
url="https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$nextest_version/$release.tar.gz"
tools="$repository/target/tools"
archive="$tools/nextest.tar.gz"
bin_directory="${CARGO_HOME:-$HOME/.cargo}/bin"

echo "Downloading cargo-nextest from $url..."
mkdir -p "$tools" "$bin_directory"
download_verified "$url" "$archive" "$sha256"
# The archive holds the bare binary, so it is extracted straight to where cargo
# resolves its subcommands from - the same place `cargo install` would have put
# the one this replaces.
tar -xzf "$archive" -C "$bin_directory" cargo-nextest
rm "$archive"

echo "Installed $("$bin_directory/cargo-nextest" nextest --version | head -1) at $bin_directory/cargo-nextest"
