# The Linux desktop

block-app is also a Linux desktop: a Wayland compositor whose shell is the linux-desktop
plugin instead of workspace-ui. These flags start it that way:

- `--session` (Linux): run on the displays and input devices themselves through
  `beui-platform-drm` instead of in a window, from a virtual terminal with no other display
  server on it, as the desktop: the shell is linux-desktop instead of workspace-ui, on a
  session of its own. The bar's power menu logs out (see below), Ctrl+Alt+Backspace quits at
  once, as does holding four fingers on a touchscreen for three seconds, and Ctrl+Alt+F<n>
  switches terminals. guides/pinenote.md covers an e-paper tablet.
  It shows one window per display (see Displays, below); `BEUI_SCALE` sets every display's scale.
- `--desktop` (every native platform): the desktop shell, linux-desktop on its own session as
  under `--session`, in a normal window, with programs opening as windows inside it as they
  do in any windowed run. What needs the displays and input devices themselves, such as
  display modes, pointer settings and the power menu's actions other than Lock, is absent,
  since `beui-platform-drm` is not running: the power actions start only when the DRM seat
  provides its `DisplayControl`. On Windows and macOS (`src/arguments.rs` parses the flags)
  it is the shell alone: there are no program windows, notification server, tray, media,
  power, lock screen or launcher.
  The app test `the_desktop_shell_opens_and_closes_cleanly` (crates/app-tests) starts it headless and closes it.
- `--install-session [PREFIX]` (Linux): see below.

Everything the desktop gets from the host comes through `be3:linux-desktop`, a WIT package
beside the core plugin protocol rather than in it
(`crates/block-plugin-api/wit/deps/linux-desktop`): one interface per part of the session
(`windows`, `input`, `displays`, `power`, `media`, `wifi`, `epaper`, `notifications`,
`screen-lock`, `programs`, `tray`, sharing `types`' `host-image`), each a latest-value watch
(`WindowsWatch`, `PowerWatch`, ...) and, where the desktop asks something of the host, a
function (`power::request(PowerAction)`, `epaper::refresh()`, ...). Another host, a compositor
in another language, can implement it from the WIT alone. A plugin uses it only by naming
`be3:linux-desktop` in its manifest's `extensions` (linux-desktop, workspace-ui for the program
windows, and settings for displays, input devices and remaps); the host denies a watch or a call
from any other. linux-desktop shows a control only while the host offers what backs it
(`editor.offered::<W>()`, true once the host answered the watch): the Programs button, the
Super tap and placeholders' Open need `ProgramsWatch`, the bell `NotificationsWatch`, the power
menu `PowerWatch`, the media keys `MediaWatch` (the volume item, its `output`), the Wi-Fi item
`WifiWatch` (its `available`), and the tray its items. block-app publishes each as soon as the
service behind it runs, even empty, and a host with no Linux session (off Linux, the web and
Android) answers every watch of the package `unsupported` and drops its calls, which the desktop
sees as nothing offered. The web and Android apps have no `--desktop`: a desktop with none of
these is only a dock of block windows, which their own shells already are.

`./scripts/buck run //crates/block-app:dev -- --desktop` runs the desktop shell headless, to
drive like any other run (guides/running/drive.md).

## Program windows

be-wayland (`crates/be-wayland`) is the compositor; a program's toplevel is a `WindowView`, a
forwarding catcher the shell places like any other node. A floating window (every window under
the desktop shell, and dialogs under workspace-ui) owns its size: a new one is configured 0×0,
`WindowInfo::size` is the size it last chose for itself (a commit that is not the answer to a
size the compositor asked for), the shell's `DialogWindow` follows that size through
`DockTabControl::resize_window`, and a window shown at its own size is never told a size, so it
stays free to resize itself. Only when the person resizes it, or it is docked, is it configured
with a size, and then with the tiled states too. A 1×1 placeholder, which Firefox commits for its
server-side-decorated windows and never grows on its own, opens at a default size instead.

Popups (xdg-popup, and Xwayland's override-redirect windows) are fitted to the monitor their window
is mostly on, not to the window, as the protocol intends, and are drawn and take input in a floating
overlay above the document, so a menu can reach past its window's edge.

Where the host opens windows (`opens_windows()`, true only in space), every toplevel is a
window, and so a panel, of its own: workspace-ui shows each top program window as a floating dock
window (`FloatingWindow`) instead of a tab, as linux-desktop always does, and the dock shows a
window-presented dialog, which is how both shells show a program's dialogs (`AttachedWindow`), as
a `Window` placed `Over` its parent's window, centred on it and `dialog_outset` (the chrome the
styled dock draws around the body) larger than its body. A toplevel's popups are a `Window` of the
`WindowView` (`WindowRole::Popup`, so the toplevel keeps the keyboard and the popup its grab), placed `Over` the window the toplevel is in where the overlay would be, and opened
again whenever the popups' corner moves, as when a submenu grows them leftward or upward. The
compositor still places each toplevel at its rectangle in its own window's coordinates, the
keyboard follows the document's focus as anywhere, and a modal dialog still turns presses on its
parent into raising the dialog. Outside space all of this stays an overlay or a tab, as above.

A window's and its popups' drawings paint too (`Paints` on `WindowDraw`, which reads each surface's
texture back, opaque), so paintings and `drive paint` show what programs drew.
## Displays

Under `--session`, `beui-platform-drm` shows one window per display, macOS-like
(guides/beui.md, Displays under beui-platform-drm), and block-app opens them from the
platform's `Displays` handle (`display/linux.rs` offers them to `display/shown.rs`, and
`ui::Displays` opens a `Window` per display at its size, with `displays.placement(id)`). The
first display's window holds the host's whole UI (`ui::Root`: sign-in, the shell, dialogs); every
display's window holds its own lock screen and display-settings prompt (`DisplayChrome`), and the
host's toasts stand in the first's. Their test ids repeat from one display's window to the next:
`drive app click ID in WINDOW` (guides/running/drive.md) finds a target among one window's
nodes. Every display has its own scale: `BEUI_SCALE`, or what
`DisplayConfig::scales` names for its monitor. Elsewhere (a window, headless, the web, Android)
there are no displays and the UI is the main window's, as it always was.

