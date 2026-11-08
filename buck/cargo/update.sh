#!/bin/sh
#
# Brings buck/cargo/crates.bzl and Cargo.lock up to date with the Cargo.toml
# files: buckify.bxl generates both on a worker from cargo's own plans, and
# this copies them into the checkout. ./scripts/verify does the same; run it yourself after
# changing a manifest to build with the change before then. When the action
# says Cargo.lock needs crates crates.bzl does not download yet, this brings
# the lockfile up to date and pins them with the network (buck-tools lock), and
# generates again. With --check it writes nothing and fails if either file is
# out of date.
#
# Usage: ./scripts/buck run //:buckify [-- --check]
#   (BUCK.v2 passes this buck-tools and the cargo to lock with first)
set -eu
tools="$1"
cargo="$2"
shift 2
check=false
[ "${1:-}" = --check ] && check=true
generate() {
    output="$(./scripts/buck bxl //buck/cargo/buckify.bxl:main)"
    generated="$(printf '%s\n' "$output" | tail -n 1)"
    if [ ! -d "$generated" ]; then
        echo "buckify.bxl did not print the directory it generated into."
        exit 1
    fi
}
generate
if [ -f "$generated/stale" ]; then
    cat "$generated/stale"
    if $check; then
        echo "Out of date with the Cargo.toml files: Cargo.lock, and updating it needs the network; run ./scripts/buck run //:buckify."
        exit 1
    fi
    "$tools" lock "$cargo"
    generate
    if [ -f "$generated/stale" ]; then
        cat "$generated/stale"
        echo "buck-tools lock left Cargo.lock needing crates buck/cargo/crates.bzl does not download."
        exit 1
    fi
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
