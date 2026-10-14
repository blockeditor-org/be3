# The Linux desktop

block-app is also a Linux desktop: a Wayland compositor whose shell is the linux-desktop plugin
instead of workspace-ui. Read this to run or change the desktop session, program windows (Wayland
and X11), notifications, the tray, media keys, the lock screen, or how the app behaves on real
displays.

The compositor is be-wayland (`crates/be-wayland`), and on Linux the app is a Wayland compositor
in every mode, the workspace shell included. The desktop shell is the linux-desktop plugin
(`crates/editors/linux-desktop`); what a plugin cannot do (D-Bus, logind, PAM, audio, reading
files) is block-app's, handed to the shell through host values and actions.

## Running it

| Command or flag | What it does |
|---|---|
| `--session` | Runs on the displays and input devices themselves through `beui-adapter-drm`, from a virtual terminal with no other display server on it, as the desktop: the shell is linux-desktop on a session of its own. `BEUI_SCALE` sets its scale. |
| `--desktop` | The desktop shell, linux-desktop on its own session as under `--session`, but in a normal window, with programs opening as windows inside it as in any windowed run. |
| `--install-session [PREFIX]` | Installs the app as a login session (see Installing it as a session). |
| `./scripts/buck run //crates/block-app:dev -- --desktop` | The desktop shell headless, to drive like any other run (guides/running/drive.md). |
| `./scripts/buck test //crates/block-app:smoke-desktop` | Starts `--desktop` headless and closes it. |

All three flags are Linux only.

Under `--session`:

- Ctrl+Alt+Backspace quits at once.
- Ctrl+Alt+F<n> switches terminals.
- The bar's power menu logs out (see Power and logind).

Under `--desktop`, what needs the displays and input devices themselves (display modes, pointer
settings, the power menu) is absent, since `beui-adapter-drm` is not running: the power actions
start only when the DRM seat provides its `DisplayControl`.

### Trying programs and services under `:dev`

Most sections below end with how to try that feature in the headless dev app. They share these
pieces:

- **The Wayland socket** is `wayland-<n>` in `XDG_RUNTIME_DIR`, or `/tmp/be-wayland-<pid>` when
  that is unset, as it is under `:dev`. Start a program against it with
  `WAYLAND_DISPLAY=/tmp/be-wayland-<the app's pid> foot`.
- **A session bus of its own.** `dbus-daemon --session --fork --print-address` prints its
  address. Run the dev app and every client with `DBUS_SESSION_BUS_ADDRESS` set to it.
  `dbus-run-session` does not suit `:dev`, since its bus goes away when the launcher returns.
- **Small clients:** `foot`, `weston-simple-shm` (from the `weston` package), `xterm`,
  `wl-copy`/`wl-paste` (`wl-clipboard`), `notify-send` (`libnotify-bin`).

## Installing it as a session

```sh
./scripts/buck run //crates/block-app:install-session [-- PREFIX]
```

- Installs the app with its plugins in `PREFIX/lib/block-app` (`/usr/local` by default, asking
  sudo when it needs root) and links `PREFIX/bin/block-app` to it.
- Writes `/usr/share/wayland-sessions/block-app.desktop` (from `src/session/block-app.desktop`),
  which runs it with `--session`, whatever the prefix. GDM, SDDM (older ones read nothing else)
  and LightDM all read that directory, and LightDM (Mint's slick-greeter) never reads
  `/usr/local/share`.
- When LightDM's `sessions-directory` (its `lightdm.conf.d` directories and
  `/etc/lightdm/lightdm.conf`) leaves that directory out, the entry also goes in the first one
  it lists. `X-LightDM-Session-Type=wayland` makes LightDM run it as Wayland from any directory.
- Writes `/etc/pam.d/block-app` for the lock screen unless one is there (see The lock screen).
- Running it again replaces the install, and removes an entry an older install left in
  `PREFIX/share/wayland-sessions`.

The login screen then offers Block on its session menu, where it can be picked and remembered as
the default.

## Logs, problems and panics

### The session log

