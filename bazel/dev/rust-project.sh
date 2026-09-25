#!/bin/sh
#
# What `./scripts/bazel run //:rust-project` runs: rules_rust's rust-project.json
# for the host's crates and, as wasm32-wasip1-threads, the plugins'.
set -eu
tools="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
cd "$BUILD_WORKSPACE_DIRECTORY"
exec "$tools" rust-project "$(pwd)/scripts/bazel"
