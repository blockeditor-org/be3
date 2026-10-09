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
`-- --desktop` runs the desktop shell instead of the workspace (see `--desktop` below); it
combines with `--fresh`.
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
  profile of its own. The bar's power menu logs out (see below), Ctrl+Alt+Backspace quits at
  once and Ctrl+Alt+F<n> switches terminals.
  `BEUI_SCALE` sets its scale.
- `--desktop` (Linux): the desktop shell, linux-desktop on its own profile as under
  `--session`, in a normal window, with programs opening as windows inside it as they do in
  any windowed run. What needs the displays and input devices themselves, such as display
  modes, pointer settings and the power menu, is absent, since `beui-adapter-drm` is not
  running: the power actions start only when the DRM seat provides its `DisplayControl`.
  `./scripts/buck run //crates/block-app:smoke-desktop` is its launch check.
- `--install-session [PREFIX]` (Linux): see below.

## The desktop session

    ./scripts/buck run //crates/block-app:install-session [-- PREFIX]

installs the app with its plugins in `PREFIX/lib/block-app` (`/usr/local` by default, asking
sudo when it needs root), links `PREFIX/bin/block-app` to it and writes
`PREFIX/share/wayland-sessions/block-app.desktop` (from `src/session/block-app.desktop`),
which runs it with `--session`. GDM and SDDM then offer Block on the login screen's session
menu, where it can be picked and remembered as the default; an SDDM older than 0.20 reads only
`/usr/share/wayland-sessions`, so give it `/usr`. Running it again replaces the install.

In session mode stdout and stderr, and so every `eprintln!`, every panic with its backtrace
and the output of every program the app starts, go to `$XDG_STATE_HOME/block/session.log`
(`~/.local/state/block/session.log`); the previous session's is kept beside it as
`session.previous.log`. Problems a person should see at once also show as toasts in the
corner of the screen: `beui_adapter_drm::Problems` carries the display and input ones,
`be_wayland::Compositor::on_failure` a program that could not be run, and `notices::report`
puts each in front of the person.

The screens turn off after the display settings' "Turn off screens after" (ten minutes by
default). `be_wayland::Compositor` counts the idle time on the frame clock from the input events
it sees, held back while a window that is shown has a `zwp_idle_inhibitor_v1`, and answers
`ext_idle_notifier_v1` for idle daemons the same way. `Windows::idle()` is true once the time
has run out; block-app hands it to `beui_adapter_drm::DisplayControl::set_blanked`, which turns
the outputs' CRTCs off and stops drawing to them until input wakes the session. In a window
nothing is turned off.

The input that wakes the screens reaches nothing: the adapter's `WakeGate` (`wake.rs`) drops it
before beui, plugins or Wayland clients see it, along with all input for a grace period after
it (`GRACE_USEC`, timed on libinput's event clock), and the release of every press it dropped,
whenever that comes. Pointer motion it drops still moves the cursor. A release whose press was
delivered before the screens went off is still delivered, so no key stays held. The adapter
turns the screens on itself and reports the wake through `DisplayControl::take_woken`, which
block-app hands to `Compositor::woke` so the idle count restarts.

The desktop bar's power button suspends, restarts, powers off or logs out. linux-desktop
asks for what it may offer with `LinuxMessage::WatchPower` and sends `RequestPower`;
`src/session/power.rs` decides what happens (programs are asked to close, and are given five
seconds before the session ends or logind is asked to restart or power off) and
`src/session/logind.rs` talks to `org.freedesktop.login1`, including its `PrepareForSleep`
signal. Logging out closes the app the way closing its window does; `beui-adapter-drm`'s
`Session::end` then drops the app, the outputs, the input devices, the DRM device (which
puts back what the console showed) and, last, the seat. Every D-Bus conversation runs on one thread, `src/dbus.rs`: `dbus::spawn` runs a
future there and `dbus::system()` is the shared system bus connection. A task hands what it
learns back through `host::waking_channel`, which wakes the event loop, so nothing on the UI
thread waits on the bus. `dbus::session()` is the shared session bus connection.

## Notifications

The desktop shell (`--session` and `--desktop`) serves `org.freedesktop.Notifications` on the
session bus from `src/notifications/server.rs`, asking for the name without replacing an owner,
so under another desktop that already runs a notification daemon it logs that and serves
nothing. The D-Bus side reads the hints and loads the picture (image-data, image-path, the
app icon through be-wayland's `IconThemes`, then the desktop entry's icon) and hands each
`Notify` to the UI thread, which answers with its id. `notifications/center.rs` holds what
happens to them, and is where to test it: ids and `replaces_id`, the toast's timeout (-1 is
five seconds, 0 and critical urgency never), actions, `resident` and `transient`, and the
reason every `NotificationClosed` carries. A toast whose time is up only hides: the
notification stays until it is dismissed, an action is taken or its program closes it, which
is the `persistence` capability, except a transient one, which closes as expired.

