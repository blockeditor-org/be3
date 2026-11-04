# The Linux host

On Linux be-host hosts the desktop (guides/desktop.md) as a Wayland compositor, be-host-wayland,
with Xwayland for X11 programs. It runs two ways:

- In a window or headless: `be-host --desktop`, or `./scripts/buck run //crates/be-host:dev --
  --desktop`. Programs open inside the shell; what needs the displays and input devices themselves
  (display modes, pointer settings, power actions other than Lock, remaps) is absent.
- As a login session: `BEUI_PLATFORM=drm be-host --desktop` runs on the displays and input devices
  through `beui-platform-drm`, from a virtual terminal with no other display server.
  `--install-login-session` installs it for display managers. Without `--desktop` it runs the
  workspace shell on the displays: log, logind, power, lock screen and panic handling still work,
  but there are no program windows, notifications or tray. be-host refuses `drm` on other systems
  (`src/arguments.rs`). guides/pinenote.md covers an e-paper tablet.

Without `--desktop` be-host runs no Wayland server or Xwayland at all.

The parts:

- guides/desktop/wayland.md: the compositor: program windows, input, the keyboard and on-screen
  keyboard, cursors, frame pacing, X11 programs, clipboard and drag and drop.
- guides/desktop/login_session.md: running on the displays: installing, the log, panics, screens
  turning off, switching terminals, power and logging out, the lock screen, rotation and pointing
  devices, input remaps.
- guides/desktop/linux_services.md: the D-Bus services: notifications, the tray, media and hardware
  keys, Wi-Fi.

## Where the code is

- `crates/be-host-wayland`: the compositor (smithay). `Compositor` is its face to be-host;
  `state.rs` the protocol handlers; `windows.rs` the window list (`Windows`, `WindowInfo`);
  `view.rs` the `WindowView`; `compositor/clipboard.rs` the clipboard; `xwayland.rs` the X11 window manager; `programs.rs` the `.desktop`
  entries, icons and MIME globs.
- `crates/be-host-desktop`: `desktop_host/linux.rs` implements `DesktopHost` over the compositor;
  `programs.rs` lists programs as block editors; `remap/linux.rs` holds the input remapper.
- `crates/be-host-services`: everything on D-Bus (guides/desktop/linux_services.md).
- `crates/be-host`: `src/compositor/` wires the compositor into the host's views
  (`desktop.rs` window placement, `editors.rs` window children, `intercept.rs` global keys);
  `src/login_session/` is install, logind, power, sleep and the lock;
  `src/display/` the displays.
- `crates/beui-platform-drm`: the DRM/KMS and libinput platform (guides/beui/platforms.md).

## Displays

In a login session `beui-platform-drm` shows one window per display (guides/beui/platforms.md,
Displays under beui-platform-drm), which be-host opens from the platform's `Displays` handle (`display/linux.rs`
offers them to `display/shown.rs`; `ui::Displays` opens a window per display). The first display's
window holds the host's whole UI (`ui::Root`); every display's window holds its own lock screen and
display-settings prompt (`DisplayChrome`). Test ids repeat across display windows, so drive them
with `drive click ID in WINDOW`. In a window or headless there is one display, be-host's window.

**Scale.** Each display's scale is the one chosen in Settings > Display
(`ScreenSettings::scale_percent`, keyed by `HostDisplay::id` like its mode and rotation, handed to
the platform as `DisplayConfig::scales`), else `HostDisplay::default_scale`: `BEUI_SCALE` when set,
else one from its pixel density (beui-platform-drm's `density.rs`).

**Arrangement.** With several displays, Settings > Display arranges them
(`display/arrangement.rs` in be-app-settings; `touching::settle` snaps them edge to edge) and sets
each display's use (part of the desktop, a mirror, or off) and the main display. These are kept per
device and reach the platform as `DisplayConfig`'s `uses`, `main` and `places`. A changed main
display or use must be kept like a new mode (`display/guard.rs`), or it reverts.

**Outputs.** be-host-wayland advertises one `wl_output` per display at its desktop position
(`Compositor::set_monitors`, `heads.rs`), with one mode and its scale rounded up for integer-scale
programs. Each window enters the output it mostly covers, and its preferred buffer scale and
`wp_fractional_scale_v1` scale follow it, so a program with fractional scaling and viewporter draws
exactly the pixels it is shown at. Xwayland binds the same outputs, so X coordinates are desktop
coordinates.

**Event loop.** In a login session the compositor's sources are inserted into the platform's
calloop loop (`beui_platform_drm::Loop`, `Compositor::join_loop`) and wake beui. Elsewhere the
platform's loop takes no file descriptors, so the "wayland watch" thread (`Server::watch`) polls
them.

## Files opened in programs

Under `--desktop` a program with a `.desktop` entry is an editor of every block whose type has a
plain-file form (guides/type_modules.md) and whose file name has an extension the entry's
`MimeType` names. So an EPUB file block opens in KOReader as a tab of whichever shell opened it.
MIME types become extensions through shared-mime-info's globs (be-host-wayland's `MimeGlobs`).

be-host-desktop's `programs.rs` adds one `EditorDescriptor` per such program to the catalog every
app gets (`Edits::Any`, with `extensions`). Its editor id (`programs::editor_id`) hashes the desktop
id, so a saved tab names the same program next time. be-sdk-ui's `BlockCatalog` lists it after the
type's own editors, matched by `BlockInfo::file_name`, and `own_editor` falls back to it for a type
with no editor of its own. Elsewhere no program is listed.

When a child view asks for a block in a program, `HostedChild` shows a `ProgramSurface` and
`programs::show` runs a session:

- **Write out.** `be_host_stack::plain_file` gets the file form and `programs/files.rs` writes it
  to `DATA/files/<block id>/<file name>`, only when the bytes differ, so what a program keeps beside
  the file (KOReader's `.sdr/` reading position) stays valid.
- **Launch.** `Programs::open_file` runs the entry with the path for `%f`/`%F`/`%u`/`%U`, tagged
  with the block, so its first toplevel is bound to the view (`DesktopHost::opening_windows`).
- **Hidden from the shell.** A bound window is left out of `WindowsWatch`
  (`programs::bound_windows`), so the desktop neither floats it nor keeps a placeholder for it.
- **Write back.** `programs/watch.rs` watches the folder; a changed file goes back into the block
  as a `from-file` commit. A file changed while nothing watched is taken back on the next open.
- **Close.** Closing the tab asks the window to close, so the program saves as usual.

A single-instance program that hands a second file to its running process (Krita) opens a window
that is not tagged, so it shows as an ordinary window.

To try it, give a program an entry in `/usr/local/share/applications` (`:dev` moves
`XDG_DATA_HOME`, so the user's own entries are not read), sync a file into a workspace with
`be WORKSPACE sync DIR` (guides/sync.md) and click it in the files.
