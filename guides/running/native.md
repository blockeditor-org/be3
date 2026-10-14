# Running the app natively

To see a change working, run the app headless and drive it (`guides/running/drive.md`). This
guide covers what that does not: block-app's flags, and running the app in a real window for
changes to the platform layer. Profiling is in `guides/performance.md`.

## block-app's flags

| Flag | Effect |
|---|---|
| `--dev-workspace` | signs in to the first local account, registering `dev@localhost` with the password `dev-password` if there is none, saves a recovery phrase without asking, and opens the last workspace, or the first one, or a new one called Dev; `:dev` passes it |
| `--desktop`, `--session` (Linux) | the desktop shell, in a window or on the displays themselves (`guides/linux_desktop.md`); `./scripts/buck run //crates/block-app:dev -- --desktop` drives the desktop shell headless |
| `--install-session [PREFIX]` (Linux) | installs the app as a login session (`guides/linux_desktop.md`) |

## In a real window

A change to the platform layer needs the app in a real window: beui-adapter-winit's translation
of keys, the pointer and the clipboard, the native file dialog, the window's surface, IME, key
repeat, or a crash only a real window shows. In a container that means a virtual X display
driven with xdotool.

### Starting it

1. Install the tools: `sudo apt-get install xvfb xdotool imagemagick`.
2. Start the display and the app, and find its window:

   ```
   Xvfb :99 -screen 0 1280x800x24 -nolisten tcp &
   export DISPLAY=:99
   BEUI_AUTOMATION=/tmp/block.sock XDG_DATA_HOME=/tmp/block-data \
       ./scripts/buck run //crates/block-app:native -- --dev-workspace > /tmp/block.log 2>&1 &
   WINDOW=$(xdotool search --sync --onlyvisible --name '^Block$' | head -n 1)
   xdotool windowfocus --sync $WINDOW
   ```

There is no window manager, so nothing else gives the window focus. If keys stop arriving,
`xdotool windowfocus --sync $WINDOW` gives it back.

### Driving it

Input goes through X and winit:

```
xdotool mousemove --window $WINDOW 455 220 click 1
xdotool type --window $WINDOW 'hello'
xdotool key --window $WINDOW ctrl+z
import -window $WINDOW shot.png
```

- Always pass `--window $WINDOW`, or keys go to whichever window has focus.
- `click 3` is the right button, and `click 4`/`click 5` scroll. `mousedown 1`, `mousemove` and
  `mouseup 1` drag.
- xdotool's coordinates are the window's pixels, the same as `drive tree`'s.
- With `BEUI_AUTOMATION` set, `drive --socket=/tmp/block.sock tree` reads the tree and
  `drive --socket=/tmp/block.sock settle` waits for the app to settle.
- xdotool returns before the app has drawn, so settle before reading anything.
