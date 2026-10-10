# Running the app

To see a change working in the real app, start it headless and drive it with `drive`:

    ./scripts/buck run //crates/block-app:dev
    source ~/.cache/be3/dev/env

The command builds the app with every plugin, starts it with no window and returns once it
has settled: signed in to an account on its embedded server, with a workspace called Dev open,
so there is no account or workspace to make first. Its data lives in `~/.cache/be3/dev/data`
and survives restarts, so running the command again restarts the app on the same workspace,
which is how to pick up a rebuild. `-- --fresh` deletes the data first, `-- --stop` stops the
app, `-- --size=WIDTHxHEIGHT[@SCALE]` sets the screen (1100x720 by default) and `-- --desktop`
runs the desktop shell instead of the workspace (see `--desktop` below); they combine.
`BLOCK_DEV_DIR` moves the directory, for running two at once. The launcher is `be-drive
launch` (crates/be-drive); the app's output goes to `~/.cache/be3/dev/app.log`.

`env` puts `drive` on `PATH`, pointed at the app's automation socket. `drive help` lists
the commands; `drive tree` prints what is on screen and `drive click '"Create"'` clicks it.

What the launcher passes the app is available to any native run:
- `--dev-workspace`: sign in to the first local account, registering `dev@localhost` with
  the password `dev-password` if there is none, save a recovery phrase without asking, and
  open the last workspace, or the first one, or a new one called Dev.
- `--headless[=WIDTHxHEIGHT[@SCALE]]`: draw offscreen with wgpu (`beui::Headless`) instead of
  in a window, needing no display server. It has no clipboard but its own and no file dialog.
- `--automation=PATH`: answer `drive` on the Unix socket PATH (see Driving the app). It works
  with a window too, for a bug that only shows with winit: give `drive --socket=PATH` the
  same path. Screenshots then need a surface that can be copied from.
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
puts each in front of the person. The host draws these itself, so they show before any shell
plugin runs (signing in, choosing a workspace) and over either shell; they stack in the
top-right corner (`Toasts` with `edge=Edge::TopEnd`), clear of linux-desktop's bar and its
notification toasts at the bottom.

The screens turn off after the display settings' "Turn off screens after" (ten minutes by
default). `be_wayland::Compositor` counts the idle time on the frame clock from the frames that
carried input (`Document::input_frames`), held back while a window that is shown has a `zwp_idle_inhibitor_v1`, and answers
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

Switching terminals hands the seat to whatever runs there, whose DRM master reprograms the
CRTCs, connectors and planes as it likes, so nothing the adapter cached about them survives.
`recovery.rs` decides what happens: when the seat comes back every output (surface,
swapchain, framebuffers, pending flips and fence watches) is dropped while the device is still
paused, the device is reset to everything off, and outputs are built again for whatever is
connected now, with the modes the display settings chose, so each one's first frame is a full
modeset rather than a page flip. Coming back also wakes the screens and starts the keyboard's
modifiers over. A frame that cannot be shown sets its output up again once; if that fails too
the output stops drawing and the problem is reported once through `Problems`, until the seat
comes back or a display is plugged in or out.

The desktop bar's power button locks, suspends, restarts, powers off or logs out. linux-desktop
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
session bus from block-app's `src/notifications/server.rs`, asking for the name without
replacing an owner, so under another desktop that already runs a notification daemon it logs
that and serves nothing. The host keeps only what a plugin cannot do: it owns the name, reads
the hints, loads the picture (image-data, image-path, the app icon through be-wayland's
`IconThemes`, then the desktop entry's icon, scaled down to at most 128 pixels and sent as
RGBA), and answers `Notify` at once with an id it picks itself (`notifications/inbox.rs`:
`replaces_id` keeps its id while the desktop still holds it). Each `Notify` and
`CloseNotification` then waits in the `Notifications` host value, a `NotificationInbox` of
numbered requests, until the shell's `NotificationReport` says it took them; the report also
carries the `NotificationClosed` and `ActionInvoked` signals the host sends and the ids the
desktop still holds.

Everything else is linux-desktop's (`src/app/notifications/`): `center.rs` is what happens to
a notification, and is where to test it: replacement in place, the toast's timeout (-1 is
five seconds, 0 and critical urgency never), actions, `resident` and `transient`, the cap of
100 kept, and the reason every close carries; `markup.rs` turns the body into plain text. A
toast whose time is up only hides: the notification stays until it is dismissed, an action
is taken or its program closes it, which is the `persistence` capability, except a transient
one, which closes as expired. The toasts are a `beui::styled::Toasts` (title, picture and
action buttons; clicking the body is the `default` action) anchored to the first screen with
the desktop bar cut off it, with ids from `TOAST_IDS` up so other toasts can share the stack.
They make way while the bell's list of notifications is open.

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

