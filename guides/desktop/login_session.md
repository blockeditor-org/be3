# The Linux login session

`BEUI_PLATFORM=drm be-host --desktop` runs be-host on the displays and input devices themselves
(guides/desktop/linux.md). This guide is what only that mode does. Most of it lives in be-host's
`src/login_session/` and in `crates/beui-platform-drm`.

Ways out: Quick settings' Log out; Ctrl+Alt+Backspace quits at once, as does holding four fingers
on a touchscreen for three seconds; Ctrl+Alt+F<n> switches terminals.

## Installing it

    ./scripts/buck run //crates/be-host:install-login-session [-- PREFIX]

installs be-host with its apps in `PREFIX/lib/be-host` (`/usr/local` by default), links
`PREFIX/bin/be-host`, and writes `/usr/share/wayland-sessions/be-host.desktop` (from
`src/login_session/be-host.desktop`) whatever the prefix, since LightDM's slick-greeter never reads
`/usr/local/share`. It also adds the entry to a LightDM `sessions-directory` that leaves that one
out, a line to `/etc/greetd/environments` when that file exists, `/etc/pam.d/be-host` unless
one is there, and the screenshot portal's `be-host.portal` and `be-host-portals.conf` under
`/usr/share/xdg-desktop-portal` (guides/desktop/linux_services.md, Screenshots). `PREFIX/bin/be-host --uninstall-login-session [PREFIX]` removes exactly what
`PREFIX/lib/be-host/installed.json` records, leaving anything someone changed since.

- It installs exactly the files `buck/program/stage.sh` listed in `program-files.txt`, and refuses
  a directory without that list or `apps.json`.
- `src/login_session/install.rs` plans every change and `install/changes.rs` checks each
  destination can be written before anything is. Without root it reruns under sudo; with a prefix
  of one's own (`~/.local`) only the files outside it go through sudo
  (`--apply-login-session-changes`, reading them on stdin). Nothing in `$HOME` is written as root.
  A failed install leaves nothing behind; a repeat install swaps a staged `lib/be-host.new` in.

## The log and problems

In a login session stdout and stderr (every `eprintln!`, panic backtrace and the output of programs
be-host starts) go to `$XDG_STATE_HOME/be-host/login-session.log` (the directory is the app id in
lower case; `be-host-dev` without bundle metadata). `login_session/log.rs` (`Cap`) keeps it under
4 MiB while keeping the session's start and latest output in it, and keeps the previous session's
log as `login-session.previous.log`. The error that ends a session is also written to the original
stderr, for the journal under systemd or greetd.

Problems a person should see show as error toasts the host draws itself, top right, before and over
any shell: `beui_platform_drm::Problems` (displays, input), `Compositor::on_failure` (a program
that could not run), all through `notices::report`; `notices::report_with` adds a Copy report.

## Panics

be-host-base's `panic_guard` sorts panics by where they happen. Inside `panic_guard::contain` a
panic stops only that app, the shell included (guides/desktop.md, A shell that stops), while the
compositor and its clients keep running; a D-Bus task ends alone with a toast; a password check
that panics fails. Any other panic is be-host's own: in a window it shows its error screen, and in a
login session it ends the session through `Session::end`, so the console and display manager come
back instead of a frozen screen.

## Screens turning off

After the display settings' "Turn off screens after", `Windows::idle()` turns true and be-host
hands it to `DisplayControl::set_blanked`, which turns the CRTCs off. The compositor counts idle
time on the frame clock from frames that carried input (`Document::input_frames`), held back by a
shown window's `zwp_idle_inhibitor_v1`, and answers `ext_idle_notifier_v1` the same way.

The input that wakes the screens reaches nothing: the platform's `WakeGate` (`wake.rs`) drops it,
everything for `GRACE_USEC` after, and the releases of what it dropped, while releases of presses
delivered before still arrive so no key sticks. The platform reports the wake through
`DisplayControl::take_woken` and be-host passes it to `Compositor::woke`.

## Switching terminals

Whatever runs on the other terminal reprograms CRTCs, connectors and planes, so nothing the
platform cached survives. `recovery.rs` drops every output while the device is paused, resets the
device to everything off on return, and builds outputs again for what is connected, so the first
frame is a full modeset. A frame that cannot be shown rebuilds its output once; a second failure
stops that output and is reported once through `Problems`.

## Power and logging out

The power row sends `power::request(PowerAction)`; `src/login_session/power.rs` (`Power`, kept on
Linux whether or not in a login session) decides, and `logind.rs` talks to `org.freedesktop.login1`.

Log out, restart, power off, and closing be-host's window while programs are open (which in a login
session is logging out) start an *ending*: `PowerAvailability::ending` is published first, and only
then are programs asked to close, so the shell keeps a placeholder for each (guides/desktop.md,
Program windows). There is no time limit. The desktop app's ending bar answers with
`power::answer_ending`: `Cancel` leaves the remaining programs open, `EndNow` ends with them open.
Once logind has been asked to restart or power off, the ending cannot be cancelled; a refusal ends
it as Cancel does. `Session::end` then drops be-host, the outputs, input, the DRM device (restoring
the console) and the seat, in that order.

## The lock screen

