#!/bin/sh
# Runs fuzz targets (cargo_fuzz in defs.bzl) from the root of the checkout,
# from a list of "id<TAB>binary" lines: one target's own, or every target's for
# //:fuzz. They fuzz until they are stopped, side by side with the cores shared
# between them, each keeping its corpus and the inputs that crash in
# target/fuzz/ID (BE3_FUZZ_DIR moves target/fuzz) and carrying on past a
# crash, so that they can be left running; starting them again picks up each
# corpus where it was. Arguments are libFuzzer's, given to every target, and a
# file among them is an input for one target to run once instead, such as a
# crash to reproduce:
#
#   ./scripts/buck run //:fuzz
#   ./scripts/buck run //crates/sequence:fuzz-sequence -- -fork=4
#   ./scripts/buck run //crates/sequence:fuzz-sequence -- target/fuzz/sequence/sequence/crashes/crash-…
set -eu
list="$1"
shift
count="$(grep -c '' "$list")"
tab="$(printf '\t')"
for argument in "$@"; do
    case "$argument" in
        -*) ;;
        *)
            if [ "$count" -ne 1 ]; then
                echo "$argument is an input to run once, which takes one fuzz target, such as ./scripts/buck run //crates/sequence:fuzz-sequence." >&2
                exit 1
            fi
            exec "$(cut -f2 "$list")" "$@"
            ;;
    esac
done
if [ "$count" -eq 0 ]; then
    echo "There are no fuzz targets." >&2
    exit 1
fi
jobs=$(($(getconf _NPROCESSORS_ONLN) / count))
[ "$jobs" -ge 1 ] || jobs=1
trap 'kill 0' INT TERM
while IFS="$tab" read -r id fuzzer || [ -n "$id" ]; do
    dir="${BE3_FUZZ_DIR:-target/fuzz}/$id"
    mkdir -p "$dir/corpus" "$dir/crashes"
    "$fuzzer" \
        -fork="$jobs" \
        -ignore_crashes=1 \
        -artifact_prefix="$dir/crashes/" \
        "$@" \
        "$dir/corpus" < /dev/null 2>&1 | sed -u "s|^|$id: |" &
done < "$list"
wait