## Media and hardware keys

linux-desktop binds the volume, mute, mic mute, brightness and media transport keys
(`BINDINGS` in its `app/media.rs`, the one place that says which key does what) as
intercepting actions, so the host hands them to the shell even while a program has the keyboard,
and a held key repeats. Each key sends a `MediaRequest` (`LinuxMessage::RequestMedia`), and
a volume, mute or brightness key shows `beui::styled::LevelOsd`, an icon and a level bar on
every monitor that fades out after 1.5 seconds, with the levels the host reports (`editor.media()`, from
`LinuxMessage::Media`).

block-app does what a plugin cannot: `src/media.rs` starts its backends once a plugin watches
the levels or the shell asks for something, carries out the shell's requests only, and reports
the levels back. The backends run as tasks on the D-Bus thread:
- `media/audio.rs`: volume steps up to 100%, set volume, mute and mic mute on the default sink
  and source, over the PulseAudio protocol (the `pulseaudio` crate's protocol layer, pure Rust),
  which pipewire-pulse serves on `$XDG_RUNTIME_DIR/pulse/native`. It subscribes to the server's
  sink, source and server events and reports every change.
- `media/backlight.rs`: brightness steps through logind's `Session.SetBrightness` on
  `/sys/class/backlight`'s device (firmware before platform before raw).
- `media/players.rs`: play/pause, next, previous and stop to the MPRIS player that most
  recently started playing, or the first one listed, on the session bus.

To try them under `:dev -- --desktop`, run `pipewire`, `wireplumber` and `pipewire-pulse`
with `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` set for the app, and press them with
`drive key volumeup` (or `micmute`, `play`, ...).

## The lock screen

In the desktop shell (`--session` and `--desktop`) the screen locks on Super+L, the bar's power
menu's Lock, idling for the display settings' "Lock the screen after" (by default when the screens
turn off), and, under `--session`, logind's `Lock` for the app's own session (`loginctl
lock-session`) and before every suspend: `DesktopSession` holds a logind `delay` inhibitor for
sleep and lets it go once the lock screen has been drawn twice (`src/session/sleep.rs`), taking it
again on waking. `src/session/lock.rs` is the state machine, `src/session.rs`'s `ScreenLock` runs
it, and `src/ui/lock.rs` shows it as `styled::LockScreen`, a card on every screen of
the document (`use_screens()`) over an opaque scrim. logind's `Unlock` is not obeyed: only a password unlocks.

The password is checked by PAM on a thread of its own, with the service `block-app` when
`/etc/pam.d/block-app` exists (`--install-session` writes it, as `auth include login`, unless one is
there) and `login` otherwise. libpam is opened with `dlopen` when a password is checked, so the app
does not link it. After three wrong passwords each attempt waits, from five seconds doubling up to
a minute. Anything that goes wrong - no libpam, a PAM error, a check that panics, an answer to an
earlier attempt - leaves the screen locked; only `Verdict::Accepted` for the attempt in flight
unlocks it. Under `:dev` the app runs as the VM's user, so that user needs a password
(`passwd`) for the lock to be tried.

While it is locked, two things keep input from everything behind it: the lock is a beui
`Overlay` with `locks` set, which nothing dismisses, which stays above every other overlay and keeps
the focus and the pointer, and while it is open `Document::key_global` hears only media keys (no other global action, no plugin's
intercepted chord, no Alt+Tab); and the app menu is closed. Windows need nothing
of their own for it: each is a forwarding catcher like a plugin's region, so with the focus and the
pointer held by the lock none is forwarded anything. `be_wayland::Compositor::set_locked` only lets
go of the buttons and popups a window held. Screens still
turn off while locked, and idle inhibitors from the windows behind it are ignored. Media keys
still reach linux-desktop's intercepting media actions (through `compositor/intercept.rs`'s
`on_global_key`), so volume and playback work over the lock. The host publishes the
`ScreenLocked` host value, and linux-desktop shows no notification toasts while it is set, so
none can be read or acted on behind the lock; they show again once it is unlocked.
`ext-session-lock-v1` (external lockers such as swaylock) is not implemented.

## Wayland programs

On Linux the app is a Wayland compositor. Its socket is `wayland-<n>` in `XDG_RUNTIME_DIR`,
or `/tmp/be-wayland-<pid>` when that is unset, as it is under `:dev`. In the desktop shell
the program launcher, which tapping Super alone or the bar's programs button opens, lists the
`.desktop` programs of the XDG data dirs and starts the one picked, or what was typed as a
command, with `WAYLAND_DISPLAY` pointing at it. linux-desktop draws it; block-app only reads the
`.desktop` files and their icons and starts the programs (the `Programs` host value and
`ProgramAction`, guides/adding_a_plugin_editor.md). The workspace shell has no launcher. A
program can also be started against the socket directly:

    WAYLAND_DISPLAY=/tmp/be-wayland-<the app's pid> foot

