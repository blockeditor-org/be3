#!/bin/sh
# A coverage-guided fuzz target (cargo_fuzz in defs.bzl), run from the root of
# the checkout. With no input files it fuzzes until it is stopped, with a job
# for every core, keeping its corpus and the inputs that crash in
# target/fuzz/NAME (BE3_FUZZ_DIR moves target/fuzz), and carries on past a
# crash so that it can be left running; stopping and starting it again picks
# up the corpus where it was. Arguments are libFuzzer's, and a file among them
# is an input to run once instead, such as a crash to reproduce:
#
#   ./scripts/buck run //crates/sequence:fuzz
#   ./scripts/buck run //crates/sequence:fuzz -- -fork=4
#   ./scripts/buck run //crates/sequence:fuzz -- target/fuzz/sequence/crashes/crash-…
set -eu
fuzzer="$1"
name="$2"
shift 2
for argument in "$@"; do
    case "$argument" in
        -*) ;;
        *) exec "$fuzzer" "$@" ;;
    esac
done
dir="${BE3_FUZZ_DIR:-target/fuzz}/$name"
mkdir -p "$dir/corpus" "$dir/crashes"
exec "$fuzzer" \
    -fork="$(getconf _NPROCESSORS_ONLN)" \
    -ignore_crashes=1 \
    -artifact_prefix="$dir/crashes/" \
    "$@" \
    "$dir/corpus"
