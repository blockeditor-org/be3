#!/bin/sh
#
# Resolves what buck/sysroot/BUCK asks for into buck/sysroot/packages.bzl, the
# lockfile of every Ubuntu package the sysroot is made of, on a worker. Run it
# after changing that list; CI fails if the lockfile disagrees with it.
#
# Usage: ./scripts/buck run //:lock-sysroot
set -eu
lock="$(./scripts/buck build //buck/sysroot:lock --show-full-output | awk '$1 == "root//buck/sysroot:lock" { print $2 }')"
cp "$lock" buck/sysroot/packages.bzl
