#!/usr/bin/env bash
#
# Installs the pinned Bazel into the checkout, where ./scripts/bazel runs it
# from: target/tools/bazel-<version>/. ./scripts/bazel runs this by itself when
# that file is not there yet, so nobody has to. The version and the hash it is
# checked against are both common.sh's.
#
# A Bazel on PATH, or bazelisk, is not used: .bazelversion names the same
# version, but only this one is checked against a hash.
#
# Usage:
#   install-bazel.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-bazel.sh' >&2
    exit 1
fi

destination="$(bazel_path)"
if [[ -x "$destination" ]]; then
    echo "Bazel $bazel_version is already installed at $destination"
    exit 0
fi

triple="$(host_triple)"
sha256="$(release_sha256 bazel "$triple")"
asset="$(bazel_asset "$triple")"
mkdir -p "$(dirname "$destination")"
download_verified "https://releases.bazel.build/$bazel_version/release/$asset" "$destination.partial" "$sha256"
chmod +x "$destination.partial"
mv -f "$destination.partial" "$destination"
echo "Installed Bazel $bazel_version at $destination"