In session mode stdout and stderr go to `$XDG_STATE_HOME/block/session.log`
(`~/.local/state/block/session.log`): every `eprintln!`, every panic with its backtrace, and the
output of every program the app starts. The previous session's is kept beside it as
`session.previous.log`.

### Problem toasts

Problems a person should see at once also show as toasts:

- `beui_adapter_drm::Problems` carries the display and input ones.
- `be_wayland::Compositor::on_failure` reports a program that could not be run.
- `notices::report` puts each in front of the person as an error toast that stays until it is
  dismissed. `notices::report_with` also keeps a full report (a crashed plugin's error, or the
  backtrace of the panic that ended the last session) behind the toast's Copy report button.

The host draws these itself, so they show before any shell plugin runs (signing in, choosing a
workspace) and over either shell. They stack in the top-right corner (`Toasts` with
`edge=Edge::TopEnd`), clear of linux-desktop's bar and its notification toasts at the bottom.

### Panics

A panic must not take every program's window with it, so block-app's `panic_guard` sorts panics
by where they happen. Every panic is logged with its backtrace.

- **Contained** (inside `panic_guard::contain`):
  - A plugin, or the host's work for one, stops only that plugin, the shell included
    (guides/plugins/architecture.md), while the compositor and its clients keep running.
  - A D-Bus task (`dbus::spawn`) ends alone with a toast.
  - A password check that panics fails.
- **The app's own** (any other panic, on the UI thread or on a thread nothing contains, such as
  the block stack's): in a window the app shows its error screen. Under `--session` it ends the
  session instead, through beui's runner and `beui_adapter_drm`'s `Session::end`, so the console
  comes back and the display manager takes over rather than a frozen screen.

## Program windows

### How a window is shown

- A program's toplevel is a `be_wayland::WindowView`, a forwarding catcher the shell places like
  any other node.
- A top-level window opens as a tab in the workspace shell and floats in a dock window of its own
  in the desktop shell.
- A window with a parent (`xdg_toplevel.set_parent`) instead hangs over its parent's tab as a dock
  dialog (guides/beui/docking.md), showing and hiding with it.
