#!/usr/bin/env bash
#
# Puts the pinned starlark_fmt in cargo's bin directory.
#
# It is built and released alongside buck2, out of the same commit, so it is
# pinned by the same version string. ./scripts/verify runs it over the BUCK
# files the way it runs rustfmt over the Rust.
#
# Usage:
#   install-starlark-fmt.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-starlark-fmt.sh' >&2
    exit 1
fi

if command -v starlark_fmt > /dev/null 2>&1; then
    echo 'starlark_fmt is already installed.'
    exit 0
fi

assert_command zstd 'Install zstd (apt install zstd, brew install zstd).'

triple="$(buck2_triple)"
case "$triple" in
    x86_64-*) sha256="$starlark_fmt_sha256_x86_64" ;;
    aarch64-*) sha256="$starlark_fmt_sha256_aarch64" ;;
esac

release="starlark_fmt-$triple.zst"
url="https://github.com/facebook/buck2/releases/download/$buck2_version/$release"
mirror="$download_mirror/$buck2_version/$release"
tools="$repository/target/tools"
archive="$tools/$release"
bin_directory="${CARGO_HOME:-$HOME/.cargo}/bin"

echo "Downloading starlark_fmt $buck2_version from $url..."
mkdir -p "$tools" "$bin_directory"
download_verified "$url" "$archive" "$sha256" "$mirror"
zstd --decompress --force --quiet "$archive" -o "$bin_directory/starlark_fmt"
chmod +x "$bin_directory/starlark_fmt"
rm "$archive"

echo "Installed starlark_fmt at $bin_directory/starlark_fmt"