Under `--desktop` the screen locks on Super+L, the power row's Lock, after the display settings'
"Lock the screen after", and in a login session on logind's `Lock` and before every suspend:
`LoginSession` holds a logind `delay` inhibitor and releases it once the lock has been drawn twice
(`src/login_session/sleep.rs`). `lock.rs` is the state machine, `ScreenLock` runs it, and
`src/ui/lock.rs`'s `LockScreen` is a `Modal` at `LayerLevel::Lock` in every display's window.
logind's `Unlock` is not obeyed; only a password unlocks.

- **The password** is checked by PAM (`pam.rs`, libpam `dlopen`ed on a thread) with the service
  `be-host` when `/etc/pam.d/be-host` exists, else `login`. Only `Verdict::Accepted` for the attempt
  in flight unlocks; every error leaves it locked. Attempts after the third wait, doubling up to a
  minute. Under `:dev` the VM's user needs a password (`passwd`) to try it.
- **Input** behind the lock: the lock overlay holds the focus and pointer, so no window or app is
  forwarded anything, and host and window key listeners hear only media keys. `Compositor::set_locked`
  lets go of held buttons and popups. Idle inhibitors are ignored while locked.
  `ScreenLockedWatch` hides notification toasts. `ext-session-lock-v1` is not implemented.

## Rotation and pointing devices

A screen's rotation (`ScreenSettings::rotation`, `DisplayConfig::rotations`) turns its picture
clockwise on the panel. The platform's `Screen` lays out and draws upright at the turned size
(`Rotation::turned`), so beui and Wayland clients only see an upright screen with
`Transform::Normal`, and turns it onto the scanout buffer on the GPU (`gpu.rs`, `rotation.rs`). It
never asks the plane to rotate, which most display devices, the PineNote's included, cannot do. A
rotation applies live and is never asked to be kept. `be_sdk_beui::RotationPicker` is the picker
for both the Display page and the Status bar's rotate button; both write through
`LayeredSettings::this_device`.

Absolute input (touchscreens, tablets) arrives in the panel's orientation and is turned back
(`layout::absolute`) onto the display its device belongs to. `pointing.rs` picks that display:
Settings > Input's choice (`InputSettings::displays`), else udev's `WL_OUTPUT`, else the only
display, else the one whose physical size matches the device's, else the built-in panel for a
touchscreen; a tablet with no screen spans every display. The host reports its own pick as
`HostInputDevice::pointing` (shown as Automatic).

## Input remaps

In a login session keys, mouse buttons and wheel notches can be remapped before anything sees them,
by a *remapper*: a WebAssembly module an app ships as data, given a config the app's editor writes.

- **Settings.** A remapper is a block of the `InputRemapper` type (`be_block::InputRemapper`),
  whose content is an envelope the host reads without the app: app id, module path in the app's
  data, and opaque config bytes. The input settings' `remapper` (`RemapperChoice`) picks one, shared
  or per profile. Any app can offer a remapper by declaring an editor for that type.
- **The shipped one** is the settings app's Input Remaps editor (`be-app-settings`, `input_remaps`):
  TOML config stored exactly as written, edited in place through `be_host_remap::edit`
  (`toml_edit`). Its editor shows rules as key caps and never needs a key name typed: keys come
  from a picker (Press it, through `InputCaptureWatch`; Mouse; All keys, listed from
  `be_host_remap::names`), with a typed name or code, read as the TOML reads it, as the fallback.
  A program group is picked from `OpenProgramsWatch` and `ProgramsWatch`; the TOML itself is
  behind an "Edit as TOML" switch. Its module is `crates/be-host-remap-module` (staged as `remapper.wasm`), which
  runs `be-host-remap`'s engine and text reader (`be_host_remap::text`, key names in
  `be_host_remap::names`). Rules can be scoped to a window by executable, program id or title.
- **The protocol.** A module exports `memory`, `be_remap_buffer` and `be_remap_handle` and trades
  bincode `be_host_remap::protocol` messages. `be-host-remap-host` runs it on wasmi with fuel and a
  memory cap.
- **Failing open.** On any failure the host releases everything the module held, passes the input
  through, toasts, and restarts the module on the next input; after three failures in a minute, or
  an unreadable config, it stays off until the remapper changes. Ctrl+Alt+Backspace, Ctrl+Alt+F<n>,
  Super+L and Super+Escape never reach the module, and nothing does while the screen is locked, so a
  module can neither trap the session nor read the password.
- **In the host.** be-host-desktop's `remap/linux.rs` fetches the module (only from an app with an
  `InputRemapper` editor), reconfigures it when only the config changes, and caches the last module
  and config in its database so remapping works from the first input. The DRM platform passes input
  through the `InputFilter` on `InputControl` after the `WakeGate`, delivers its output as if
  libinput had sent it, and `InputControl::inject` carries changes made outside input (releases on
  focus or lock). `Compositor::keyboard_window` names the focused window; its executable is the last
  part of its process's first argument (a Windows path for a Proton game). `RemapsWatch` reports the
  state to Settings, and is answered only where the filter runs.
- **Catching a chord.** A remap editor learns which key, button or wheel turn to remap by opening
  an `InputCaptureWatch` (with `Editor::watch_while`): the session catches input ahead of the
  remapper, by Linux input code, until the first chord, and keeps it from programs. The rules are
  `be_host_remap_host::capture`'s, and the WIT doc comment; `remap/linux.rs` starts a capture at
  each opening (`watches::openings`) and stops it once no watch is open. A new watch is never told
  the chord an earlier one caught (`be_sdk_api::starts_anew`).
