#!/bin/sh
#
# deb_lock's action (defs.bzl): unpacks the snapshot's package indexes for
# each architecture and has buck-tools resolve every set into packages.bzl.
#
# Usage: deb_lock.sh OUT RESOLVER BASE ARCHITECTURE=INDEX... -- SET...
#   each INDEX a Packages.xz for ARCHITECTURE, and each SET
#   name:architecture:package,package,...
set -eu
out="$1"; resolver="$2"; base="$3"; shift 3
scratch="$(mktemp -d)"
number=0
while [ "$1" != "--" ]; do
    architecture="${1%%=*}"
    mkdir -p "$scratch/$architecture"
    xz --decompress --stdout "${1#*=}" > "$scratch/$architecture/$number"
    number=$((number + 1))
    shift
done
shift
"$resolver" resolve "$base" "$scratch" "$@" > "$out"
rm -rf "$scratch"
