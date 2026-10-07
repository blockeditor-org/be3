#!/bin/sh
# The native smoke check: the app in a virtual display with its own data
# directory, loading the sysroot's libraries, signed in with a workspace open.
# It must run for ten seconds, then exit cleanly when its window is closed.
# Needs xvfb-run, xdotool and python3.
#
#   ./scripts/buck run //crates/block-app:smoke
set -u
app="$1"
libraries="$2"
close="$3"
for command in xvfb-run xdotool python3; do
    command -v "$command" > /dev/null || { echo "Missing $command: run ./scripts/setup." >&2; exit 1; }
done
data="$(mktemp -d)"
trap 'rm -rf "$data"' EXIT
XDG_DATA_HOME="$data" xvfb-run -a sh -c '
    LD_LIBRARY_PATH="$2" "$1" --dev-workspace &
    pid=$!
    window="$(timeout 60 xdotool search --sync --onlyvisible --pid "$pid" 2> /dev/null | head -n 1)"
    if [ -z "$window" ]; then
        echo "Native smoke check failed: the app did not open a window." >&2
        kill "$pid" 2> /dev/null
        wait "$pid"
        exit 1
    fi
    sleep 10
    if ! kill -0 "$pid" 2> /dev/null; then
        wait "$pid"
        echo "Native smoke check failed: the app exited before ten seconds, with status $?." >&2
        exit 1
    fi
    python3 "$3" "$window"
    (sleep 30 && kill -9 "$pid" 2> /dev/null) &
    watchdog=$!
    wait "$pid"
    status=$?
    kill "$watchdog" 2> /dev/null
    case "$status" in
        0) echo "Native smoke check passed." ;;
        137) echo "Native smoke check failed: the app did not exit within 30 seconds of its window closing." >&2; exit 1 ;;
        *) echo "Native smoke check failed: the app exited with status $status when its window closed." >&2; exit 1 ;;
    esac
' sh "$app" "$libraries" "$close"