The desktop is one space in points: the displays side by side along their bottom edges. Each
display's window shows its own region of the shell, macOS-like: the first display the shell's
frame (`ui::Root`'s `MainSurface`), each further one a `display(n)` region of the same instance
(`ui::SecondaryDisplay` mounts `compositor::ShellDisplay`, n being the display's place in the
list from 1), each at its display's size and scale. The shell's windows are the host's own,
placed in the desktop's points: the shell's regions say the host shows the windows the plugin
opens from them (`windows` in the screen request, set for the shell's frame and display regions
and for the windows themselves; guides/adding_a_plugin_editor.md), so every `Window` the shell
opens, each of linux-desktop's dock windows, becomes a `window(w)` region that block-app keeps
in `compositor/desktop.rs`: a list of `Floated` windows, each its rect in desktop points, in
stacking order (a press raises one). The shell's region on each display draws, between its own
surface and its floating layers (menus, popovers, toasts), the windows that display holds most
of (`desktop::shown_on`, `DesktopWindows` in `compositor/region.rs`), each a `PluginRegion` of
its window region placed at its rect less the display's corner, whose children are the program
windows and block editors the shell placed in it. A window opens where the plugin asked: at a
point of the region it opened from (`Over`, measured from that region's corner on the desktop),
at the pointer, or at the `placement` it kept (`desktop X Y`); the host answers with that
placement on the window's screen request, which the dock keeps. `BeginMove` and `BeginResize`
hand the press to the host: the window follows the pointer from where it was pressed (the
press's point and the window's rect are noted on the press, `desktop::follow`), the plugin sees
no moves meanwhile, and the release ends it and gives the new placement. While one moves it is
drawn on every display it overlaps and on the one the move began on, which holds the press;
once let go it is drawn only on the display holding most of it. Each region is laid out at the
document's scale and rendered at the raster of the display that shows most of it (each mount's
`EmbedPlacement::raster`, `RegionPlacement::raster` of the offer whose clip is largest), so a window moved onto a display at another scale is rendered at that scale once
it is mostly there; while it straddles two, the display that holds less of it shows the same
texture scaled. Program windows and block editors are children of the windows they are tabs
of, so they stack and take the focus with them. The pointer crosses between display windows
with a press held, by beui's cross-window rules (the platform reports each window's desktop
origin, `WindowReport::Moved`), and between the shell's regions by the protocol's `over`: a
press held in one of the shell's regions names the region the pointer is over, so a tab dragged
out of one window and over another, on any display, lands on that window's bar.

`--desktop` in a window, or headless, is the same with one display, the window: the shell's frame
fills it, and its windows are the host's own, drawn over it in that window's points. The plugin
tests do the same through block-ui-test's `BeuiTest::show_on_displays`, which shows the regions
and hosts the windows as block-app does, takes input in desktop points (`desk_press`,
`desk_move`, `desk_release`, `desk_drag`) and paints a display with the windows on it
(`desk_snapshot`); without it a plugin draws its windows inside its frame.

