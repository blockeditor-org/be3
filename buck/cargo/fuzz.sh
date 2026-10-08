#!/bin/sh
# Runs fuzz targets (cargo_fuzz in defs.bzl) from the root of the checkout,
# from a list of "id<TAB>binary" lines: one target's own, or every target's for
# //:fuzz. They fuzz until they are stopped, taking turns with every core: each
# fuzzes for BE3_FUZZ_SLICE seconds (600 unless it says) and then the next,
# round and round, and a target alone fuzzes without a turn ending. Each keeps
# its corpus and the inputs that crash in target/fuzz/ID (BE3_FUZZ_DIR moves
# target/fuzz), so a turn and a restart pick up where the last left off, and
# carries on past a crash, so that they can be left running. Arguments are
# libFuzzer's, given to every target, and a file among them is an input for one
# target to run once instead, such as a crash to reproduce:
#
#   ./scripts/buck run //:fuzz
#   ./scripts/buck run //crates/sequence:fuzz-sequence -- -fork=4
#   ./scripts/buck run //crates/sequence:fuzz-sequence -- target/fuzz/sequence/sequence/crashes/crash-…
set -eu
list="$1"
shift
count="$(grep -c '' "$list")"
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
jobs="$(getconf _NPROCESSORS_ONLN)"
turn=""
if [ "$count" -gt 1 ]; then
    turn="-max_total_time=${BE3_FUZZ_SLICE:-600}"
fi
pid=""
trap '[ -z "$pid" ] || kill "$pid" 2> /dev/null; exit 130' INT TERM
while :; do
    line=0
    while [ "$line" -lt "$count" ]; do
        line=$((line + 1))
        id="$(sed -n "${line}p" "$list" | cut -f1)"
        fuzzer="$(sed -n "${line}p" "$list" | cut -f2)"
        dir="${BE3_FUZZ_DIR:-target/fuzz}/$id"
        mkdir -p "$dir/corpus" "$dir/crashes"
        echo "Fuzzing $id." >&2
        "$fuzzer" \
            -fork="$jobs" \
            -ignore_crashes=1 \
            -artifact_prefix="$dir/crashes/" \
            ${turn:+"$turn"} \
            "$@" \
            "$dir/corpus" < /dev/null &
        pid=$!
        wait "$pid" || true
        pid=""
    done
    [ -n "$turn" ] || break
done