- `xdg-dialog-v1` (be-wayland's `dialog.rs`) lets a program mark such a window modal:
  `WindowInfo::modal` is set, and `Windows::blocked_by` names the innermost modal dialog of a
  window. be-wayland gives that dialog the keyboard whenever the parent would get it, and drops
  the parent's pointer input. An Xwayland window's parent is its `WM_TRANSIENT_FOR` and its modal
  flag `_NET_WM_STATE_MODAL`, which smithay's `X11Surface::is_popup` reads.
- `xdg-foreign` v2 lets a program parent its window to another program's, as a portal does with
  its file chooser: the child's `HostWindow::parent` is the exported window, and it is shown as
  that window's child like any other dialog.

### Window size

A floating window (every window under the desktop shell, and dialogs under workspace-ui) owns its
size:

- A new one is configured 0x0.
- `WindowInfo::size` is the size it last chose for itself (a commit that is not the answer to a
  size the compositor asked for). The shell's `DialogWindow` follows that size through
  `DockTabControl::resize_window`.
- A window shown at its own size is never told a size, so it stays free to resize itself. Only
  when the person resizes it, or it is docked, is it configured with a size, and then with the
  tiled states too.
- A 1x1 placeholder, which Firefox commits for its server-side-decorated windows and never grows
  on its own, opens at a default size instead.

A `WindowView` that changes size configures its client with the new size in that same frame.
Until the client commits a buffer of that size, its last one is drawn at its own size from the
view's top-left corner, clipped to the view, never stretched to fit. The size a surface is drawn
at is its buffer's size over its buffer scale, or the destination it set through viewporter.
`render.rs` draws it; `a_resized_window_is_drawn_unscaled_until_it_commits_the_new_size` checks
the pixels.

### Popups

Popups (xdg-popup, and Xwayland's override-redirect windows) are fitted to the screen their window
is on, not to the window, as the protocol intends. They are drawn and take input in a floating
overlay above the document, so a menu can reach past its window's edge.

### Window management in the desktop shell

| Input | Effect |
|---|---|
| Super alone, or the bar's programs button | Opens the program launcher |
| Super+F | Toggles fullscreen on the focused window (linux-desktop's `desktop.fullscreen` action, which sends `WindowAction::Fullscreen`) |
| Super+drag anywhere on a floating window, its bar included | Moves the window (`--session` and `--desktop`) |
| Super+right-drag | Resizes from the nearest edge or corner, or moves the split beside a docked window |
| Alt+Tab, Alt held | Opens the window switcher and walks the windows from the one used last; Alt+Shift+Tab goes the other way |
| Letting go of Alt | Focuses and raises the chosen window |
| Alt+Escape | Leaves the switcher where it was |
| Super+L | Locks the screen |
| Super+Escape | Takes shortcuts back from a window that inhibits them |

- To drive the switcher, hold Alt across the presses:
  `drive keydown alt; drive key tab tab; drive keyup alt`.
- A window's tab menu offers fullscreen too. `FloatingWindow` offers it on its tab;
  workspace-ui's `WindowTab` does not, since nothing there would take it back out.
- `foot --fullscreen` starts a window fullscreen.

### The program launcher

The launcher lists the `.desktop` programs of the XDG data dirs and starts the one picked, or what
was typed as a command, with `WAYLAND_DISPLAY` pointing at the compositor. linux-desktop draws it;
block-app only reads the `.desktop` files and their icons and starts the programs (the `Programs`
host value and `ProgramAction`, guides/plugins/editor_api.md). The workspace shell has no
launcher.

### Input to windows

Each window is a `WindowView`, a forwarding catcher like a plugin's region (the one input path in
guides/beui/input.md):

- beui decides which window a press, motion, scroll or key goes to, and `Compositor::after` turns
  what each window was forwarded into `wl_pointer` and `wl_keyboard` events, keys by their scan
  codes (`Event::Key`'s `code`).
- Where the shell draws over a window (a floating window's tab, say) the window takes nothing
  (`occluders`, from the region's occlusion).
- A press keeps going to the window or plugin it began on until it is let go.

### Shortcut inhibitors

A program that holds a `zwp_keyboard_shortcuts_inhibitor_v1` on its window (a remote desktop, a
virtual machine) gets the desktop's shortcuts itself while its window has the keyboard. The
window's `WindowView` sets `inhibits_shortcuts`, and beui keeps every window listener (capturing
actions, taps and F6) from it (guides/beui/input.md).

Two chords always work, as host actions:

- Super+L locks the screen.
- Super+Escape (`register_restore_shortcuts`, registered by block-app's `WindowActions`) takes
  the shortcuts back from the focused window until it loses the keyboard and gets it again.

The inhibitor is told it is active only while its window has the keyboard.

### Icons and window buttons

- A window's tab shows the icon its program set with `xdg-toplevel-icon-v1`. be-wayland reads the
  pixels it was handed (the smallest at least 64 pixels, shrunk to 64) or looks the name up in
  the icon theme. It reaches the shell as `HostWindow::icon` beside the title, which
  block-shell's `WindowTab` hands the dock as its `DockTab` `picture`. Without one the tab shows
  a generic glyph.
- The xdg shell offers only the fullscreen capability (`wm_capabilities`): a tab has nowhere to
  minimize to, and its size is the dock's, so maximizing would change nothing. A request to
  maximize is still answered with the maximized state, for programs that ask anyway.

GTK ignores those capabilities before 4.20 and from then on only greys the buttons out; its header
bar lays out its window buttons from GSettings' `org.gnome.desktop.wm.preferences button-layout`.
So the compositor (`src/gtk_settings.rs`) hides them through dconf:

- It writes a dconf profile into `$XDG_RUNTIME_DIR/be-wayland`: the profile dconf would otherwise
  use, plus a database of its own that sets and locks that key to `appmenu:` (no window
  buttons). The lock makes it win over the user's own value.
- Programs it launches get it as `DCONF_PROFILE` (`Compositor::environment`), so they read and
  write every other setting as before and the user's settings outside the session never change.
- Flatpak programs read the setting through the portal instead, which sees it only if it started
  after a `--session` exported the variable.

### Other protocol hints

- `wp-content-type-v1` and `wp-tearing-control-v1` hints are kept per surface
  (`State::content_type`, `State::tearing`) and do not change how anything is drawn yet.
- `wp-single-pixel-buffer-v1` buffers are drawn as one-pixel textures.

### Cursors

Under `--session` the DRM adapter draws the pointer into each output's frame, redrawing only the
outputs the cursor moved off or onto.

- beui's `CursorIcon`s (every CSS cursor; `name()` is the CSS name) come from the XCursor theme
  (`XCURSOR_THEME`, `XCURSOR_SIZE`, `XCURSOR_PATH` or the XDG icon directories, following
  `Inherits`), looked up by CSS name and then the old X11 names (`cursors.rs`), at the size the
  scale needs. Without a theme the built-in arrow is drawn.
- Over a window, a `wp_cursor_shape_v1` shape becomes the window's `CursorIcon`
  (`Windows::cursor`, which the winit backend shows too).
- A `wl_pointer.set_cursor` surface reaches the adapter through
  `beui_adapter_drm::SoftwareCursor` (block-app's `wayland/linux.rs`), redrawn whenever the
  client commits to it. A null cursor hides the pointer.

### Frame callbacks and presentation

Under `--session` the app runs a frame only when an output has flipped since the last one
(`beui-adapter-drm`'s `pacing.rs`). A window's `wl_surface.frame` callbacks and
`wp_presentation_feedback` are answered by the flip of the screen it mostly covers, with the
flip's time, refresh and sequence: the adapter reports each flip through
`beui_adapter_drm::Flips`, which block-app hands to `Compositor::presented` (`presentation.rs`).

Elsewhere there are no flips, so they are answered as soon as the window is painted, at the frame
clock's time.

### Activation

A program brings one of its windows forward, or another program's (a link opened in a browser
that is already running), with `xdg_activation_v1` (be-wayland's `activation.rs`).

- A client is given a token only while it has the keyboard and only for the serial of an input
  event since it got it.
- The token works once, within `TOKEN_LIFETIME` on the frame clock, and only while the keyboard is
  still where it was when the token was made.
- Every program the compositor starts (`Windows::run`, which the launcher and `ProgramAction::Run`
  use) gets a fresh token of the compositor's own as `XDG_ACTIVATION_TOKEN` and
  `DESKTOP_STARTUP_ID`, honoured whatever has the keyboard.
- An honoured request goes through `Windows::activate`, as `WindowAction::Focus` does, which bumps
  the window's `raised` count so the shell shows its tab.
- Anything else is ignored; the dock has no way yet to mark a tab as wanting attention.

### Not responding

The compositor pings a window's client when it is clicked, typed into, focused or asked to close.
A client that has not answered within `PING_TIMEOUT` (be-wayland's `state.rs`) is listed as not
responding: its window is dimmed and its tab offers Wait and Force close.

Closing a window that is not responding forces it: the compositor kills the process if it
launched it (or one of its descendants) and disconnects the client either way.

To try it, `kill -STOP` a client's pid to freeze it, and `kill -CONT` to bring it back.

## X11 programs

`be_wayland::Server::start_xwayland`, which block-app calls once the server is up, starts
`Xwayland` on the first free display, with smithay's `X11Wm` as its window manager
(`crates/be-wayland/src/xwayland.rs`).

- It needs the `xwayland` package; without it X11 programs just do not run.
- It starts with the compositor rather than when an X11 client first connects, because smithay
  creates the Xwayland client and its X11 sockets only inside `XWayland::spawn`, which starts the
  server at once.
- Programs the compositor launches get `DISPLAY`. Under `--session` both it and
  `WAYLAND_DISPLAY` go to D-Bus's activation environment and systemd's user manager
  (`src/session/environment.rs`).
- If Xwayland dies, its windows close and it starts again on the same display, up to three
  times.

To try it under `:dev`, the app's log names the display:

```sh
DISPLAY=:0 xterm
DISPLAY=:0 GDK_BACKEND=x11 xfce4-terminal --disable-server
```

### How X11 windows fit in

- An X11 toplevel is one more kind of window behind the same `WindowId`. `ClientWindow` holds a
  smithay `desktop::Window`, which is either an xdg toplevel or an `X11Surface`, so it is listed,
  docked, focused, resized, fullscreened and closed (`WM_DELETE_WINDOW`) like any other, and its
  input goes through the same forwarding.
- The keyboard focus is `focus::KeyboardFocus`, which for an X11 window also sets the X input
  focus.
- X11 has no ping here, so an X11 window is always responding.
- Each X11 window is configured at its place on the screen (`State::place`), so X coordinates are
  the screen's.
- An override-redirect window (a menu, tooltip or dropdown) positions itself in those
  coordinates and is drawn and hit-tested as a layer of the toplevel it belongs to (its
  `WM_TRANSIENT_FOR`, else the X11 window under the pointer or with the keyboard), drawn past
  that window's edge like an xdg popup.

## Clipboard and drag and drop

### Clipboard

The app's clipboard (beui's, through the runner's `Platform`) and the Wayland clipboard are one
(be-wayland's `selection.rs` and `compositor/clipboard.rs`).

- A program that sets the selection, through `wl_data_device` or the data-control protocols
  (`ext-data-control-v1` and `zwlr-data-control-v1`, which clipboard managers, `wl-copy` and
  `wl-paste` use), is asked for its text. The compositor reads it without blocking and hands it
  to the app with `Context::copy_text`; the program's own selection stays, with all its types.
- A selection that only offers images is not handed to the app.
- Everything the app copies (`Context::watch_copies`) becomes a selection the compositor serves
  as text.
- Plugins paste through the host's `request_paste`, so both work in a session, where the
  platform's clipboard is the DRM adapter's own.

### Middle-click paste

Middle-click paste (`zwp_primary_selection_device_manager_v1`, and the primary selection of the
data-control protocols) is off unless the input settings' "Middle-click paste" is on.

- The globals are always advertised, because a program that finds no primary selection protocol
  emulates one (Firefox then pastes the clipboard on a middle click).
- While the setting is off the primary selection just stays empty: `selection/primary.rs` answers
  every `set_selection` on a primary device, and `set_primary_selection` on a data-control
  device, by cancelling the source.
- Switching it off clears the current one, so it takes effect at once in running programs.
- X11 programs keep their own `PRIMARY` in Xwayland, which nothing bridges to Wayland, so they can
  still middle-click paste among themselves.

To try them, run `wl-copy TEXT` and `wl-paste` with `WAYLAND_DISPLAY` set to the app's socket,
and copy or paste in the app with `drive key ctrl+c` and `ctrl+v`.

### Drag and drop

A window's drag (`wl_data_device.start_drag`) goes to the window under the pointer: the press
stays with the window it began in, and while the drag lasts the compositor moves it to the window
beui finds under the pointer (`over` on each `Forwarded::Pointer`).

Only a shown window can be dropped on: a dock tab does not switch under a drag, and the drag's
icon is not drawn (BE3-296).

## Notifications

The desktop shell (`--session` and `--desktop`) serves `org.freedesktop.Notifications` on the
session bus. It asks for the name without replacing an owner, so under another desktop that
already runs a notification daemon it logs that and serves nothing.

### The host's part

block-app's `src/notifications/server.rs` keeps only what a plugin cannot do:

- It owns the name and reads the hints.
- It loads the picture: image-data, image-path, the app icon through be-wayland's `IconThemes`,
  then the desktop entry's icon, scaled down to at most 128 pixels and sent as RGBA.
- It answers `Notify` at once with an id it picks itself (`notifications/inbox.rs`;
  `replaces_id` keeps its id while the desktop still holds it).
- Each `Notify` and `CloseNotification` then waits in the `Notifications` host value, a
  `NotificationInbox` of numbered requests, until the shell's `NotificationReport` says it took
  them. The report also carries the `NotificationClosed` and `ActionInvoked` signals the host
  sends and the ids the desktop still holds.

### The shell's part

Everything else is linux-desktop's (`src/app/notifications/`):

- `center.rs` is what happens to a notification, and is where to test it: replacement in place,
  the toast's timeout (-1 is five seconds, 0 and critical urgency never), actions, `resident` and
  `transient`, the cap of 100 kept, and the reason every close carries.
- `markup.rs` turns the body into plain text.
- A toast whose time is up only hides: the notification stays until it is dismissed, an action
  is taken or its program closes it (the `persistence` capability). A transient one closes as
  expired instead.
- The toasts are a `beui::styled::Toasts` (title, picture and action buttons; clicking the body
  is the `default` action) anchored to the first screen with the desktop bar cut off it, with ids
  from `TOAST_IDS` up so other toasts can share the stack.
- They make way while the bell's list of notifications is open, and none show while the screen is
  locked (see The lock screen).

### Trying it

With a session bus of its own at ADDRESS (see Trying programs and services under `:dev`):

```sh
DBUS_SESSION_BUS_ADDRESS=ADDRESS ./scripts/buck run //crates/block-app:dev -- --desktop
DBUS_SESSION_BUS_ADDRESS=ADDRESS gdbus monitor --session -d org.freedesktop.Notifications
DBUS_SESSION_BUS_ADDRESS=ADDRESS notify-send --action=default=Open --action=reply=Reply Mail 'Lunch at noon?'
```

- Start the monitor (in the background, or another shell) before `notify-send`, so it sees the
  signals.
- `notify-send` with an action waits, and prints the action taken, until the notification
  closes.

## The tray

Programs such as Discord, Steam and Nextcloud show tray icons as StatusNotifierItems. The desktop
shell serves `org.kde.StatusNotifierWatcher` on the session bus from block-app's `src/tray/` and
is its `StatusNotifierHost`. Under another desktop whose watcher already owns the name, it
registers with that one as a host instead.

### The host's part (`src/tray/`)

- `client.rs` is one task on the D-Bus thread that every change reaches as an `Event`:
  - registrations;
  - `NameOwnerChanged` (an item goes with its program);
  - the items' `New*` signals, which read the item's properties again;
  - `com.canonical.dbusmenu`'s `LayoutUpdated` and `ItemsPropertiesUpdated`, which read its
    whole menu again with `GetLayout`.
- Reading runs in tasks of its own, ticketed so a slow answer never replaces a newer one.
- `icons.rs` picks the picture: when the item needs attention its attention icon, then `IconName`
  (in `IconThemePath`, then the icon themes through be-wayland's `IconThemes`) before
  `IconPixmap`, scaled to at most 48 pixels.
- `model.rs` is the pure part and where to test parsing: services, pixmaps (ARGB in network
  order), properties and menus (hidden entries dropped, `_` mnemonics removed).

### The shell's part

The shell watches the `Tray` host value, the items as `HostTrayItem`s with each menu a tree of
`TrayMenuEntry`s, and answers with `TrayRequest`s. linux-desktop's `src/app/tray.rs` shows each
item that is not `Passive` as a `PictureMenuButton` in the bar:

| Input | Request |
|---|---|
| Click | `Activate`, or opens the menu of an `ItemIsMenu` item |
| Right click | Opens the menu |
| Middle click | `SecondaryActivate` |
| Wheel | `Scroll`, 120 to a notch |
| Opening the menu | `MenuOpened`, which the host turns into `AboutToShow` and the `opened` event, or into the item's own `ContextMenu` when it has no dbusmenu |
| Choosing an entry | the `clicked` event |

### Trying it

The tests run a private bus with the test runtime's `dbus-daemon` (`BE3_DBUS_DAEMON`). To try it
in the dev app, start a bus as for notifications and run a tray program against it with the
compositor's socket, for example a Python script using `gir1.2-ayatanaappindicator3-0.1`:

```sh
DBUS_SESSION_BUS_ADDRESS=ADDRESS WAYLAND_DISPLAY=/tmp/be-wayland-PID python3 indicator.py
```

## Media and hardware keys

### The shell's part

- linux-desktop binds the volume, mute, mic mute, brightness and media transport keys as
  intercepting actions. `BINDINGS` in its `app/media.rs` is the one place that says which key
  does what.
- Being intercepting, the host hands them to the shell even while a program has the keyboard, and
  a held key repeats. They still work over the lock screen.
- Each key sends a `MediaRequest` host action (`editor.act`).
- A volume, mute or brightness key shows `beui::styled::LevelOsd`, an icon and a level bar on
  every monitor that fades out after 1.5 seconds, with the levels the host reports (the `Media`
  host value, `editor.host_value::<Media>()`).
- The bar's volume item (`app/volume.rs`) shows the same output level while the host reports one.
  Its popup sets the volume, the mute and the default output (`MediaRequest::SetVolume`,
  `SetMute`, `ChooseOutput`) from the sinks the host lists in `MediaLevels::outputs`.

### The host's part

block-app's `src/media.rs` starts its backends once a plugin watches the levels or the shell asks
for something, carries out the shell's requests only, and reports the levels back. The backends
run as tasks on the D-Bus thread:

| Backend | Does |
|---|---|
| `media/audio.rs` | Volume steps up to 100%, set volume, mute and mic mute on the default sink and source, the list of sinks and choosing the default one, over the PulseAudio protocol (the `pulseaudio` crate's protocol layer, pure Rust), which pipewire-pulse serves on `$XDG_RUNTIME_DIR/pulse/native`. It subscribes to the server's sink, source and server events and reports every change. |
| `media/backlight.rs` | Brightness steps through logind's `Session.SetBrightness` on `/sys/class/backlight`'s device (firmware before platform before raw). |
| `media/players.rs` | Play/pause, next, previous and stop to the MPRIS player that most recently started playing, or the first one listed, on the session bus. |

### Trying it

1. Run `pipewire`, `wireplumber` and `pipewire-pulse` with `XDG_RUNTIME_DIR` and
   `DBUS_SESSION_BUS_ADDRESS` set for the app under `:dev -- --desktop`.
2. The VM has no sound card, so give it outputs:

   ```sh
   pactl load-module module-null-sink sink_name=speakers sink_properties=device.description=Speakers
   ```

3. Press keys with `drive key volumeup` (or `micmute`, `play`, ...).
4. Change things from outside the app with `pactl set-sink-volume`, `set-sink-mute` and
   `set-default-sink`.

## The lock screen

In the desktop shell (`--session` and `--desktop`) the screen locks on:

- Super+L;
- the bar's power menu's Lock;
- idling for the display settings' "Lock the screen after" (by default when the screens turn
  off);
- under `--session`, logind's `Lock` for the app's own session (`loginctl lock-session`);
- under `--session`, before every suspend: `DesktopSession` holds a logind `delay` inhibitor for
  sleep and lets it go once the lock screen has been drawn twice (`src/session/sleep.rs`), taking
  it again on waking.

logind's `Unlock` is not obeyed: only a password unlocks. `ext-session-lock-v1` (external lockers
such as swaylock) is not implemented.

### Code

- `src/session/lock.rs` is the state machine.
- `src/session.rs`'s `ScreenLock` runs it.
- `src/ui/lock.rs` shows it as `styled::LockScreen`, a card on every screen of the document
  (`use_screens()`) over an opaque scrim.

### The password

- PAM checks it on a thread of its own, with the service `block-app` when `/etc/pam.d/block-app`
  exists and `login` otherwise.
- `--install-session` writes `/etc/pam.d/block-app` unless one is there, as
  `auth include common-auth` where that exists (as on Debian and Ubuntu) and `auth include login`
  elsewhere.
- libpam is opened with `dlopen` when a password is checked, so the app does not link it.
- After three wrong passwords each attempt waits, from five seconds doubling up to a minute.
- Anything that goes wrong (no libpam, a PAM error, a check that panics, an answer to an earlier
  attempt) leaves the screen locked. Only `Verdict::Accepted` for the attempt in flight unlocks
  it.
- Under `:dev` the app runs as the VM's user, so that user needs a password (`passwd`) for the
  lock to be tried.

### Keeping input from what is behind it

- The lock is a beui `Overlay` at `LayerLevel::Lock`, which nothing dismisses, which stays above
  every other layer and keeps the focus and the pointer.
- While it is open the host and window key listeners hear only media keys: no other window
  action, no plugin's intercepted chord, no Alt+Tab. Media keys still reach linux-desktop's
  intercepting media actions (through `compositor/intercept.rs`'s `on_global_key`), so volume and
  playback work over the lock.
- The app menu is closed.
- Windows need nothing of their own: each is a forwarding catcher like a plugin's region, so with
  the focus and the pointer held by the lock none is forwarded anything.
  `be_wayland::Compositor::set_locked` only lets go of the buttons and popups a window held.
- The host publishes the `ScreenLocked` host value, and linux-desktop shows no notification
  toasts while it is set, so none can be read or acted on behind the lock. They show again once
  it is unlocked.
- Screens still turn off while locked, and idle inhibitors from the windows behind it are
  ignored.

## Screens: idle, blanking and waking

The screens turn off after the display settings' "Turn off screens after" (ten minutes by
default). In a window nothing is turned off.

- `be_wayland::Compositor` counts the idle time on the frame clock from the frames that carried
  input (`Document::input_frames`). The count is held back while a shown window has a
  `zwp_idle_inhibitor_v1`, and `ext_idle_notifier_v1` is answered for idle daemons the same way.
- `Windows::idle()` is true once the time has run out. block-app hands it to
  `beui_adapter_drm::DisplayControl::set_blanked`, which turns the outputs' CRTCs off and stops
  drawing to them until input wakes the session.

### The input that wakes them

The input that wakes the screens reaches nothing. The adapter's `WakeGate` (`wake.rs`) drops,
before beui, plugins or Wayland clients see them:

- the waking input itself;
- all input for a grace period after it (`GRACE_USEC`, timed on libinput's event clock);
- the release of every press it dropped, whenever that comes.

Pointer motion it drops still moves the cursor. A release whose press was delivered before the
screens went off is still delivered, so no key stays held.

The adapter turns the screens on itself and reports the wake through
`DisplayControl::take_woken`, which block-app hands to `Compositor::woke` so the idle count
restarts.

## Switching terminals

Switching terminals hands the seat to whatever runs there, whose DRM master reprograms the CRTCs,
connectors and planes as it likes, so nothing the adapter cached about them survives.
`recovery.rs` decides what happens:

- When the seat comes back, every output (surface, swapchain, framebuffers, pending flips and
  fence watches) is dropped while the device is still paused.
- The device is reset to everything off, and outputs are built again for whatever is connected
  now, with the modes the display settings chose, so each one's first frame is a full modeset
  rather than a page flip.
- Coming back also wakes the screens and starts the keyboard's modifiers over.
- A frame that cannot be shown sets its output up again once. If that fails too, the output stops
  drawing and the problem is reported once through `Problems`, until the seat comes back or a
  display is plugged in or out.

## Power and logind

The desktop bar's power button locks, suspends, restarts, powers off or logs out.

- linux-desktop asks for what it may offer with `LinuxMessage::WatchPower` and sends
  `RequestPower`.
- `src/session/power.rs` decides what happens: programs are asked to close, and are given five
  seconds before the session ends or logind is asked to restart or power off.
- `src/session/logind.rs` talks to `org.freedesktop.login1`, including its `PrepareForSleep`
  signal.
- Logging out closes the app the way closing its window does. `beui-adapter-drm`'s
  `Session::end` then drops the app, the outputs, the input devices, the DRM device (which puts
  back what the console showed) and, last, the seat.

## The D-Bus thread

Every D-Bus conversation runs on one thread, `src/dbus.rs`:

- `dbus::spawn` runs a future there.
- `dbus::system()` is the shared system bus connection; `dbus::session()` the shared session bus
  connection.
- A task hands what it learns back through `host::waking_channel`, which wakes the event loop, so
  nothing on the UI thread waits on the bus.
