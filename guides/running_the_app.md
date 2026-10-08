# Running the app

To see a change working in the real app, start it in a virtual display and drive it with
xdotool:

    ./scripts/buck run //crates/block-app:dev
    source ~/.cache/be3/dev/env

The command builds the app with every plugin, starts Xvfb on `:99` if it is not already
running, starts the app in it and returns once its window is up. The app is signed in to an
account on its embedded server with a workspace called Dev open, so there is no account or
workspace to make first. Its data lives in `~/.cache/be3/dev/data` and survives restarts,
so running the command again restarts the app on the same workspace, which is how to pick
up a rebuild. `-- --fresh` deletes the data first, and `-- --stop` stops the app and Xvfb.
`BLOCK_DEV_DIR` and `BLOCK_DEV_DISPLAY` move the directory and the display, for running two
at once.

`env` exports `DISPLAY`, `WINDOW` (the app's X window id) and `TREE`. The app's output goes
to `~/.cache/be3/dev/app.log`.

What the launcher passes the app is available to any native run:
- `--dev-workspace`: sign in to the first local account, registering `dev@localhost` with
  the password `dev-password` if there is none, save a recovery phrase without asking, and
  open the last workspace, or the first one, or a new one called Dev.
- `--accessibility-tree=PATH`: write the accessibility tree to PATH (see below).
- `--session` (Linux): run on the displays and input devices themselves through
  `beui-adapter-drm` instead of in a window, from a virtual terminal with no other display
  server on it, as the desktop: the shell is linux-desktop instead of workspace-ui, on a
  profile of its own. Ctrl+Alt+Backspace quits and Ctrl+Alt+F<n> switches terminals.
  `BEUI_SCALE` sets its scale.

## Wayland programs

On Linux the app is a Wayland compositor. Its socket is `wayland-<n>` in `XDG_RUNTIME_DIR`,
or `/tmp/be-wayland-<pid>` when that is unset, as it is under `:dev`. The app menu's "Run a
program" starts a command with `WAYLAND_DISPLAY` pointing at it, and a program can also be
started against it directly:

    WAYLAND_DISPLAY=/tmp/be-wayland-<the app's pid> foot

Each window opens as a tab of the workspace. `weston-simple-shm` (from the `weston` package)
and `foot` are small clients to try it with. `foot --fullscreen` starts one fullscreen, and Super+F toggles fullscreen on the focused
window.

## Seeing what is on screen

`cat $TREE` is the accessibility tree as text, rewritten whenever it changes, one node per
line, indented under its parent:

    Dialog "Invite member" at 360,180 size 380x300
      TextInput "Email address" at 380,260 size 340x36
      Button at 380,420 size 116x36
        Label value="Send invitation" at 392,430 size 92x15

Coordinates are pixels relative to the window, the same ones xdotool takes, so the centre of
a node is `x + width / 2, y + height / 2`. `offscreen` marks a node scrolled out of view, and
`focused` the node with keyboard focus.

The tree holds only what the host draws itself: the account and workspace pages, the app
menu, the debugging panels and host dialogs such as Invite member. Everything a plugin draws,
which includes the workspace's dock and file tree and every block's editor, is a texture to
the host and does not appear. For those, take a screenshot and read it:

    import -window $WINDOW shot.png

`-crop WxH+X+Y` after the window id keeps a region, which is cheaper to read than the whole
window.

## Input

    xdotool mousemove --window $WINDOW 455 220 click 1
    xdotool type --window $WINDOW 'hello'
    xdotool key --window $WINDOW ctrl+z

Always pass `--window $WINDOW`: without it, keys go to whichever window has focus, which is
not always the app's. There is no window manager, so the launcher gives the window focus
once; if keys stop arriving, `xdotool windowfocus --sync $WINDOW` gives it back. Click a
text field before typing into it. Other buttons are `click 3` (right) and `click 4`/`click 5`
(scroll up and down); `mousedown 1`, `mousemove`, `mouseup 1` drag.

The app draws when something changes, so give it a moment after an input before reading the
tree or taking a screenshot.

The web build has a launcher of its own: guides/running_the_web_app.md.
