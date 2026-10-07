#!/bin/sh
#
# deb_lock's action (defs.bzl): downloads the snapshot's package indexes for
# each architecture a set names and has buck-tools resolve every set into
# packages.bzl.
#
# Usage: deb_lock.sh OUT RESOLVER BASE INDEXES SET...
#   INDEXES is the space-separated dists/<suite>/<component> list, and each SET
#   is name:architecture:package,package,...
set -eu
out="$1"; resolver="$2"; base="$3"; indexes="$4"; shift 4
scratch="$(mktemp -d)"
for set in "$@"; do
    architecture="$(echo "$set" | cut -d: -f2)"
    directory="$scratch/$architecture"
    [ -d "$directory" ] && continue
    mkdir -p "$directory"
    number=0
    for index in $indexes; do
        curl --fail --silent --show-error --location --retry 5 \
            --output "$directory/$number.xz" "$base/$index/binary-$architecture/Packages.xz"
        xz --decompress "$directory/$number.xz"
        number=$((number + 1))
    done
done
"$resolver" resolve "$base" "$scratch" "$@" > "$out"
rm -rf "$scratch"
