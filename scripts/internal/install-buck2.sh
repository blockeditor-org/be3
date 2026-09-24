#!/usr/bin/env bash
#
# Installs the pinned buck2 into the checkout, where ./scripts/buck runs it
# from: target/tools/buck2-<version>/. ./scripts/buck runs this by itself when
# that file is not there yet, so nobody has to.
#
# The release upstream publishes is a zstd-compressed bare binary rather than an
# archive, so this decompresses it straight to where it is going, with zstd if
# the machine has it and otherwise with the zstd Python has had since 3.14. The
# version and the hash it is checked against are both common.sh's.
#
# A buck2 on PATH is not used: every buck2 carries the prelude it was built
# with, so the pinned one is the only one that reads the BUCK files the way
# they were written for.
#
# Usage:
#   install-buck2.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-buck2.sh' >&2
    exit 1
fi

destination="$(buck2_path)"
if [[ -x "$destination" ]]; then
    echo "buck2 $buck2_version is already installed at $destination"
    exit 0
fi

# The decompressor, as a command that reads the archive named by its first
# argument and writes the binary to its second.
decompress=''
if command -v zstd > /dev/null 2>&1; then
    decompress='zstd'
else
    for python in python3 python py; do
        if command -v "$python" > /dev/null 2>&1 \
            && "$python" -c 'import compression.zstd' > /dev/null 2>&1; then
            decompress="$python"
            break
        fi
    done
fi
if [[ -z "$decompress" ]]; then
    echo 'buck2 is released as a zstd stream, and there is nothing here to decompress it:' >&2
    echo 'install zstd (apt install zstd, brew install zstd, winget install Meta.Zstandard)' >&2
    echo 'or Python 3.14 or newer.' >&2
    exit 1
fi

triple="$(buck2_triple)"
sha256="$(buck2_release_sha256 buck2 "$triple")"

release="buck2-$triple$(buck2_release_suffix)"
url="https://github.com/facebook/buck2/releases/download/$buck2_version/$release"
mirror="$download_mirror/$buck2_version/$release"
directory="$(dirname "$destination")"
archive="$directory/$release"

echo "Downloading buck2 $buck2_version from $url..." >&2
mkdir -p "$directory"
download_verified "$url" "$archive" "$sha256" "$mirror"
# Written beside the destination and moved into place, so that an interrupted
# install leaves nothing ./scripts/buck would take for a working buck2.
if [[ "$decompress" == 'zstd' ]]; then
    zstd --decompress --force --quiet "$archive" -o "$destination.partial"
else
    "$decompress" -c '
import sys
from compression import zstd
with open(sys.argv[1], "rb") as source, open(sys.argv[2], "wb") as target:
    target.write(zstd.decompress(source.read()))
' "$(native_path "$archive")" "$(native_path "$destination.partial")"
fi
chmod +x "$destination.partial"
mv -f "$destination.partial" "$destination"
rm -f "$archive"

echo "Installed $("$destination" --version) at $destination" >&2
