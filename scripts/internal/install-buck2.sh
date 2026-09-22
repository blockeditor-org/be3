#!/usr/bin/env bash
#
# Puts the pinned buck2 in cargo's bin directory.
#
# The release upstream publishes is a zstd-compressed bare binary rather than an
# archive, so this decompresses it straight to where it is going, the way
# install-nextest.sh untars one binary out of a tarball. The version and the
# hash it is checked against are both common.sh's.
#
# A buck2 already on PATH is left exactly as it is, whatever version it is: the
# machine chose it, and replacing it is not this script's business. A version
# other than the pinned one will read the same BUCK files with a different
# prelude, which is worth knowing about, so it says which one it found.
#
# Usage:
#   install-buck2.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-buck2.sh' >&2
    exit 1
fi

if command -v buck2 > /dev/null 2>&1; then
    echo "buck2 is already installed: $(buck2 --version)"
    exit 0
fi

assert_command zstd 'Install zstd (apt install zstd, brew install zstd).'

triple="$(buck2_triple)"
case "$triple" in
    x86_64-*) sha256="$buck2_sha256_x86_64" ;;
    aarch64-*) sha256="$buck2_sha256_aarch64" ;;
esac

release="buck2-$triple.zst"
url="https://github.com/facebook/buck2/releases/download/$buck2_version/$release"
mirror="$download_mirror/$buck2_version/$release"
tools="$repository/target/tools"
archive="$tools/$release"
bin_directory="${CARGO_HOME:-$HOME/.cargo}/bin"

echo "Downloading buck2 $buck2_version from $url..."
mkdir -p "$tools" "$bin_directory"
download_verified "$url" "$archive" "$sha256" "$mirror"
zstd --decompress --force --quiet "$archive" -o "$bin_directory/buck2"
chmod +x "$bin_directory/buck2"
rm "$archive"

echo "Installed $("$bin_directory/buck2" --version) at $bin_directory/buck2"
