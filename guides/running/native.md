# Running be-host natively

To see a change working, run be-host headless and drive it: guides/running/drive.md. This
guide is for what that does not cover.

## be-host's flags

- `--dev-workspace`: sign in to the first local account, registering `dev@localhost` with
  the password `dev-password` if there is none, save a recovery phrase without asking, and
  open the last organization, or the first one (the server's personal organization, which
  registering joins), on the account's My files workspace. `:dev` passes it.
- `--desktop` (every native platform): the desktop shell in a window; with `BEUI_PLATFORM=drm`
  (Linux), a login session on the displays themselves (guides/desktop/linux.md). `BEUI_PLATFORM=drm`
  without `--desktop` runs the workspace shell on the displays instead.
  `./scripts/buck run //crates/be-host:dev -- --desktop` drives the desktop shell headless.
- `--install-login-session [PREFIX]` and `--uninstall-login-session [PREFIX]` (Linux): install
  be-host as a login session, or remove it again (guides/desktop/login_session.md, Installing it).

## Data directories

The bundle's `build-info.json` selects be-host's identity: local builds, release ones included,
use `be-host-dev` (`~/.local/share/be-host-dev` on Linux), and deploy uses `be-host`. They keep separate
accounts, workspaces and caches.

One be-host runs on a data directory at a time: it holds an advisory lock on `instance.lock` there
(`src/instance.rs`), which the OS drops when it exits or crashes, and listens on localhost for a
second be-host, whose port and token it writes to `instance`. A second be-host on the same data
hands over to it (the running one opens the dev workspace if `--dev-workspace` asked and raises its
window, beui's `Context::raise_window`) and exits; a second `--desktop` or login session refuses
with an error instead. To run two, give each its own `XDG_DATA_HOME`, as `drive` and the host
tests do.

## In a window

A change to the platform layer - beui-platform-winit's translation of keys, the pointer and
the clipboard, the native file dialog, the window's surface, IME, key repeat, a crash only a
real window shows - needs be-host in a real window, which in a container means a virtual X
display driven with xdotool. `./scripts/buck run //crates/be-host:native` runs it in a window,
with every app beside it. Install the tools first (`sudo apt-get install xvfb xdotool imagemagick`):

    Xvfb :99 -screen 0 1280x800x24 -nolisten tcp &
    export DISPLAY=:99
    BEUI_LAYERS=drive BEUI_DRIVE_SOCKET=/tmp/be-host.sock XDG_DATA_HOME=/tmp/be-host-data \
        ./scripts/buck run //crates/be-host:native -- --dev-workspace > /tmp/be-host.log 2>&1 &
    WINDOW=$(xdotool search --sync --onlyvisible --name '^be-host$' | head -n 1)
    xdotool windowfocus --sync $WINDOW

There is no window manager, so nothing else gives the window focus; if keys stop arriving,
`xdotool windowfocus --sync $WINDOW` gives it back. Then input goes through X and winit:

    xdotool mousemove --window $WINDOW 455 220 click 1
    xdotool type --window $WINDOW 'hello'
    xdotool key --window $WINDOW ctrl+z
    import -window $WINDOW shot.png

Without `--window $WINDOW`, keys go to whichever window has focus. `click 3` is the
right button and `click 4`/`click 5` scroll; `mousedown 1`, `mousemove` and `mouseup 1` drag.
xdotool's coordinates are the window's pixels, the same as the tree's, and with
the drive layer stacked `drive --socket=/tmp/be-host.sock tree` reads the tree and
`drive --socket=/tmp/be-host.sock settle` waits for the host to settle in between. xdotool
returns before the host has drawn, so settle before reading anything.

A window also shows how apps' frames arrive relative to each other, which the headless
platform hides: dragging workspace-ui's splitter between Files and an editor, say. Drag in
small steps (`mousedown 1`, then `mousemove` one step at a time) and run `import -window root`
after each step to see every frame. `WINIT_X11_SCALE_FACTOR=1.5` gives the window a fractional
scale.

To skip building every app, run the be-host that `:dev` unpacked
(`target/programs/dev-app-*/be-host`) in place of `:native`, with the variables above and the
`LD_LIBRARY_PATH` and `XDG_DATA_HOME` that `:dev` gives it (read them from `/proc/PID/environ`
of the headless copy, then stop that copy).

## Profiling

Profile a release build: `./scripts/buck run //crates/beui-demo:dev -m release` (or be-host's
`:dev`) runs it headless with debug info, ready for `drive`. The inspector's Perf tab
(guides/beui/programs.md, The inspector and the simulation tools) times each frame: `drive key ctrl+shift+i`,
`drive click '"Perf"'`, act, then read the Peak column from `drive tree`. Interaction covers
whatever the input ran, so a component built in response to a click counts there, not in
Layout. Take the median of several runs: the headless renderer draws on the CPU and competes
for the same cores. For perf, see guides/performance.md, Profiling.