Each window opens as a tab of the workspace. `weston-simple-shm` (from the `weston` package)
and `foot` are small clients to try it with. `foot --fullscreen` starts one fullscreen, and in the desktop shell Super+F toggles fullscreen on the focused
window (linux-desktop's `desktop.fullscreen` action, which sends `WindowAction::Fullscreen`). Under `--session` and `--desktop`, Super+drag anywhere on a floating window, its bar
included, moves the window, and Super+right-drag resizes it from its nearest edge or corner,
or moves the split beside a docked one. In the desktop shell Alt+Tab, with Alt held, opens the window switcher and walks the
windows from the one used last (Alt+Shift+Tab the other way); letting go of Alt focuses and
raises the chosen one and Alt+Escape stays where it was. To drive it, hold Alt across the
presses: `drive hold alt; drive key tab tab; drive release alt`.

Each window is a `be_wayland::WindowView`, a forwarding catcher like a plugin's region (see
guides/beui_keyboard.md, One input path): beui decides which window a press, motion, scroll or key
goes to, and `Compositor::after` turns what each window was forwarded into `wl_pointer` and
`wl_keyboard` events, keys by their scan codes (`Event::PhysicalKey`). Where the shell draws over a
window (a floating window's tab, say) the window takes nothing (`occluders`, from the region's
occlusion), and a press keeps going to the window or plugin it began on until it is let go.

The compositor pings a window's client when it is clicked, typed into, focused or asked to
close, and a client that has not answered within `PING_TIMEOUT` (be-wayland's `state.rs`) is listed as
not responding: its window is dimmed and its tab offers Wait and Force close. Closing a window
that is not responding forces it: the compositor kills the process if it launched it (or one
of its descendants) and disconnects the client either way. `kill -STOP` on a client's pid
freezes it to try this, and `kill -CONT` brings it back.

## Driving the app

`drive COMMAND` sends one command to the app and prints its answer; an error exits nonzero.
beui runs the commands (`beui_core::app::automation`), so they work the same in any beui app
started with an automation inbox, and the web build answers the same ones
(guides/running_the_web_app.md).

`drive tree` is the accessibility tree as text, one node per line, indented under its parent:

    Dialog "Invite member" at 360,180 size 380x300
      TextInput "Email address" at 380,260 size 340x36
      Button at 380,420 size 116x36
        Label value="Send invitation" at 392,430 size 92x15
    Pane "Checklist" at 250,10 size 840x700
      CheckBox "Buy milk" toggled=True at 270,60 size 400x24

Coordinates are pixels from the window's top left corner. `offscreen` marks a node scrolled
or clipped out of view, and `focused` the host's node with keyboard focus.

The tree holds what plugins draw too. Each plugin region the host shows is a `Pane` named
after its plugin, holding the nodes its editor describes: the host sends `Message::Describe`
while a driver is attached, and each `FrameReport` then carries a `Description`, the editor's
accessibility nodes and test ids in region coordinates, which `plugin_host/description.rs`
places in the host's tree. `drive ids` lists every test id on screen, the host's and the
plugins', which are the steadiest way to name a control (guides/testing_a_gui.md).

Commands that act take a TARGET:
- `#TEST_ID`, such as `#checklist.add`;
- `X,Y` in the tree's pixels;
- text found in exactly one line of the tree, such as `'"Send invitation"'` or
  `'Button "Create"'`; when several lines match, the error lists them.

    drive click '#checklist.add'          click (`right`, `middle` or `double` after it)
    drive type 'buy milk'                 type into what has focus (a newline presses Enter)
    drive key ctrl+z                      chords, such as Enter, shift+Tab or ctrl+shift+z
    drive hold alt; drive key tab tab; drive release alt
    drive drag '#card.1' '#column.done'   drag with the left button
    drive scroll '"Files"' 300            scroll down 300 pixels at a target
    drive hover '"Settings"'
    drive shot shot.png ['"Add block"']   a PNG of the window, or of one target

Every command that gives input waits until the app has settled, then prints what changed in
the tree (`+` and `-` lines), so there is no need to wait or to read the tree again. The app
has settled once a frame asks for no other and nothing is in flight: no plugin is starting, owes
a frame or holds unanswered input, and no account or workspace request is pending
(`App::busy`). Work the app cannot see, such as a server push, is waited for with `drive wait
TEXT` or `drive gone TEXT`, which return once a line of the tree contains TEXT or none does.
`drive settle` waits on its own. Each command gives up after 30 seconds, or `--timeout=SECONDS`.

Click a text field before typing into it. `type` sends text, which a Wayland window does not
take; `key` sends each key's scan code too, which it does.

The web build has a launcher of its own: guides/running_the_web_app.md.
