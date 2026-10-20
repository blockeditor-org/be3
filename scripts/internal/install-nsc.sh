#!/usr/bin/env bash
#
# Downloads the pinned nsc, Namespace's command-line tool, for the host that
# runs `be-server namespace`, which wakes the remote execution cluster and
# names its hosts for every machine that builds (guides/build_server.md). It
# lands in target/tools/nsc-<version>-<platform>/, and its path is printed.
#
# Usage:
#   install-nsc.sh [linux_amd64 | linux_arm64]
#
# The platform defaults to this machine's.

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

# Its archives are checked against these hashes, by the platform upstream
# names them after.
nsc_version='0.0.587'
nsc_sha256_linux_amd64='f47a9eff06f41d0f342277ba097e04a215bd88019f184271042f8eb5ec94018f'
nsc_sha256_linux_arm64='fc121a4fda8083f9e2815a2ecb4e75d06649b98316d4c793434d03fc443a1516'

if [[ $# -gt 1 ]]; then
    echo 'Usage: install-nsc.sh [linux_amd64 | linux_arm64]' >&2
    exit 1
fi

if [[ $# -eq 1 ]]; then
    platform="$1"
else
    case "$(uname -m)" in
        x86_64 | amd64) platform='linux_amd64' ;;
        aarch64 | arm64) platform='linux_arm64' ;;
        *) platform="linux_$(uname -m)" ;;
    esac
fi
sha256="$(buck2_release_sha256 nsc "$platform")"
destination="$repository/target/tools/nsc-$nsc_version-$platform/nsc"
if [[ -x "$destination" ]]; then
    echo "$destination"
    exit 0
fi

release="nsc_${nsc_version}_$platform.tar.gz"
url="https://get.namespace.so/packages/nsc/v$nsc_version/$release"
directory="$(dirname "$destination")"
archive="$directory/$release"

echo "Downloading nsc $nsc_version from $url..." >&2
mkdir -p "$directory"
download_verified "$url" "$archive" "$sha256"
# Unpacked beside the destination and moved into place, so that an interrupted
# install leaves nothing that looks like a working nsc.
unpacked="$directory/unpacked"
rm -rf "$unpacked"
mkdir -p "$unpacked"
tar -xzf "$archive" -C "$unpacked" --no-same-owner nsc
chmod +x "$unpacked/nsc"
mv -f "$unpacked/nsc" "$destination"
rm -rf "$unpacked" "$archive"

echo "$destination"
