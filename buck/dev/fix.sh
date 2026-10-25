#!/bin/sh
#
# The autofixes as an action, which buck/dev/verify.bxl runs on a worker: a
# copy of SOURCES, fixed, compared with SOURCES. OUT/changed holds every file
# the fixes changed or added, at its path in the repository, and OUT/deleted
# names every file they removed, one a line; ./scripts/verify copies the one
# into the checkout and deletes the other. OUT/original holds each of those
# files as the fixes read it, and ./scripts/verify touches only the files that
# still read the same: an output left behind by an earlier run, when this one
# could not run, is not this checkout's fix.
#
# rust fixes one crate's Rust files with fix-rust-source, then rustfmt. starlark formats the BUCK and .bzl files with starlark_fmt.
#
# Usage:
#   fix.sh rust OUT SOURCES FIX_RUST_SOURCE RUSTFMT_SYSROOT
#   fix.sh starlark OUT SOURCES STARLARK_FMT CONFIG
set -eu
here="$(pwd)"
absolute() {
    case "$1" in
        /*) echo "$1" ;;
        *) echo "$here/$1" ;;
    esac
}
kind="$1" out="$(absolute "$2")" sources="$(absolute "$3")"
shift 3
tree="$(mktemp -d)"
mkdir -p "$out/changed" "$out/original"
: > "$out/deleted"
cp -RL "$sources/." "$tree/"
chmod -R u+w "$tree"
case "$kind" in
    rust)
        fix_rust_source="$(absolute "$1")" rustfmt="$(absolute "$2")/bin/rustfmt"
        (cd "$tree" && "$fix_rust_source")
        find "$tree" -type f -name '*.rs' -exec "$rustfmt" --edition 2024 {} +
        ;;
    starlark)
        starlark_fmt="$(absolute "$1")" config="$(absolute "$2")"
        find "$tree" -type f -exec "$starlark_fmt" --config "$config" fmt {} +
        ;;
    *)
        echo "unknown kind $kind" >&2
        exit 1
        ;;
esac
(cd "$sources" && find . -type f) | while IFS= read -r path; do
    [ -f "$tree/$path" ] && continue
    echo "${path#./}" >> "$out/deleted"
    mkdir -p "$(dirname "$out/original/$path")"
    cp "$sources/$path" "$out/original/$path"
done
(cd "$tree" && find . -type f) | while IFS= read -r path; do
    cmp -s "$sources/$path" "$tree/$path" && continue
    mkdir -p "$(dirname "$out/changed/$path")"
    cp "$tree/$path" "$out/changed/$path"
    if [ -f "$sources/$path" ]; then
        mkdir -p "$(dirname "$out/original/$path")"
        cp "$sources/$path" "$out/original/$path"
    fi
done
rm -rf "$tree"
