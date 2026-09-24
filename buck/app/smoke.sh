#!/bin/sh
# The native startup smoke check: starts the app for ten seconds in a virtual
# display, with a data directory of its own, and passes if it is still running
# at the end. It needs xvfb-run. The libraries it loads are the sysroot's it was
# linked against, so it needs no GTK or WebKitGTK installed here.
#
#   ./scripts/buck run //crates/block-app:smoke
set -u
app="$1"
LD_LIBRARY_PATH="$2"
export LD_LIBRARY_PATH
command -v xvfb-run > /dev/null || { echo 'Install xvfb to run the smoke check.' >&2; exit 1; }
data="$(mktemp -d)"
trap 'rm -rf "$data"' EXIT
XDG_DATA_HOME="$data" xvfb-run -a timeout --kill-after=5s 10s "$app"
status=$?
case "$status" in
    124) echo 'Native startup smoke check passed.' ;;
    0) echo 'Native startup smoke check failed: the app exited before ten seconds.' >&2; exit 1 ;;
    *) echo "Native startup smoke check failed with status $status." >&2; exit "$status" ;;
esac
