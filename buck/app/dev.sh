#!/bin/sh
# Starts the app in a virtual display for an agent to drive, signed in to a
# local account with a workspace open, and returns once its window is up.
# guides/running_the_app.md says how to drive it.
#
#   ./scripts/buck run //crates/block-app:dev              start, or restart
#   ./scripts/buck run //crates/block-app:dev -- --fresh   start with no data
#   ./scripts/buck run //crates/block-app:dev -- --stop    stop it
set -u
app="$1"
LD_LIBRARY_PATH="$2"
export LD_LIBRARY_PATH
shift 2
fresh=false
stop=false
for argument in "$@"; do
    case "$argument" in
        --fresh) fresh=true ;;
        --stop) stop=true ;;
        *) echo "Unknown argument $argument; expected --fresh or --stop." >&2; exit 1 ;;
    esac
done

dir="${BLOCK_DEV_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/be3/dev}"
display="${BLOCK_DEV_DISPLAY:-:99}"
mkdir -p "$dir"

stop_pid() {
    [ -f "$dir/$1.pid" ] || return 0
    pid="$(cat "$dir/$1.pid")"
    if kill "$pid" 2> /dev/null; then
        while kill -0 "$pid" 2> /dev/null; do sleep 0.1; done
    fi
    rm -f "$dir/$1.pid"
}

stop_pid app
if $stop; then
    stop_pid xvfb
    echo 'Stopped the app and its display.'
    exit 0
fi

for command in Xvfb xdotool import; do
    command -v "$command" > /dev/null || { echo "Missing $command: run ./scripts/setup." >&2; exit 1; }
done

if $fresh; then
    rm -rf "$dir/data"
fi

if ! [ -f "$dir/xvfb.pid" ] || ! kill -0 "$(cat "$dir/xvfb.pid")" 2> /dev/null; then
    setsid Xvfb "$display" -screen 0 1280x800x24 -nolisten tcp > "$dir/xvfb.log" 2>&1 < /dev/null &
    echo $! > "$dir/xvfb.pid"
fi
export DISPLAY="$display"
if ! timeout 10 sh -c 'until xdotool getmouselocation > /dev/null 2>&1; do sleep 0.1; done'; then
    echo "The virtual display $display did not start; see $dir/xvfb.log." >&2
    exit 1
fi

tree="$dir/accessibility.txt"
rm -f "$tree"
XDG_DATA_HOME="$dir/data" setsid "$app" --dev-workspace "--accessibility-tree=$tree" \
    > "$dir/app.log" 2>&1 < /dev/null &
pid=$!
echo "$pid" > "$dir/app.pid"

window="$(timeout 60 xdotool search --sync --onlyvisible --pid "$pid" 2> /dev/null | head -n 1)"
if [ -z "$window" ]; then
    echo "The app did not open a window; see $dir/app.log." >&2
    exit 1
fi
# With no window manager nothing gives the window focus, and without it the
# keys xdotool sends are dropped.
xdotool windowfocus --sync "$window"
if ! timeout 60 sh -c "until grep -q '^Window' '$tree' 2> /dev/null || ! kill -0 $pid 2> /dev/null; do sleep 0.1; done"; then
    echo "The app did not write its accessibility tree; see $dir/app.log." >&2
    exit 1
fi
if ! kill -0 "$pid" 2> /dev/null; then
    echo "The app exited; see $dir/app.log." >&2
    exit 1
fi

cat > "$dir/env" <<ENV
export DISPLAY=$display
export WINDOW=$window
export TREE=$tree
ENV
cat <<INFO
The app is running in $display as window $window (pid $pid); guides/running_the_app.md
says how to drive it.
  source $dir/env    sets DISPLAY, WINDOW and TREE
  log:  $dir/app.log
INFO
