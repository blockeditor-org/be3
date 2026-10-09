#!/bin/sh
# Starts the app in a virtual display for an agent to drive, signed in to a
# local account with a workspace open, and returns once its window is up.
# guides/running_the_app.md says how to drive it.
#
#   ./scripts/buck run //crates/block-app:dev              start, or restart
#   ./scripts/buck run //crates/block-app:dev -- --fresh   start with no data
#   ./scripts/buck run //crates/block-app:dev -- --stop    stop it
#   ./scripts/buck run //crates/block-app:dev -- --desktop the desktop shell
set -u
app="$1"
# Only the app loads the sysroot's libraries. Xvfb and xdotool are this
# machine's, and Xvfb outlives the build that made this path.
libraries="$2"
shift 2
fresh=false
stop=false
desktop=
for argument in "$@"; do
    case "$argument" in
        --fresh) fresh=true ;;
        --stop) stop=true ;;
        --desktop) desktop=--desktop ;;
        *) echo "Unknown argument $argument; expected --fresh, --stop or --desktop." >&2; exit 1 ;;
    esac
done

dir="${BLOCK_DEV_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/be3/dev}"
display="${BLOCK_DEV_DISPLAY:-:99}"
mkdir -p "$dir"

# A pid file outlives its process when the machine or container restarts, and
# its pid may then name another program, so a pid is only stopped while it
# still runs the program it was written for.
stop_pid() {
    [ -f "$dir/$1.pid" ] || return 0
    pid="$(cat "$dir/$1.pid")"
    if [ "$(cat "/proc/$pid/comm" 2> /dev/null)" = "$2" ] && kill "$pid" 2> /dev/null; then
        while kill -0 "$pid" 2> /dev/null; do sleep 0.1; done
    fi
    rm -f "$dir/$1.pid"
}

stop_pid app block-app
if $stop; then
    stop_pid xvfb Xvfb
    echo 'Stopped the app and its display.'
    exit 0
fi

for command in Xvfb xdotool import; do
    command -v "$command" > /dev/null || { echo "Missing $command: run ./scripts/setup." >&2; exit 1; }
done

if $fresh; then
    rm -rf "$dir/data"
fi

number="${display#:}"
number="${number%%.*}"
lock="/tmp/.X$number-lock"

answers() {
    DISPLAY="$display" timeout 5 xdotool getmouselocation > /dev/null 2>&1
}

# Whether a pid is an X server, for the same reason as stop_pid.
is_x_server() {
    case "$(cat "/proc/$1/comm" 2> /dev/null)" in
        Xvfb | Xorg | X | Xwayland) return 0 ;;
        *) return 1 ;;
    esac
}

ours=''
[ -f "$dir/xvfb.pid" ] && ours="$(cat "$dir/xvfb.pid")"
if [ -n "$ours" ] && ! { is_x_server "$ours" && answers; }; then
    is_x_server "$ours" && kill "$ours" 2> /dev/null
    rm -f "$dir/xvfb.pid"
    ours=''
fi
if [ -z "$ours" ] && ! answers; then
    if [ -f "$lock" ]; then
        owner="$(tr -d ' ' < "$lock" 2> /dev/null)"
        if [ -n "$owner" ] && is_x_server "$owner"; then
            echo "The display $display belongs to an X server (pid $owner) that does not answer; stop it, or set BLOCK_DEV_DISPLAY to another display." >&2
            exit 1
        fi
        rm -f "$lock" "/tmp/.X11-unix/X$number"
    fi
    setsid Xvfb "$display" -screen 0 1280x800x24 -nolisten tcp > "$dir/xvfb.log" 2>&1 < /dev/null &
    xvfb=$!
    echo "$xvfb" > "$dir/xvfb.pid"
    waited=0
    until answers; do
        if ! kill -0 "$xvfb" 2> /dev/null || [ "$waited" -ge 300 ]; then
            echo "The virtual display $display did not start. Xvfb said, in $dir/xvfb.log:" >&2
            grep -v -e '^> ' -e 'xkbcomp' -e '^$' "$dir/xvfb.log" | tail -n 8 >&2
            rm -f "$dir/xvfb.pid"
            exit 1
        fi
        sleep 0.1
        waited=$((waited + 1))
    done
fi
export DISPLAY="$display"

tree="$dir/accessibility.txt"
rm -f "$tree"
LD_LIBRARY_PATH="$libraries" XDG_DATA_HOME="$dir/data" setsid "$app" --dev-workspace $desktop "--accessibility-tree=$tree" \
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
