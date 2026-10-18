#!/usr/bin/env bash
#
# Installs the pinned be release into the checkout, where ./scripts/be runs it
# from: target/tools/be-<release>/be, with a build-id file beside it naming the
# release, which is the build the server must be. ./scripts/setup runs this, and
# ./scripts/be runs it by itself when that file is not there yet. The release
# and the hashes it is checked against are common.sh's.
#
# Usage:
#   install-be.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-be.sh' >&2
    exit 1
fi

if [[ -z "$be_release" ]]; then
    echo 'No be release is pinned yet (be_release in scripts/internal/common.sh).' >&2
    exit 1
fi

destination="$(be_path)"
directory="$(dirname "$destination")"
if [[ -x "$destination" && -f "$directory/$be_metadata_file" ]]; then
    echo "be $be_release is already installed at $destination"
    exit 0
fi

triple="$(buck2_triple)"
sha256="$(buck2_release_sha256 be "$triple")"
url="$be_mirror/$be_release/be-$triple/be"

echo "Downloading be $be_release from $url..." >&2
mkdir -p "$directory"
download_verified "$url" "$destination.partial" "$sha256"
chmod +x "$destination.partial"
if [[ "$be_metadata_file" == build-id ]]; then
    printf '%s\n' "$be_release" > "$directory/$be_metadata_file"
else
    printf '{"id":"%s","app_id":"Block"}\n' "$be_release" > "$directory/$be_metadata_file"
fi
mv -f "$destination.partial" "$destination"
echo "Installed be $be_release at $destination" >&2
