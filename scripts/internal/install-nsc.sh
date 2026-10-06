#!/usr/bin/env bash
#
# Installs the pinned nsc, Namespace's command-line tool, into the checkout,
# where ./scripts/buck runs it from: target/tools/nsc-<version>/. ./scripts/buck
# runs this by itself when that file is not there yet. nsc is what wakes the
# remote execution cluster and names its hosts (guides/build_server.md).
#
# Usage:
#   install-nsc.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-nsc.sh' >&2
    exit 1
fi

destination="$(nsc_path)"
if [[ -x "$destination" ]]; then
    echo "nsc $nsc_version is already installed at $destination"
    exit 0
fi

platform="$(nsc_platform)"
sha256="$(buck2_release_sha256 nsc "$platform")"
extension='tar.gz'
if [[ "$platform" == windows_* ]]; then
    extension='zip'
fi
release="nsc_${nsc_version}_$platform.$extension"
url="https://get.namespace.so/packages/nsc/v$nsc_version/$release"
directory="$(dirname "$destination")"
archive="$directory/$release"

echo "Downloading nsc $nsc_version from $url..." >&2
mkdir -p "$directory"
download_verified "$url" "$archive" "$sha256"
# Unpacked beside the destination and moved into place, so that an interrupted
# install leaves nothing ./scripts/buck would take for a working nsc.
unpacked="$directory/unpacked"
rm -rf "$unpacked"
mkdir -p "$unpacked"
if [[ "$extension" == 'zip' ]]; then
    assert_command unzip 'Install unzip.'
    unzip -q "$archive" "nsc$(buck2_binary_suffix)" -d "$unpacked"
else
    tar -xzf "$archive" -C "$unpacked" --no-same-owner nsc
fi
chmod +x "$unpacked/nsc$(buck2_binary_suffix)"
mv -f "$unpacked/nsc$(buck2_binary_suffix)" "$destination"
rm -rf "$unpacked" "$archive"

echo "Installed nsc $nsc_version at $destination" >&2
