#!/bin/sh
#
# Brings buck/cargo/crates.bzl and Cargo.lock up to date with the Cargo.toml
# files: buckify.bxl generates both on a worker from cargo's own plans, and
# this copies them into the checkout. //:verify runs it; run it yourself after
# changing a manifest to build with the change before then. With --check it
# writes nothing and fails if either file is out of date.
#
# Usage: ./scripts/buck run //:buckify [-- --check]
set -eu
check=false
[ "${1:-}" = --check ] && check=true
generated="$(./scripts/buck bxl //buck/cargo/buckify.bxl:main | tail -n 1)"
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
    echo "Out of date with the Cargo.toml files:$stale; run ./scripts/buck run //:buckify, or //:verify without --check."
    exit 1
fi
