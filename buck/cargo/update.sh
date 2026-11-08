#!/bin/sh
#
# Brings buck/cargo/crates.bzl and Cargo.lock up to date with the Cargo.toml
# files, as ./scripts/verify does: run it after changing a manifest to build
# with the change before then. The bootstrap's lock step brings Cargo.lock and
# crates.bzl's downloads up to date first, with the network
# (crates/be-tool-bootstrap/src/lock.rs), then buckify.bxl generates the rest
# of crates.bzl on a worker from cargo's own plans, and this copies it into the
# checkout. With --check it writes nothing and fails if either file is out of
# date.
#
# Usage: ./scripts/buck run //:buckify [-- --check]
set -eu
check=false
[ "${1:-}" = --check ] && check=true
if $check; then
    lock='lock --check'
else
    lock='lock'
fi
# shellcheck disable=SC2086 # lock is the command and its flag
if ! sh scripts/internal/bootstrap.sh $lock; then
    echo "The lock step failed (a bootstrap pinned before it has none: run this with BE3_BOOTSTRAP=cargo)."
    exit 1
fi
output="$(./scripts/buck bxl //buck/cargo/buckify.bxl:main)"
generated="$(printf '%s\n' "$output" | tail -n 1)"
if [ ! -d "$generated" ]; then
    echo "buckify.bxl did not print the directory it generated into."
    exit 1
fi
if [ -f "$generated/stale" ]; then
    cat "$generated/stale"
    echo "The lock step left Cargo.lock needing crates buck/cargo/crates.bzl does not download."
    exit 1
fi
stale=''
for pair in "crates.bzl buck/cargo/crates.bzl" "Cargo.lock Cargo.lock"; do
    from="$generated/${pair% *}" to="${pair#* }"
    cmp -s "$from" "$to" && continue
    if $check; then
        stale="$stale $to"
        diff -u "$to" "$from" | head -n 40
    else
        cp "$from" "$to.partial"
        mv -f "$to.partial" "$to"
        echo "Updated $to."
    fi
done
if [ -n "$stale" ]; then
    echo "Out of date with the Cargo.toml files:$stale; run ./scripts/buck run //:buckify, or ./scripts/verify without --check."
    exit 1
fi