The shell learns the displays from `DisplaysWatch`: each `HostDisplay` carries its `rect` in the
desktop's points and its `scale`. linux-desktop draws, in every region, a bar along its foot and
a level display (`desktop.osd.{n}`, n the region's display), and in the frame alone its dock,
launcher, pickers and notification toasts (above that display's bar); `BeuiApp::Display`
draws the further displays' regions from what the frame's region shares (`app.rs`, `Desktop`).
The dock fills the frame above the bar, and its windows open over it.

be-wayland advertises one `wl_output` per display (`Compositor::set_monitors`, from the same
handle; `heads.rs`), at its desktop position and its scale rounded up, and keeps every window
entered into the output of the display it mostly covers (`wl_surface.enter`/`leave` and the
preferred buffer scale follow it as it moves), sends its frame callbacks and presentation feedback
through that output, and honours the output named in `xdg_toplevel.set_fullscreen` by making the
window fullscreen on that display. Xwayland binds the same outputs, so X11 programs see the same
monitors. A `WindowView` reports its rect in desktop coordinates (its window's origin added). In a
window or headless, the compositor's area is one output.

Under `--session` be-wayland's sources (the display's socket and clients, its own event loop,
buffers' fences) are inserted into `beui-platform-drm`'s calloop loop, which the platform hands out
as `beui_platform_drm::Loop` through `Setup` (`Compositor::join_loop`); they wake beui, whose
frame dispatches the clients. Elsewhere, where the platform's loop is winit's or the headless one
and takes no file descriptors, the "wayland watch" thread (`Server::watch`) polls them instead.

## Installing it as a session

    ./scripts/buck run //crates/block-app:install-session [-- PREFIX]

installs the app with its plugins in `PREFIX/lib/block-app` (`/usr/local` by default, asking
sudo when it needs root), links `PREFIX/bin/block-app` to it and writes
`/usr/share/wayland-sessions/block-app.desktop` (from `src/session/block-app.desktop`), which
runs it with `--session`, whatever the prefix: GDM, SDDM (older ones read nothing else) and
LightDM all read that directory, and LightDM (Mint's slick-greeter) never reads
`/usr/local/share`. When LightDM's `sessions-directory` (its `lightdm.conf.d` directories and
`/etc/lightdm/lightdm.conf`) leaves that directory out, the entry also goes in the first one it
lists; `X-LightDM-Session-Type=wayland` makes LightDM run it as Wayland from any directory. The
login screen then offers Block on its session menu, where it can be picked and remembered as
the default. Running it again replaces the install, and removes an entry an older install left
in `PREFIX/share/wayland-sessions`. greetd's greeters (gtkgreet, regreet) list the commands in
`/etc/greetd/environments` instead, so when that file exists the installed link with
`--session` is added to it once.

In session mode stdout and stderr, and so every `eprintln!`, every panic with its backtrace
and the output of every program the app starts, go to `$XDG_STATE_HOME/block/session.log`
(`~/.local/state/block/session.log`); the previous session's is kept beside it as
`session.previous.log`. The directory is the app id in lower case (`blockdev` for a build
without bundle metadata). The error that ends a session is also written to the stderr the app
started with, so a session run by systemd or greetd shows it in the journal. Problems a person should see at once also show as toasts in the
corner of the screen: `beui_platform_drm::Problems` carries the display and input ones,
`be_wayland::Compositor::on_failure` a program that could not be run, and `notices::report`
puts each in front of the person, as an error toast that stays until it is dismissed;
`notices::report_with` also keeps a full report (a crashed plugin's error, or the backtrace of
the panic that ended the last session) behind the toast's Copy report button. The host draws these itself, so they show before any shell
plugin runs (signing in, choosing an organization) and over either shell; they stack in the
top-right corner (`Toasts` with `edge=Edge::TopEnd`), clear of linux-desktop's bar and its
notification toasts at the bottom.

A panic must not take every program's window with it, so block-app's `panic_guard` sorts
panics by where they happen. Every panic is logged with its backtrace. One inside
`panic_guard::contain` is contained: a plugin, or the host's work for one, stops only that
plugin (guides/adding_a_plugin_editor.md), the shell included, while the compositor and its
clients keep running (see A shell that stops, below); a D-Bus task (`dbus::spawn`) ends alone with a toast; a
password check that panics fails. Any other panic, on the UI thread or on a thread nothing
contains (the block stack's, say), is the app's own: in a window the app shows its error
screen, and under `--session` it ends the session instead, through beui's runner and
`beui_platform_drm`'s `Session::end`, so the console comes back and the display manager takes
over rather than a frozen screen.

The screens turn off after the display settings' "Turn off screens after" (ten minutes by
default). `be_wayland::Compositor` counts the idle time on the frame clock from the frames that
carried input (`Document::input_frames`), held back while a window that is shown has a `zwp_idle_inhibitor_v1`, and answers
`ext_idle_notifier_v1` for idle daemons the same way. `Windows::idle()` is true once the time
has run out; block-app hands it to `beui_platform_drm::DisplayControl::set_blanked`, which turns
the outputs' CRTCs off and stops drawing to them until input wakes the session. In a window
nothing is turned off.

The input that wakes the screens reaches nothing: the platform's `WakeGate` (`wake.rs`) drops it
before beui, plugins or Wayland clients see it, along with all input for a grace period after
it (`GRACE_USEC`, timed on libinput's event clock), and the release of every press it dropped,
whenever that comes. Pointer motion it drops still moves the cursor. A release whose press was
delivered before the screens went off is still delivered, so no key stays held. The platform
turns the screens on itself and reports the wake through `DisplayControl::take_woken`, which
block-app hands to `Compositor::woke` so the idle count restarts.

Switching terminals hands the seat to whatever runs there, whose DRM master reprograms the
CRTCs, connectors and planes as it likes, so nothing the platform cached about them survives.
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
signal. Logging out closes the app the way closing its window does; `beui-platform-drm`'s
`Session::end` then drops the app, the outputs, the input devices, the DRM device (which
puts back what the console showed) and, last, the seat. Every D-Bus conversation runs on one thread, `src/dbus.rs`: `dbus::spawn` runs a
future there and `dbus::system()` is the shared system bus connection. A task hands what it
learns back through `host::waking_channel`, which wakes the event loop, so nothing on the UI
thread waits on the bus. `dbus::session()` is the shared session bus connection.

## A shell that stops

When the shell's plugin stops (a panic, a trap, a hang or its session failing; any stop
`Runtime::crashed` reports), block-app's `watch_shell` closes it and every editor under it,
drops its runtime and shows the host's own session and profile picker (`Screen::Sessions`)
with the error toast and its Copy report. Choosing a session or profile, or keeping the current
one, opens a fresh shell. The windows stay: the compositor keeps every client connected,
and `Compositor::set_owned(false)`, which block-app sets whenever no shell is on screen, takes
the keyboard, the pointer, held buttons and popups from them, drops any input meant for them
and pings every client each `PING_TIMEOUT`, so one that froze meanwhile is listed as not
responding. The lock screen is the host's and works throughout. The new shell sees the
windows in `WindowsWatch` and shows them as it shows any window; linux-desktop's saved layout
names window tabs by `HostWindowId`, which the compositor keeps, so a restored layout puts
each window back where it was, fullscreen and dialogs included. To try it, open the Plugins
panel from the bar's menu and press Crash beside `be3.linux-desktop`.

## Notifications

The desktop shell (`--session` and `--desktop`) serves `org.freedesktop.Notifications` on the
session bus from block-app's `src/notifications/server.rs`, asking for the name without
replacing an owner, so under another desktop that already runs a notification daemon it logs
that and serves nothing. The host keeps only what a plugin cannot do: it owns the name, reads
the hints, loads the picture (image-data, image-path, the app icon through be-wayland's
`IconThemes`, then the desktop entry's icon, scaled down to at most 128 pixels and sent as
RGBA), and answers `Notify` at once with an id it picks itself (`notifications/inbox.rs`:
`replaces_id` keeps its id while the desktop still holds it). Each `Notify` and
`CloseNotification` then waits in the `NotificationsWatch` value, a `NotificationInbox` of
numbered requests, until the shell's `NotificationReport` (`notifications::report`) says it
took them; the report also
carries the `NotificationClosed` and `ActionInvoked` signals the host sends and the ids the
desktop still holds.

Everything else is linux-desktop's (`src/app/notifications/`): `center.rs` is what happens to
a notification, and is where to test it: replacement in place, the toast's timeout (-1 is
five seconds, 0 and critical urgency never), actions, `resident` and `transient`, the cap of
100 kept, and the reason every close carries; `markup.rs` turns the body into plain text. A
toast whose time is up only hides: the notification stays until it is dismissed, an action
is taken or its program closes it, which is the `persistence` capability, except a transient
one, which closes as expired. The toasts are a `beui::styled::Toasts` (title, picture and
action buttons; clicking the body is the `default` action) anchored to the first display with
its bar cut off it, with ids from `TOAST_IDS` up so other toasts can share the stack.
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

## The tray

Programs such as Discord, Steam and Nextcloud show tray icons as StatusNotifierItems. The
desktop shell serves `org.kde.StatusNotifierWatcher` on the session bus from block-app's
`src/tray/` and is its `StatusNotifierHost`; under another desktop whose watcher already owns
the name it registers with that one as a host instead. `client.rs` is one task on the D-Bus
thread that every change reaches as an `Event`: registrations, `NameOwnerChanged` (an item
goes with its program), the items' `New*` signals (which read the item's properties again) and
`com.canonical.dbusmenu`'s `LayoutUpdated` and `ItemsPropertiesUpdated` (which read its whole
menu again with `GetLayout`). Reading runs in tasks of its own, ticketed so a slow answer never
replaces a newer one. `icons.rs` picks the picture: when the item needs attention its attention
icon, then `IconName` (in `IconThemePath`, then the icon themes through be-wayland's
`IconThemes`) before `IconPixmap`, scaled to at most 48 pixels. `model.rs` is the pure part and
where to test parsing: services, pixmaps (ARGB in network order), properties and menus
(hidden entries dropped, `_` mnemonics removed).

The shell watches `TrayWatch`, the items as `HostTrayItem`s, and answers with `TrayRequest`s
(`tray::request`). WIT has no recursive types, so a menu crosses flat: `TrayMenuItem`s in
order, each one deeper than the submenu it is in; the host builds that list straight from
dbusmenu's layout, and the shell nests it into `TrayMenuEntry` trees with
`HostTrayItem::entries`, the helper the shell's own menus nest with (block-plugin-api's
`nesting.rs`), which refuses a menu nested deeper than `MAX_MENU_DEPTH`. linux-desktop's `src/app/tray.rs` shows each
item that is not `Passive` as a `MenuButton` with a picture face in the bar: a click is `Activate` (or opens
the menu of an `ItemIsMenu` item), a right click opens the menu, a middle click is
`SecondaryActivate` and the wheel `Scroll`s, 120 to a notch. Opening the menu sends
`MenuOpened`, which the host turns into `AboutToShow` and the `opened` event, or into the item's
own `ContextMenu` when it has no dbusmenu; choosing an entry is the `clicked` event.

The tests run a private bus with the test runtime's `dbus-daemon` (`BE3_DBUS_DAEMON`). To try it
in the dev app, start a bus as for notifications and run a tray program against it with the
compositor's socket, for example a Python script using `gir1.2-ayatanaappindicator3-0.1`:

    DBUS_SESSION_BUS_ADDRESS=ADDRESS drive exec python3 indicator.py

