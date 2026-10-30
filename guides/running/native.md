# Running the app natively

To see a change working, run the app headless and drive it: guides/running/drive.md. This
guide is for what that does not cover.

## block-app's flags

- `--dev-workspace`: sign in to the first local account, registering `dev@localhost` with
  the password `dev-password` if there is none, save a recovery phrase without asking, and
  open the last organization, or the first one (the server's personal organization, which
  registering joins), on the account's My files workspace. `:dev` passes it.
- `--desktop` (every native platform): the desktop shell in a window; `--session` (Linux):
  on the displays themselves (guides/desktop/linux.md).
  `./scripts/buck run //crates/block-app:dev -- --desktop` drives the desktop shell headless.
- `--install-session [PREFIX]` (Linux): install the app as a login session
  (guides/desktop/linux.md).

`./scripts/buck run //crates/block-app:native` runs it in a window, with every plugin beside it.
The bundle’s `build-info.json` selects the app identity: local builds use
`BlockDev` (`~/.local/share/blockdev` on Linux), and deploy uses `Block`.
They keep separate accounts, workspaces and caches, including for local release builds.

## In a window

A change to the platform layer - beui-platform-winit's translation of keys, the pointer and
the clipboard, the native file dialog, the window's surface, IME, key repeat, a crash only a
real window shows - needs the app in a real window, which in a container means a virtual X
display driven with xdotool. Install them first (`sudo apt-get install xvfb xdotool imagemagick`):

    Xvfb :99 -screen 0 1280x800x24 -nolisten tcp &
    export DISPLAY=:99
    BEUI_LAYERS=drive BEUI_DRIVE_SOCKET=/tmp/block.sock XDG_DATA_HOME=/tmp/block-data \
        ./scripts/buck run //crates/block-app:native -- --dev-workspace > /tmp/block.log 2>&1 &
    WINDOW=$(xdotool search --sync --onlyvisible --name '^Block$' | head -n 1)
    xdotool windowfocus --sync $WINDOW

There is no window manager, so nothing else gives the window focus; if keys stop arriving,
`xdotool windowfocus --sync $WINDOW` gives it back. Then input goes through X and winit:

    xdotool mousemove --window $WINDOW 455 220 click 1
    xdotool type --window $WINDOW 'hello'
    xdotool key --window $WINDOW ctrl+z
    import -window $WINDOW shot.png

Always pass `--window $WINDOW`, or keys go to whichever window has focus. `click 3` is the
right button and `click 4`/`click 5` scroll; `mousedown 1`, `mousemove` and `mouseup 1` drag.
xdotool's coordinates are the window's pixels, the same as the tree's, and with
the drive layer stacked `drive --socket=/tmp/block.sock tree` reads the tree and
`drive --socket=/tmp/block.sock settle` waits for the app to settle in between. xdotool
returns before the app has drawn, so settle before reading anything.

A window also shows how plugins' frames arrive relative to each other, which the headless
platform hides: dragging workspace-ui's splitter between Files and an editor, say. Drag in
small steps (`mousedown 1`, then `mousemove` one step at a time) and run `import -window root`
after each step to see every frame. `WINIT_X11_SCALE_FACTOR=1.5` gives the window a fractional
scale. To skip building every plugin, run the binary that `:dev` built
(`buck-out/v2/art/root/crates/block-app/__dev-app__/*/dev-app/block-app`, with the
`LD_LIBRARY_PATH` and `XDG_DATA_HOME` that `:dev` gives it, read from `/proc/PID/environ`,
then stop that headless app) with the variables above in place of `:native`.

## Profiling

Profile a release build: `./scripts/buck run //crates/beui-demo:dev -m release` (or block-app's
`:dev`) runs it headless with debug info, ready for `drive`. The inspector's Perf tab
(guides/beui.md, The inspector) times each frame of the document: open it with
`drive key ctrl+shift+i` and `drive click '"Perf"'`, act, then read the Peak column from
`drive tree`; Reset samples starts over. Interaction covers whatever the input ran, so a
component built in response to a click is counted there, not in Layout. Take the median of
several runs: the headless renderer draws on the CPU and competes for the same cores.

For where the time goes, `sudo apt-get install linux-tools-generic` installs `perf` as
`/usr/lib/linux-tools-*/perf`. The VM has no hardware counters, so it samples the software
clock, and the binary has no frame pointers, so it unwinds with DWARF:

    perf record -F 10000 --call-graph dwarf,16384 -p "$(pgrep -x demo)" -o out.data

To record only some of the time, start it paused with `-D -1 --control=fifo:ctl,ack` and
write `enable` and `disable` to the `ctl` fifo around the drive commands to measure.
`perf report` and `perf script` need `--no-inline`, or they spend minutes resolving inlined
frames. The headless renderer's shaders show up as `[JIT]`; leave them out of a beui profile.
