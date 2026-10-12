# Running the app natively

To see a change working, run the app headless and drive it: guides/running/drive.md. This
guide is for what that does not cover.

## block-app's flags

- `--dev-workspace`: sign in to the first local account, registering `dev@localhost` with
  the password `dev-password` if there is none, save a recovery phrase without asking, and
  open the last workspace, or the first one, or a new one called Dev. `:dev` passes it.
- `--desktop` and `--session` (Linux): the desktop shell, in a window or on the displays
  themselves (guides/linux_desktop.md). `./scripts/buck run //crates/block-app:dev -- --desktop`
  drives the desktop shell headless.
- `--install-session [PREFIX]` (Linux): install the app as a login session
  (guides/linux_desktop.md).

`./scripts/buck run //crates/block-app:app` runs it in a window, with every plugin beside it.
`./scripts/buck run //crates/block-app:smoke` (and `:smoke-desktop`) is the bounded launch
check for changes that could affect native startup: the app for ten seconds in a virtual
display.

## In a window

A change to the platform layer - beui-adapter-winit's translation of keys, the pointer and
the clipboard, the native file dialog, the window's surface, IME, key repeat, a crash only a
real window shows - needs the app in a real window, which in a container means a virtual X
display driven with xdotool. Install them first (`sudo apt-get install xvfb xdotool imagemagick`):

    Xvfb :99 -screen 0 1280x800x24 -nolisten tcp &
    export DISPLAY=:99
    BEUI_AUTOMATION=/tmp/block.sock XDG_DATA_HOME=/tmp/block-data \
        ./scripts/buck run //crates/block-app:app -- --dev-workspace > /tmp/block.log 2>&1 &
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
`BEUI_AUTOMATION` set `drive --socket=/tmp/block.sock tree` reads the tree and
`drive --socket=/tmp/block.sock settle` waits for the app to settle in between. xdotool
returns before the app has drawn, so settle before reading anything.