## Media and hardware keys

linux-desktop binds the volume, mute, mic mute, brightness and media transport keys
(`BINDINGS` in its `app/media.rs`, the one place that says which key does what) as
intercepting actions, so the host hands them to the shell even while a program has the keyboard,
and a held key repeats. Each key sends a `MediaRequest` (`media::request` inside
`editor.act`), and a volume, mute or brightness key shows `LevelOsd` (`app/osd.rs`), an icon and
a `styled::Progress` bar on every display (one card in each display's region, `desktop.osd.{n}`) that fades out after 1.5 seconds, with the levels the host reports
(`editor.watch::<MediaWatch>()`). The bar's volume item (`app/volume.rs`) shows
the same output level while the host reports one, and its popup sets the volume, the mute and
the default output (`MediaRequest::SetVolume`, `SetMute`, `ChooseOutput`) from the sinks the
host lists in `MediaLevels::outputs`.

block-app does what a plugin cannot: `src/media.rs` starts its backends once a plugin watches
the levels or the shell asks for something, carries out the shell's requests only, and reports
the levels back. The backends run as tasks on the D-Bus thread:
- `media/audio.rs`: volume steps up to 100%, set volume, mute and mic mute on the default sink
  and source, the list of sinks and choosing the default one, over the PulseAudio protocol (the `pulseaudio` crate's protocol layer, pure Rust),
  which pipewire-pulse serves on `$XDG_RUNTIME_DIR/pulse/native`. It subscribes to the server's
  sink, source and server events and reports every change.
- `media/backlight.rs`: brightness steps through logind's `Session.SetBrightness` on
  `/sys/class/backlight`'s device (firmware before platform before raw).
- `media/players.rs`: play/pause, next, previous and stop to the MPRIS player that most
  recently started playing, or the first one listed, on the session bus.

To try them under `:dev -- --desktop`, run `pipewire`, `wireplumber` and `pipewire-pulse`
with `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` set for the app, and press them with
`drive app key volumeup` (or `micmute`, `play`, ...). The VM has no sound card, so give it outputs with
`pactl load-module module-null-sink sink_name=speakers sink_properties=device.description=Speakers`,
and change them from outside the app with `pactl set-sink-volume`, `set-sink-mute` and
`set-default-sink`.

## Wi-Fi

The bar's Wi-Fi item (linux-desktop's `app/wifi.rs`) shows while the host reports a Wi-Fi
device (`WifiStatus::available` in `WifiWatch`'s value): its glyph is the joined network's
strength, and its popup turns Wi-Fi on and off, lists the networks in range and joins or leaves
one with `WifiRequest`s. Opening the popup asks for a scan. A saved or open network is joined at
once; a protected one asks for its password first, and asks again when the host reports that
joining it `failed`. Enterprise and WEP networks (`WifiSecurity::Unsupported`) can only be joined
from a connection saved elsewhere.

block-app's `src/wifi/` starts once a plugin watches `Wifi` and talks to NetworkManager on the
system bus (`network_manager.rs`, a task on the D-Bus thread). It reads the first Wi-Fi device,
its access points and the saved Wi-Fi connections again after every signal NetworkManager sends,
so a change made elsewhere (`nmcli`, another program) shows at once. A saved network is joined
with `ActivateConnection`; anything else with `AddAndActivateConnection`, which saves it. A new
connection that fails to come up is deleted again, and one made with a new password replaces the
saved ones for that network once it is up. `model.rs` is the pure part and where to test it:
security from the access point's flags, the list (hidden networks left out, each name once at its
strongest), how a network is joined, and when an attempt has connected or failed. The test
`wifi_on_a_private_bus_lists_networks_and_connects` runs a fake NetworkManager on a private bus.

The VM has no Wi-Fi device, so under `:dev -- --desktop` the item does not show.

## The on-screen keyboard

beui's own on-screen keyboard (guides/beui.md, The on-screen keyboard) is the desktop's. Under
`--session` it opens by itself when a field is tapped and no libinput device with letter keys is
plugged in; the keyboard button beside the bar's clock (`desktop.keyboard`, which calls
`toggle_keyboard()` and so goes to the host as `WindowRequests::keyboard_toggles`) opens and closes
it whatever is plugged in. It is drawn by the host along the bottom of the window the focused field
is in (under `--session`, a display's window), and that window is laid out above it.

Programs reach it through `zwp_text_input_v3` (be-wayland's `text_input.rs`; there is no input
method protocol, since the keyboard is the compositor's own): a text input is entered on the surface
with the keyboard focus, and while one of the focused window's is enabled the window's
`WindowView` is a field that takes text (`ime`) with the number of enables so far as its
`keyboard_asks`, so a program enabling its field again, as GTK does when a focused entry is tapped,
asks for the keyboard again. Keys reach a program as any key does, by scan code; text no key types
(é, π, an emoji) reaches it as `commit_string`, and a program without an enabled text input never
gets it. X11 programs, and programs such as foot that use no text input, take the keyboard's keys
but never open it by themselves, so the bar's button is how it comes up over them. Pressing the
button gives the shell the focus, so tap the window again before typing into it.

## Input remaps

Under `--session` the keys, mouse buttons and wheel notches libinput reports can be remapped before
anything else sees them, the way AutoHotkey does on Windows. What remaps them is a remapper: a
WebAssembly module a plugin ships as its data, given a config the plugin's editor writes.

A remapper's settings are a block of the `InputRemapper` type (`be_block::InputRemapper`), which any
plugin can edit by declaring an editor for that block type. Its content is an envelope the host reads
without the plugin running: the plugin's id, the module's path in that plugin's data, and the config,
bytes only the module reads. The input settings' `remapper` (`RemapperChoice`: the block and the
editor that made it) picks one, in the shared input settings for every device or in a profile's for
its devices, the profile's replacing the shared one. The input settings' Remaps section offers every
editor of the `InputRemapper` type as the Remapper, and its Remaps link opens the chosen block with its
editor in a tab, making it first (with the chosen remapper, else the first there is) when there is none.

The remapper that ships is the settings plugin's Input Remaps editor (`editors/settings`, `input_remaps`): its block's config is TOML text, stored exactly as
written, comments and layout included, and its module is `crates/remap/module`, staged as its data
(`remapper.wasm`). The module is `be-remap`'s engine and text reader (`be_remap::text`, whose errors
name the group and rule): rules apply in every window or only while a window whose executable, app id
or title matches has the keyboard, and hold a key while a key or button is held, or tap a key for some
milliseconds or play a sequence of taps. Keys are named the way `be_remap::names` does (evdev's names,
AutoHotkey's for the common ones, and `Key:CODE` or `Button:CODE` for any other). Its page has two
tabs over the same text. Editor shows each group and rule as settings rows and changes the text through
`be_remap::edit`, which reads an outline of it as written (so a name that does not read yet still shows)
and edits it in place with `toml_edit`, so comments and layout outside what changed stay; each change is
stored once the whole text reads, and a problem is shown until it does. TOML is the text itself, stored
when Apply is pressed and it reads.

A module exports `memory`, `be_remap_buffer` and `be_remap_handle`, and trades `be_remap::protocol`
requests (start with the config, focus, input, timer) and replies in bincode; a start can answer with a
problem instead, such as a config it cannot read. `be-remap-host` runs it on wasmi: each call gets a
fixed amount of fuel and the instance a capped memory, so a module that loops, traps or answers with
nonsense only stops itself. The host fails open: on any failure it lets go of every key and button the
module held down, passes the input that failed through unchanged, says so with a toast, and starts the
module again on the next input; after three failures in a minute, or a config it cannot read, it stays
off until the remapper changes. Ctrl+Alt+Backspace, Ctrl+Alt+F<n>, Super+L and Super+Escape never reach
the module (the modifiers they need are pressed for them if the module held them back), and while the
screen is locked nothing does, so a module can neither keep the session from ending nor read the
password. Locking lets go of what the module held, too.

block-app (`src/remap/linux.rs`) holds the chosen block, reads the envelope whenever it changes, fetches
the module from the plugin's data (`discovery::data`, only from a plugin that has an `InputRemapper`
editor) when the plugin or path changes and only reconfigures the running one when only the config
does, and keeps the last module and config it ran in its own database (`remapper`), so the session
remaps from its first input, before the workspace is open. `RemapsWatch` says whether this
device runs a remapper, runs none, or stopped it. The DRM platform hands every key, button and wheel
notch to the `InputFilter` block-app sets on its `InputControl`, after the `WakeGate`, delivers what the
filter returns as if libinput had sent it (so a remapped `VolumeUp` reaches linux-desktop's media keys
and shows the level, and a remapped key goes to the window with the keyboard), and runs a timer for
the filter's next deadline, on its monotonic clock. Changes the filter makes outside input, such as
the releases when the focus, the lock or the remapper change, go through `InputControl::inject`.
block-app tells the filter which window has the keyboard (`Compositor::keyboard_window`); its
executable is the last part of the first argument of its process's command line, which for a Proton
or Wine game is the Windows path of its `.exe`. Outside a session nothing runs.

