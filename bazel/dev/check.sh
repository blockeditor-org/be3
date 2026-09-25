#!/bin/sh
#
# What `./scripts/bazel run //:check` runs: rustc's check pass, through clippy,
# over every first-party target in every configuration it is built in, and the
# compiler's errors if there are any. crates/bazel-tools' clippy says which
# targets and configurations those are.
set -eu
tools="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
cd "$BUILD_WORKSPACE_DIRECTORY"
exec "$tools" clippy "$(pwd)/scripts/bazel" --no-lints
