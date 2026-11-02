#!/bin/sh
#
# Writes OUT, every plugin's manifest and its type module in one file, for an
# executable that carries them (be): for each plugin, its manifest's length as
# 8 hex digits and the manifest, then its type module's the same way (length 0
# for a plugin with no module of its own).
#
# Usage: bundle.sh OUT (MANIFEST [MODULE])...   each as MANIFEST=MODULE or MANIFEST=
set -eu
out="$1"
shift
: > "$out"
record() {
    if [ -n "$1" ]; then
        printf '%08x' "$(wc -c < "$1")" >> "$out"
        cat "$1" >> "$out"
    else
        printf '%08x' 0 >> "$out"
    fi
}
for argument in "$@"; do
    record "${argument%%=*}"
    record "${argument#*=}"
done