## The lock screen

In the desktop shell (`--session` and `--desktop`) the screen locks on Super+L, the bar's power
menu's Lock, idling for the display settings' "Lock the screen after" (by default when the screens
turn off), and, under `--session`, logind's `Lock` for the app's own session (`loginctl
lock-session`) and before every suspend: `DesktopSession` holds a logind `delay` inhibitor for
sleep and lets it go once the lock screen has been drawn twice (`src/session/sleep.rs`), taking it
again on waking. `src/session/lock.rs` is the state machine, `src/session.rs`'s `ScreenLock` runs
it, and `src/ui/lock.rs` shows it as its `LockScreen`: a `Modal` filling its window at
`LayerLevel::Lock` over an opaque scrim, in every display's window (one in the main window
elsewhere). logind's `Unlock` is not obeyed: only a password unlocks.

The password is checked by PAM on a thread of its own, with the service `block-app` when
`/etc/pam.d/block-app` exists (`--install-session` writes it unless one is there, as
`auth include common-auth` where that exists, as on Debian and Ubuntu, and `auth include login`
elsewhere) and `login` otherwise. libpam is opened with `dlopen` when a password is checked, so the app
does not link it. After three wrong passwords each attempt waits, from five seconds doubling up to
a minute. Anything that goes wrong - no libpam, a PAM error, a check that panics, an answer to an
earlier attempt - leaves the screen locked; only `Verdict::Accepted` for the attempt in flight
unlocks it. Under `:dev` the app runs as the VM's user, so that user needs a password
(`passwd`) for the lock to be tried.

While it is locked, two things keep input from everything behind it: the lock is a beui
`Overlay` at `LayerLevel::Lock`, which nothing dismisses, which stays above every other layer and keeps
the focus and the pointer, and while it is open the host and window key listeners hear only media keys (no other window action, no plugin's
intercepted chord, no Alt+Tab); and the app menu is closed. Windows need nothing
of their own for it: each is a forwarding catcher like a plugin's region, so with the focus and the
pointer held by the lock none is forwarded anything. `be_wayland::Compositor::set_locked` only lets
go of the buttons and popups a window held. Screens still
turn off while locked, and idle inhibitors from the windows behind it are ignored. Media keys
still reach linux-desktop's intercepting media actions (through `compositor/intercept.rs`'s
`on_global_key`), so volume and playback work over the lock. The host publishes
`ScreenLockedWatch`, and linux-desktop shows no notification toasts while it is set, so
none can be read or acted on behind the lock; they show again once it is unlocked.
`ext-session-lock-v1` (external lockers such as swaylock) is not implemented.

## Wayland programs

On Linux the app is a Wayland compositor. Its socket is `wayland-<n>` in `XDG_RUNTIME_DIR`.
When that is unset, as it is under `:dev`, be-wayland (`src/runtime_dir.rs`) makes a private
one, `/tmp/be-wayland-<pid>`, with the socket `wayland-0` in it, gives it to the programs it
starts as their `XDG_RUNTIME_DIR` and removes it when the server goes; at start it removes
those that processes now ended left behind (by name and owner). In the desktop shell
the program launcher, which tapping Super alone or the bar's programs button opens, lists the
`.desktop` programs of the XDG data dirs and starts the one picked, or what was typed as a
command, with the variables of `Compositor::environment` (`XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`,
`DISPLAY` for Xwayland and `DCONF_PROFILE`). linux-desktop draws it; block-app only reads the
`.desktop` files and their icons and starts the programs (`ProgramsWatch` and
`programs::request(ProgramAction)`, guides/adding_a_plugin_editor.md). The workspace shell has no launcher. A
program can also be started against a driven app directly: the app hands the same variables to
`drive app env` (`App::environment`), so `drive exec foot` runs `foot` inside it, and
`eval "$(drive app env)"` sets them in a shell for anything else, such as `wl-paste`.

