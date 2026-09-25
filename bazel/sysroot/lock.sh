#!/bin/sh
#
# Resolves bazel/sysroot/wanted.bzl into bazel/sysroot/packages.bzl, the
# lockfile of every Ubuntu package the sysroots are made of, on a worker. Run it
# after changing what they want; CI fails if the lockfile disagrees with it.
#
# Usage: ./scripts/bazel run //:lock-sysroot
set -eu
lock="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
cd "$BUILD_WORKSPACE_DIRECTORY"
cp "$lock" bazel/sysroot/packages.bzl
chmod 644 bazel/sysroot/packages.bzl
