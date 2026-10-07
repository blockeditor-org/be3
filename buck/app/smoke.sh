#!/bin/sh
# The native smoke test: the app with every plugin and its own data directory,
# with no display, loading the libraries the third argument names. It opens
# the dev workspace, closes once every plugin editor it shows has drawn, and
# must exit cleanly.
#
#   ./scripts/buck test //crates/block-app:smoke
set -u
app="$1/$2"
LD_LIBRARY_PATH="$3"
export LD_LIBRARY_PATH
unset DISPLAY WAYLAND_DISPLAY
data="$(mktemp -d)"
trap 'rm -rf "$data"' EXIT
XDG_DATA_HOME="$data" timeout --kill-after=5s 120s "$app" --headless --dev-workspace --close-when-ready
status=$?
case "$status" in
    0) echo 'Native smoke test passed.' ;;
    124) echo 'Native smoke test failed: the app did not draw its workspace and close within two minutes.' >&2; exit 1 ;;
    *) echo "Native smoke test failed: the app exited with status $status." >&2; exit 1 ;;
esac