A top-level window opens as a tab in the workspace shell and floats in a dock window of its own in the desktop shell, unless it was launched to open a block (Files opened in programs, below). A window with a parent (`xdg_toplevel.set_parent`) instead hangs over its parent's tab as a dock dialog (see the dock in guides/beui.md), showing and hiding with it. `xdg-dialog-v1` (be-wayland's `dialog.rs`) lets a program mark such a window modal (an XWayland window's parent is its `WM_TRANSIENT_FOR` and its modal flag `_NET_WM_STATE_MODAL`, which smithay's `X11Surface::is_popup` reads): `WindowInfo::modal` is set, and `Windows::blocked_by` names the innermost modal dialog of a window, to which be-wayland gives the keyboard whenever the parent would get it, and for which it drops the parent's pointer input. `weston-simple-shm` (from the `weston` package)
and `foot` are small clients to try it with. `foot --fullscreen` starts one fullscreen, and in the desktop shell Super+F toggles fullscreen on the focused
window (linux-desktop's `desktop.fullscreen` action, which sends `WindowAction::Fullscreen`). A window's
tab menu offers the same (`FloatingWindow` offers fullscreen on its tab; workspace-ui's
`WindowTab` does not, since nothing there would take it back out). Under `--session` and `--desktop`, Super+drag anywhere on a floating window, its bar
included, moves the window, and Super+right-drag resizes it from its nearest edge or corner,
or moves the split beside a docked one. In the desktop shell Alt+Tab, with Alt held, opens the window switcher and walks the
windows from the one used last (Alt+Shift+Tab the other way); letting go of Alt focuses and
raises the chosen one and Alt+Escape stays where it was. To drive it, hold Alt across the
presses: `drive app keydown alt; drive app key tab tab; drive app keyup alt`.

Each window is a `be_wayland::WindowView`, a forwarding catcher like a plugin's region (see
guides/beui_keyboard.md, One input path): beui decides which window a press, motion, scroll or key
goes to, and `Compositor::after` turns what each window was forwarded into `wl_pointer` and
`wl_keyboard` events, keys by their evdev codes (`Event::Key`'s `evdev`) and buttons by theirs: a
button beyond beui's five is `PointerButton::Other` carrying its evdev code, which the DRM platform
reads from libinput and the window gets unchanged (`drive app down TARGET button:0x115`). Where the shell draws over a
window (a floating window's tab, say) the window takes nothing (`occluders`, from the region's
occlusion), and a press keeps going to the window or plugin it began on until it is let go.

A pen (a drawing tablet's tool or a screen's stylus, `Event::Pen` in guides/beui.md) reaches a
window as a `zwp_tablet_tool_v2` of one tablet be-wayland makes when a pen is first used
(`src/tablet.rs`), with its pressure and tilt; the tip is the tool's `down` and the barrel buttons
are `BTN_STYLUS` and `BTN_STYLUS2`. A program that never asked for a tablet seat
(`zwp_tablet_manager_v2.get_tablet_seat`) gets the pen as its pointer instead, the tip a left
click, decided when the pen comes over it and kept while the tip is down. Under `--session` the
DRM platform reads libinput's tablet tools over the area of every display, as it does an absolute
pointer, and moves the cursor with them; with one display both turn with its rotation (below).

A finger (`Event::Touch`) reaches a window as a `wl_touch` point (`src/touch.rs`), in the
coordinates of the surface, sub-surface or popup it went down on, where it stays until it lifts,
as a press does; each finger goes down on the window under it, so two windows can each hold
one. A cancelled finger cancels the client's whole touch sequence, as `wl_touch.cancel` means.
A client with no `wl_touch` gets its first finger as its pointer instead, a left click, and its
other fingers not at all, decided when the finger goes down. The seat offers touch only while a
touchscreen is attached (beui-platform-drm's `WindowReports::touchscreen_attached`, read as
`Context::touchscreen_attached`) or once a finger has been seen, since GTK, SDL and browsers
take the capability as a touchscreen and change their interfaces for it (a browser tells pages
it has touch). be-wayland's own default touch grab gives each point its own surface, where
smithay's would send every later finger to the first one's, so a client's touch drag and drop
(`wl_data_device.start_drag` from a touch serial) is not taken.

A screen's rotation (`ScreenSettings::rotation`, through `DisplayConfig::rotations`) turns its
picture clockwise on the panel: `Clockwise90` puts the picture's top along the panel's right
edge, so the device is held with that edge up. The platform's `Screen` lays out and draws the
picture upright at its turned size (`Rotation::turned`), so the layout, beui and the nested
Wayland clients only ever see an upright screen of that size with `Transform::Normal`, and
turns it onto the scanout buffer on the GPU (`gpu.rs`'s sprite pass with turned corners, where
an unturned screen copies), the software pointer and the damage clips with it
(`rotation.rs`). It never asks the plane to rotate, which most display devices, the e-paper
panel's included, cannot do. Absolute input arrives in the panel's own orientation
(libinput's calibration matrix lines a touchscreen up with the panel unturned) and is turned
back (`layout::absolute`), before beui or a window sees it: touch onto the first display, an absolute pointer or a pen onto the
one display when there is one and across all of them unturned otherwise, and a pen's tilt
with it. A relative pointer is already in the picture's terms. A rotation applies live, rebuilding
the output's `Screen` as a mode change does, and never asks to be kept.

The rotation is picked with `block_editor_beui::RotationPicker`, which both the Display page
and the bar use. When the display settings' `rotate_in_bar` is on and the host reports a
screen, linux-desktop's bar shows a rotate button (`desktop.rotate`, `app/rotate.rs`) whose
popover holds the picker for the first display in `Displays`. It reads and writes the setting
through `block_editor_beui::LayeredSettings::this_device`, as the Display page does, so the
rotation of a monitor known by its EDID serial goes to every device and that of a panel known
only by its connector stays in this device's profile, and block-app's `SettingsSync` applies it
as it does an edit from the Display page.

A window's tab shows the icon its program set with `xdg-toplevel-icon-v1`: be-wayland
reads the pixels it was handed (the smallest at least 64 pixels, shrunk to 64) or looks the
name up in the icon theme, and it reaches the shell as `HostWindow::icon` beside the title,
which block-shell's `WindowTab` hands the dock as its `DockTab` `picture`. Without one the
tab shows a generic glyph. `xdg-foreign` v2 lets a program parent its window to another
program's, as a portal does with its file chooser: the child's `HostWindow::parent` is the
exported window, and it is shown as that window's child like any other dialog. The xdg shell offers only the
fullscreen capability (`wm_capabilities`): a tab has nowhere to minimize to, and its size is
the dock's, so maximizing would change nothing (a request to maximize is still answered
with the maximized state, for programs that ask anyway). GTK ignores those capabilities
before 4.20 and from then on only greys the buttons out; its header bar lays out its window
buttons from GSettings' `org.gnome.desktop.wm.preferences button-layout`. So the compositor
(`src/gtk_settings.rs`) writes a dconf profile into `be-wayland` in its runtime directory: the
profile dconf would otherwise use, plus a database of its own that sets and locks that key to
`appmenu:` (no window buttons); the lock makes it win over the user's own value. Programs
it launches get it as `DCONF_PROFILE` (`Compositor::environment`), so they read and write
every other setting as before and the user's settings outside the session never change.
Flatpak programs read the setting through the portal instead, which sees it only if it
started after a `--session` exported the variable. `wp-content-type-v1` and
`wp-tearing-control-v1` hints are kept per surface (`State::content_type`,
`State::tearing`) and do not change how anything is drawn yet; `wp-single-pixel-buffer-v1`
buffers are drawn as one-pixel textures.

A program that holds a `zwp_keyboard_shortcuts_inhibitor_v1` on its window (a remote
desktop, a virtual machine) gets the desktop's shortcuts itself while its window has the
keyboard: the window's `WindowView` sets `inhibits_shortcuts`, and beui keeps every
window listener - capturing actions, taps and F6 - from it (guides/beui_keyboard.md). Two chords always work, as host actions:
Super+L locks the screen, and Super+Escape (`register_restore_shortcuts`, registered by
block-app's `WindowActions`) takes the shortcuts back from the focused window until it
loses the keyboard and gets it again. The inhibitor is told it is active only while its
window has the keyboard.

Under `--session` the DRM platform draws the pointer into each output's frame, redrawing only the
outputs the cursor moved off or onto. beui's `CursorIcon`s (every CSS cursor; `name()` is the CSS
name) come from the XCursor theme (`XCURSOR_THEME`, `XCURSOR_SIZE`, `XCURSOR_PATH` or the XDG
icon directories, following `Inherits`), looked up by CSS name and then the old X11 names
(`cursors.rs`), at the size the scale needs; without a theme the built-in arrow is drawn. Over a
window, a `wp_cursor_shape_v1` shape becomes the window's `CursorIcon` (`Windows::cursor`, which
the winit backend shows too), and a `wl_pointer.set_cursor` surface reaches the platform through
`beui_platform_drm::SoftwareCursor` (block-app's `wayland/linux.rs`), redrawn whenever the client
commits to it; a null cursor hides the pointer.

Under `--session` the app runs a frame only when an output has flipped since the last one (`beui-platform-drm`'s `pacing.rs`), and a window's `wl_surface.frame` callbacks and `wp_presentation_feedback` are answered by the flip of the display it mostly covers, with the flip's time, refresh and sequence: the platform reports each flip through `beui_platform_drm::Flips`, which block-app hands to `Compositor::presented` (`presentation.rs`). Elsewhere there are no flips, so they are answered as soon as the window is painted, at the frame clock's time.

A `WindowView` that changes size configures its client with the new size in that same frame.
Until the client commits a buffer of that size, its last one is drawn at its own size from the
view's top-left corner, clipped to the view, never stretched to fit; the size a surface is
drawn at is its buffer's size over its buffer scale, or the destination it set through
viewporter. `render.rs` draws it, `a_resized_window_is_drawn_unscaled_until_it_commits_the_new_size`
checks the pixels. A surface that commits on its own (a toplevel, or a desynchronized
sub-surface such as Firefox's content) and changes its viewport without attaching a buffer keeps
the view it had with its last buffer until the next one arrives (`held_view.rs`): Firefox sets
its content's destination to the new window size as soon as it is configured and commits it
before it has drawn at that size, which would otherwise stretch the old buffer. A synchronized
sub-surface's viewport applies with its parent's commit, and a single-pixel buffer's at once.
A program brings one of its windows forward, or another program's (a link opened in a browser
that is already running), with `xdg_activation_v1` (be-wayland's `activation.rs`). A client is
given a token only while it has the keyboard and only for the serial of an input event since it
got it; the token works once, within `TOKEN_LIFETIME` on the frame clock, and only while the
keyboard is still where it was when the token was made. Every program the compositor starts
(`Windows::run`, which the launcher and `ProgramAction::Run` use) gets a fresh token of the
compositor's own as `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`, honoured whatever has the
keyboard. An honoured request goes through `Windows::activate`, as `WindowAction::Focus` does,
which bumps the window's `raised` count so the shell shows its tab. Anything else is ignored;
the dock has no way yet to mark a tab as wanting attention.

### Windows kept across restarts

A desktop session keeps its program windows: the next time it is opened with none of them
running, as after the computer restarts, each one's tab is where the window was and shows a
placeholder instead, with the program's icon and name, the window's old title, Open and
Dismiss, and the bar offers to open them all. block-shell's `Workspace` (with `keeps_windows`,
which only linux-desktop sets) saves a `KeptWindow` per top-level window beside the dock layout
in its `layout` view state. Each window tab is keyed by a kept key that outlives the window
(`window_tab(key)`), and `kept.rs` binds live windows to keys: a window a record names is bound
to its key (a shell started again over the same compositor keeps every window), a window that
appears after the restore takes the placeholder it was launched from, else the oldest unbound
one with its app id, and any other window is its own key. The dock never sees a binding change:
`WindowTab` shows the bound window's `WindowPanel`, or the placeholder linux-desktop hands it
when none is bound. While the host's `PowerAvailability` says the session is `ending`, nothing
is saved, so the programs closing as it ends stay kept.

Window ids start from the compositor's start time in microseconds, so an id never names two
windows across runs: a saved record's window id is only ever adopted by that window, and a new
window's own id never collides with a kept key. The host fills `HostWindow::origin`: when the
compositor launched the program or an ancestor of it, the `.desktop` entry it was launched from
and the `launch` number the shell put on its `ProgramAction::Launch` or `Exec`; otherwise the
program's executable, the first word of the client's `/proc` command line (an X11 window's
`_NET_WM_PID`). The arguments are never kept, since saved layouts outlive the process and an
argument can be a secret. linux-desktop opens a placeholder with that entry, else one from
`Programs` whose `StartupWMClass` or id matches the app id, else the executable. The host runs
a `Launch` that arrives before it has listed the programs once the list is in.

### Files opened in programs

On Linux a program installed with a `.desktop` entry is an editor of every block whose type has a
plain-file form (guides/type_modules.md) and whose file name has an extension the entry's
`MimeType` names, so a book (an EPUB file block, the fallback type that holds any bytes; it needs
no type of its own) opens in KOReader, and later a `.kra` in Krita or a `.blend` in Blender, as a
tab of whichever shell opened it, the workspace-ui inside linux-desktop included. be-wayland's
`programs` reads `MimeType` (`DesktopEntry::mime_types`) and turns MIME types into extensions with
shared-mime-info's `mime/globs2` and `mime/aliases` in the XDG data dirs (`MimeGlobs`).

block-app's `src/programs.rs` scans the programs once the compositor runs and adds one
`EditorDescriptor` per program that opens anything to the catalog every plugin gets: `Edits::Any`,
the program's name, and `extensions` (shell.wit's `editor-descriptor`). Its editor id is `programs::editor_id`, `program\0` in the
high half and a hash of the desktop id in the low, so a tab saved with it names the same program
on the next start. block-ui's `BlockCatalog` matches such an editor by the block's file name
(`BlockInfo::file_name`: its intrinsic name, the file it came from, so a renamed book still
matches) and only for a type with a plain-file form: `editors_for(block_type, file_name)` lists
it after the type's own editors (the file tree's Open with), and `own_editor` falls back to the
first one for a type with no editor of its own (the EPUB's default). Elsewhere (the web,
Android, Windows, macOS) no program is listed, so nothing changes there.

