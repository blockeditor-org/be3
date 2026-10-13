#!/bin/sh
# The native smoke test: the app with every plugin, headless with fresh data,
# driven by `be-drive run` (guides/running/drive.md). It opens the dev
# workspace, waits until the tree shows WAIT, closes the window as its close
# button would, and fails unless the app exits cleanly. Its LD_LIBRARY_PATH is
# an argument, so that it survives a worker that sets its own.
#
#   ./scripts/buck test //crates/block-app:smoke
#   ./scripts/buck test //crates/block-app:smoke-desktop    the same with --desktop
#
# Usage: smoke.sh APP_DIR EXECUTABLE BE_DRIVE LIBRARIES WAIT [APP ARGUMENTS...]
set -u
app="$1/$2"
drive="$3"
libraries="$4"
wait_for="$5"
shift 5
dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT
BE_DRIVE_DIR="$dir" "$drive" run --program="$app" --data --libraries="$libraries" --dev-workspace "$@" <<EOF
wait '$wait_for'
close
EOF