Each shows as one of the host's toasts (`beui::styled::Toasts`, with a title, picture and
action buttons; clicking the body is the `default` action). The desktop bar's bell lists
them, newest first: linux-desktop watches them with `LinuxMessage::WatchNotifications` and
answers with `InvokeNotification` and `DismissNotifications`.

To try it, start a session bus of its own (`dbus-daemon --session --fork --print-address`
prints its address; `notify-send` is in the `libnotify-bin` package), and run the dev app and
every client with `DBUS_SESSION_BUS_ADDRESS` set to it:

    DBUS_SESSION_BUS_ADDRESS=ADDRESS ./scripts/buck run //crates/block-app:dev -- --desktop
    DBUS_SESSION_BUS_ADDRESS=ADDRESS gdbus monitor --session -d org.freedesktop.Notifications
    DBUS_SESSION_BUS_ADDRESS=ADDRESS notify-send --action=default=Open --action=reply=Reply Mail 'Lunch at noon?'

Start the monitor (in the background, or another shell) before `notify-send`, so it sees the
signals.

`notify-send` with an action waits, and prints the action taken, until the notification
closes. `dbus-run-session` does not suit `:dev`, since its bus goes away when the launcher
returns.

## Wayland programs

On Linux the app is a Wayland compositor. Its socket is `wayland-<n>` in `XDG_RUNTIME_DIR`,
or `/tmp/be-wayland-<pid>` when that is unset, as it is under `:dev`. The launcher lists the
`.desktop` programs of the XDG data dirs and starts the one picked, or what was typed as a
command, with `WAYLAND_DISPLAY` pointing at it. Tapping Super alone opens it, as do the app
menu's "Programs" and the desktop bar's programs button; in its accessibility tree each
program is a `ListBoxOption`. A program can also be started against the socket directly:

    WAYLAND_DISPLAY=/tmp/be-wayland-<the app's pid> foot

Each window opens as a tab of the workspace. `weston-simple-shm` (from the `weston` package)
and `foot` are small clients to try it with. `foot --fullscreen` starts one fullscreen, and Super+F toggles fullscreen on the focused
window. Under `--session` and `--desktop`, Super+drag anywhere on a floating window, its bar
included, moves the window, and Super+right-drag resizes it from its nearest edge or corner,
or moves the split beside a docked one. In the desktop shell Alt+Tab, with Alt held, opens the window switcher and walks the
windows from the one used last (Alt+Shift+Tab the other way); letting go of Alt focuses and
raises the chosen one and Alt+Escape stays where it was. To drive it, hold Alt across the
presses: `xdotool keydown alt key Tab key Tab keyup alt`.

The compositor pings a window's client when it is clicked, typed into, focused or asked to
close, and a client that has not answered within `PING_TIMEOUT` (be-wayland's `state.rs`) is listed as
not responding: its window is dimmed and its tab offers Wait and Force close. Closing a window
that is not responding forces it: the compositor kills the process if it launched it (or one
of its descendants) and disconnects the client either way. `kill -STOP` on a client's pid
freezes it to try this, and `kill -CONT` brings it back.

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