The plugin side is only that editor id on a `ChildTarget`; the host does the rest. When a
child region asks for a block in a listed program (`programs::opens`), `HostedChild` shows a
`ProgramSurface` instead of a plugin, and `programs::show` starts a session for the block:

- **Write out.** `be::plain_file` asks the block's type module for `to-file`, the same
  file form `be sync` writes, and `programs/files.rs` puts it at a path that stays the same for
  the block, `DATA/files/<block id>/<file name>` in the app's data directory (not a cache). It
  writes only when the bytes differ, so what a program keeps beside the file stays valid:
  KOReader's default `document_metadata_folder` (`doc`) keeps the reading position in
  `<name>.sdr/` next to the book, so the book resumes with KOReader's own settings and none of
  ours. `.written` beside the file records the size and time it was left at.
- **Launch.** `wayland::Programs::open_file` runs the entry with the path in place of its first
  `%f`, `%F`, `%u` or `%U` (appended when the line has none), tagged with the block, so its
  first top-level window is bound to the session (`wayland::opening_windows`) and drawn in the
  region by `WindowSurface`. Until then the region says it is opening; once the program exits
  it says so with Open again (`program.reopen`).
- **Hidden from the shell.** A bound window is left out of `WindowsWatch`
  (`programs::bound_windows`), so linux-desktop does not float it and keeps no placeholder for
  it; its dialogs are listed as top-level windows of their own.
