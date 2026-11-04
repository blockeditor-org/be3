# The Linux desktop's D-Bus services

be-host-services (`crates/be-host-services`) does what the desktop app cannot do from wasm: it
talks to D-Bus and the sound server and hands the results to the shell through `be3:desktop`
watches (guides/desktop.md). The desktop app (`crates/be-app-desktop/src/root/`) decides
everything a person sees. Notifications, the tray, media players and the session environment use
the session bus; Wi-Fi, the backlight and logind use the system bus.

## D-Bus

Every D-Bus conversation runs on one thread (`src/dbus.rs`); `dbus::spawn` runs a future there,
and a task hands results back through a `host::mailbox`, so the UI thread never waits on the bus.

- `dbus::SYSTEM` and `dbus::SESSION` are `dbus::Bus`es that share one connection and open a new
  one once it closes. Either bus can go away and come back while be-host runs, so every service
  runs once per connection: `bus.connected()` waits for one, and `dbus::until_closed(&connection,
  service)` runs the service until it ends or the connection closes. A service returns
  `ControlFlow::Break` only when no one wants it any more, and `Continue` when it cannot work on
  this connection (another notification server owns the name).
- State that outlives a connection (request channels, notification ids, logind's sleep inhibitor)
  lives outside the per-connection future; what a service showed from the bus is cleared by a guard
  dropped with it.
- Build every connection with `dbus::build`. It sets `METHOD_TIMEOUT` (a peer that never answers
  fails the call instead of hanging the task), and serves `ObjectManager` at `/` while connecting,
  because zbus drops method calls that arrive before its object server is listening, which loses
  a client's first call to a name served right after connecting.
- Calls to programs that can be slow (tray items) run in their own task; media keys are sent
  without waiting for a reply.

Tests run services on `dbus::test_bus::PrivateBus` (the test runtime's `dbus-daemon`,
`BE3_DBUS_DAEMON`), whose `kill` and `start_again` check that services come back, and wait with
`test_bus::next` and `test_bus::within` (30 s, then fail).

To try any service under `:dev`, start a session bus of its own and run `:dev` and every client
with it (`dbus-run-session` does not suit `:dev`, whose launcher returns at once):

    dbus-daemon --session --fork --print-address
    DBUS_SESSION_BUS_ADDRESS=ADDRESS ./scripts/buck run //crates/be-host:dev -- --desktop

## Notifications

Under `--desktop` the host serves `org.freedesktop.Notifications` (`src/notifications/server.rs`),
without replacing an existing owner. The host only owns the name, reads hints, loads the picture
(image data, image path, app icon through be-host-wayland's `IconThemes`, desktop entry icon; at
most 128 px) and answers `Notify` at once with its own id (`notifications/inbox.rs`). Requests
wait in `NotificationsWatch` (a `NotificationInbox`) until the shell's `NotificationReport` says it
took them; the report carries the signals to send and the ids the desktop still holds.

Everything else is the desktop app's `root/notifications/`: `center.rs` is the behaviour and where
to test it (replacement, timeouts, actions, `resident`/`transient`, the cap of 100, close
reasons); `markup.rs` turns the body to plain text. A toast whose time is up only hides; the
notification stays listed until dismissed or closed. One with nothing to show is kept but never
toasted or listed.

    DBUS_SESSION_BUS_ADDRESS=ADDRESS gdbus monitor --session -d org.freedesktop.Notifications
    DBUS_SESSION_BUS_ADDRESS=ADDRESS notify-send --action=default=Open Mail 'Lunch at noon?'

Start the monitor first so it sees the signals; `notify-send` (package `libnotify-bin`) with an
action waits until the notification closes.

## The tray

The host serves `org.kde.StatusNotifierWatcher` and is its `StatusNotifierHost` (`src/tray/`);
under another desktop that owns the watcher, it registers with that one as a host.

- `client.rs` is one task every change reaches as an `Event` (registrations, `NameOwnerChanged`,
  the items' `New*` signals, dbusmenu's `LayoutUpdated` and `ItemsPropertiesUpdated`). Reads run in
  tasks of their own, ticketed so a slow answer never replaces a newer one.
- `icons.rs` picks the picture; `model.rs` is the pure part and where to test parsing (pixmaps are
  ARGB in network order; hidden menu entries dropped, `_` mnemonics removed).
- WIT has no recursive types, so a menu crosses flat as `TrayMenuItem`s with depths, and the shell
  nests it with `HostTrayItem::entries` (be-sdk-api's `nesting.rs`, capped at `MAX_MENU_DEPTH`).
- Many items, every libappindicator one among them (Discord, Steam, Nextcloud), have no
  `Activate`. As Plasma and waybar do, the host publishes such an item as `item_is_menu` when its
  introspection lists no `Activate` or an `Activate` fails with `UnknownMethod`, and the shell opens
  the menu when an item it just activated turns `item_is_menu`, so the first click shows it.
- Click points (`TrayClick`) are desktop points, which are also X11 screen coordinates.

The desktop app's `root/tray.rs` shows every item that is not `Passive` as a `MenuButton`. To try
it, run an indicator program against the bus with the compositor's socket, such as a Python script
using `gir1.2-ayatanaappindicator3-0.1`: `DBUS_SESSION_BUS_ADDRESS=ADDRESS drive exec python3
indicator.py`.

## Media and hardware keys

The desktop app binds volume, mute, mic mute, brightness and media transport keys (`BINDINGS` in
`root/media.rs`) as intercepting actions, so they work while a program has the keyboard and over
the lock. Each sends a `MediaRequest`; level keys show `LevelOsd` (`root/osd.rs`) on every display,
with the levels from `MediaWatch`.

be-host-services' `src/media.rs` starts its backends once an app watches levels or asks for
something:

- `media/audio.rs`: volume, mute, mic mute and the default output over the PulseAudio protocol
  (the `pulseaudio` crate's protocol layer, pure Rust), which pipewire-pulse serves. It subscribes
  to server events, so changes made elsewhere show. An unreachable server is logged once per error.
- `media/backlight.rs`: brightness through logind's `Session.SetBrightness`.
- `media/players.rs`: transport keys go to the MPRIS player that is playing (the latest started),
  else the one that played last, else the first listed.

To try them, run `pipewire`, `wireplumber` and `pipewire-pulse` with `XDG_RUNTIME_DIR` and
`DBUS_SESSION_BUS_ADDRESS` set for be-host, press `drive key volumeup` (or `micmute`, `play`). The
VM has no sound card: `pactl load-module module-null-sink sink_name=speakers
sink_properties=device.description=Speakers`.

## Wi-Fi

The status button and Quick settings show Wi-Fi while `WifiWatch` reports a device
(`WifiStatus::available`). The desktop app's `root/wifi.rs` is the network list: on/off, scan on
open, join or leave, password or WEP key prompts (asked again on `failed`), and "Other network…"
(`WifiRequest::JoinHidden`). Enterprise networks join only from a connection saved elsewhere.

be-host-services' `src/wifi/` talks to NetworkManager on the system bus (`network_manager.rs`),
rereading the device, access points and saved connections after every signal, so changes from
`nmcli` show at once. A saved network is joined with `ActivateConnection`, anything else with
`AddAndActivateConnection`; a new connection that fails is deleted again. `model.rs` is the pure
part and where to test it. `wifi_on_a_private_bus_lists_networks_and_connects` runs a fake
NetworkManager.

The VM has no Wi-Fi device. To see the UI, run `python3 -m dbusmock --system --template
networkmanager` on a `dbus-daemon` of your own, add a device and access points with its
`AddWiFiDevice` and `AddAccessPoint`, and start `:dev` with the same `DBUS_SYSTEM_BUS_ADDRESS`. The
mock does not signal properties it sets, so call its `EmitSignal` after flipping the switch.