- **Write back.** The file's folder is watched (`programs/watch.rs`, the `notify` crate), and
  when the file's size or time differs from what was recorded, its bytes go back into the block
  as a `from-file` change (`be::from_file`), a commit like any other edit. A file changed while
  nothing watched it (the app ended first) is taken back instead of written over when the block
  is next opened.
- **Close.** Closing the tab closes its editor (`programs::close`, from `close_editor`, which
  also closes the sessions of blocks an editor being closed showed), which asks the window to
  close (`xdg_toplevel.close`), so the program saves as it would when its own window is closed.

A restored tab names the same block and editor, so a session that is started again opens the
book in KOReader by itself, where it was left: no placeholder, and no program arguments stored.
Two sessions never share a file, since the path is per block. A single-instance program that
hands a second file to its running process (Krita's default) opens its window from that process,
which is not tagged, so it shows as an ordinary window instead.

To try it, give a program a `.desktop` entry in `/usr/local/share/applications` (`:dev` moves
`XDG_DATA_HOME`, so the user's own entries are not read there), get a file into a workspace with
`be WORKSPACE sync DIR` (an agent's credentials, guides/tracker.md), and click it in the files.

### X11 programs

`be_wayland::Server::start_xwayland`, which block-app calls once the server is up, starts
`Xwayland` (the `xwayland` package; without it X11 programs just do not run) on the first free
display, with smithay's `X11Wm` as its window manager (`crates/be-wayland/src/xwayland.rs`).
It starts with the compositor rather than when an X11 client first connects: smithay creates
the Xwayland client and its X11 sockets only inside `XWayland::spawn`, which starts the server
at once. Programs the compositor launches get `DISPLAY`, and under `--session` both it and
`WAYLAND_DISPLAY` go to D-Bus's activation environment and systemd's user manager
(`src/session/environment.rs`). If Xwayland dies, its windows close and it starts again on the
same display, up to three times. To try it under `:dev`, the app's log names the display:

    DISPLAY=:0 xterm
    DISPLAY=:0 GDK_BACKEND=x11 xfce4-terminal --disable-server

An X11 toplevel is one more kind of window behind the same `WindowId`: `ClientWindow` holds a
smithay `desktop::Window`, which is either an xdg toplevel or an `X11Surface`, so it is listed,
docked, focused, resized, fullscreened and closed (`WM_DELETE_WINDOW`) like any other, and its
input goes through the same forwarding. The keyboard focus is `focus::KeyboardFocus`, which for
an X11 window also sets the X input focus. X11 has no ping here, so an X11 window is always
responding. Each X11 window is configured at its place on the desktop (`State::place`), so X
coordinates are the desktop's, the space Xwayland's outputs (the same `wl_output`s, see Displays) are laid out in; an override-redirect window (a menu, tooltip or dropdown)
positions itself in those coordinates and is drawn and hit-tested as a layer of the toplevel it
belongs to (its `WM_TRANSIENT_FOR`, else the X11 window under the pointer or with the
keyboard), drawn past that window's edge like an xdg popup (see Program windows above).

The compositor pings a window's client when it is clicked, typed into, focused or asked to
close, and a client that has not answered within `PING_TIMEOUT` (be-wayland's `state.rs`) is listed as
not responding: its window is dimmed and its tab offers Wait and Force close. Closing a window
that is not responding forces it: the compositor kills the process if it launched it (or one
of its descendants) and disconnects the client either way. `kill -STOP` on a client's pid
freezes it to try this, and `kill -CONT` brings it back.

### Clipboard and drag and drop

The app's clipboard (beui's, through the runner's `PlatformServices`) and the Wayland clipboard are one
(be-wayland's `selection.rs` and `compositor/clipboard.rs`). A program that sets the selection,
through `wl_data_device` or the data-control protocols (`ext-data-control-v1` and
`zwlr-data-control-v1`, which clipboard managers, `wl-copy` and `wl-paste` use), is asked for its
text, which the compositor reads without blocking and hands to the app with `Context::copy_text`;
the program's own selection stays, with all its types. Everything the app copies
(`Context::watch_copies`) becomes a selection the compositor serves as text. Plugins paste
through the host's `request_paste`, so both work in a session, where the platform's clipboard is
the DRM platform's own. A selection that only offers images is not handed to the app.

Middle-click paste (`zwp_primary_selection_device_manager_v1`, and the primary selection of the
data-control protocols) is off unless the input settings' "Middle-click paste" is on. The globals are always
advertised, because a program that finds no primary selection protocol emulates one (Firefox
then pastes the clipboard on a middle click). While the setting is off the primary selection
just stays empty: `selection/primary.rs` answers every `set_selection` on a primary device, and
`set_primary_selection` on a data-control device, by cancelling the source, and switching it
off clears the current one, so it takes effect at once in running programs. X11 programs keep
their own `PRIMARY` in Xwayland, which nothing bridges to Wayland, so they can still
middle-click paste among themselves.

To try them, run `wl-copy TEXT` and `wl-paste` (the `wl-clipboard` package) with
`drive exec`, and copy or paste in the app with `drive app key ctrl+c`
and `ctrl+v`.

A window's drag (`wl_data_device.start_drag`) goes to the window under the pointer: the press
stays with the window it began in, and while the drag lasts the compositor moves it to the
window beui finds under the pointer (`over` on each `Forwarded::Pointer`). Only a shown window can be dropped on: a
dock tab does not switch under a drag, and the drag's icon is not drawn (BE3-296).
