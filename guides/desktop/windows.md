# The Windows host

`--desktop` on Windows shows the person's Windows monitors and windows inside the desktop shell
(guides/desktop.md). XR is the main target: each shown entry is a panel of its own. A windowed
`--desktop` is a development view with no virtual display, so a focused window comes up on the
real monitor above be-host. `BEUI_PLATFORM=drm` and `--install-login-session` are refused.

Nothing in verify or CI compiles it: after changing it, check it by hand with `./scripts/buck
build '//crates/be-host-win32:be-host-win32[check]' --target-platforms
//buck/platforms:windows_x86_64`, and the same for be-host-win32-capture, be-host-win32-input,
beui-host and be-host. The logic behind
the Win32 calls is plain Rust tested on Linux with fakes; put new logic there. Running it needs a
real Windows machine (Wine has no Windows.Graphics.Capture).

Not there yet: opening a block's file in a Windows program, packaged (Store) apps, controller
presses into popups, the tray, notifications, media, Wi-Fi, input devices and the lock screen
(Explorer owns them). Deferred input plans: plans/deferred/windows_input.md.

## The crates

- `crates/be-host-win32`: the windows and the session. `Watcher::spawn` runs a thread of its own
  (per-monitor DPI aware, COM, `SetWinEventHook` out of context, no low-level hooks). `Model` folds
  what it reads into a `Snapshot`: entries with stable `EntryId`s, displays first (one per
  monitor), then the windows Alt+Tab would show (owner, icon, title, program id, frame bounds,
  popups, `Control::Elevated`), the monitors, and whether the secure desktop is up. It also lists
  and launches Start Menu `.lnk` programs, matches their windows by process id
  (`Watcher::launched`), offers power actions, and closes, resizes and maximizes windows. The
  watcher's threading rules are on `Watcher`: break them and the UI thread deadlocks.
- A typed line: the host names `cmd.exe /D /C` as its shell (`ProgramCatalog::shell`), but
  `Win32::exec` runs a command that starts with it as the Run dialog does, never through cmd:
  the line is split into arguments and its first is shell-executed with the rest as parameters,
  so App Paths (`chrome`), documents and URLs open, no console shows, and the launch tracks the
  program's own process.
- `crates/be-host-win32-capture`: Windows.Graphics.Capture of a monitor or window, its newest frame
  a `wgpu::Texture` on wgpu's Vulkan device through a shared D3D11 texture and fence. be-host's
  `RunOptions::device_extensions` adds the capture's `DEVICE_EXTENSIONS` on Windows. One `Device` per
  wgpu device, one `Capture` per entry.
- `crates/be-host-win32-input`: `inject` puts pointer, key and text events into an entry with
  `SendInput`; `reach::target` refuses elevated windows and the secure desktop.

In be-host-desktop, `desktop_host/win32.rs` (`Win32`) is the `DesktopHost`, with the mapping to
`be3:desktop` in `win32_map.rs`, sizes in `win32_shape.rs` and content overlays in
`win32_overlay.rs`, all tested on Linux. Only `windows`, `programs` and `power` are published;
`displays` stays unsupported, since the shell's display is be-host's one window.

## How entries show

- The watcher starts when the desktop shell is first shown (`DesktopHost::configure` with
  `owned` set) and registers `Hotkey::DEFAULT` (Ctrl+Alt+Space). The host prints a line to stderr for each rare event and every
  failure, since nothing else shows what a first run on a new machine did.
- `WindowsWatch` is the snapshot's entries; `win32_map::known_window` makes the rest of each
  window's status. A window's name is its Start menu program's (`win32_map::ProgramNames`): the one
  it was launched from, else the shortcut whose AppUserModelID or target matches its program id or
  command, known once the Apps panel has listed programs. The shell opens a tab for every window
  and a hidden one for every display.
- `WindowSurface` starts a capture when mounted, which happens only for shown entries, and stops it
  when unmounted. `before` hands the newest `Frame` to each surface. A capture `Lost::Failed`
  restarts once, then toasts (`win32_map::recovery`).
- Popups (owned or same-thread `WS_POPUP` windows) are captured and drawn as `WindowRole::Popup`
  windows over the panel (`win32_map::popup_rect`), as on Linux. A press on one focuses its
  toplevel at that point.
- Owned windows are dialogs: the owner is `host-window.parent`, so the shells draw the dock's
  anchored card.
- Captures include be-host's own window (decisions/windows_captures_show_be_host_itself.md).

**Sizes.** `KnownWindow::size` (the child's `intrinsic`) is the size the window last chose itself.
`WindowSurface` draws the capture top-left at the window's frame size (`win32_map::drawn_size`) and,
when the panel and frame disagree by more than 2 px, asks `be_host_win32::resize_window` for the
panel's size. `Shaping` remembers the last few sizes asked, so a frame matching one is an answer and
any other is the window's own choice. Displays, minimized, maximized, fullscreen and elevated
windows are never resized. Windows has no least size and no suspended state.

**Fullscreen.** There is no way to make another program's window fullscreen, so the `fullscreen`
action maximizes it (`set_maximized`). `KnownWindow::fullscreen` is set for a window the shell
maximized while it stays maximized, and for any window covering a whole monitor (a game, F11).

## Focus mode

Captures can't take real input, so the mouse and keyboard reach a window by handing it the real
cursor. Only a click on an entry's content starts it: a press and release that moved less than
`win32_map::PRESS_SLOP` (`ContentPress`). `Watcher::focus(id, point)` raises the window and makes
it the foreground; once Windows reports it in front (`Focused::arrived`), the cursor moves to the
release point and is clipped there, so the physical devices drive it natively, elevated windows
included.

- Nothing is clipped before it arrives: winit resets any clip it did not set while be-host's window
  is active, and a window that never arrives must not trap the cursor. `FocusMode::reclip` puts the
  clip back on the events that drop it.
- The cursor is clipped only while the hotkey is registered (`FocusMode::set_hotkey`). Without it
  (still registering, or another program holds it) the window is foregrounded but the mouse stays
  free, and the panel says Alt+Tab comes back.
- Ways back, each ending focus and unclipping (`FocusMode::returns`): the hotkey, or be-host
  becoming the foreground by any means. A window that has not arrived after `ARRIVAL_TICKS` ends it
  as `NeverArrived`.
- On a display entry, the release point on the monitor (`focus::display_point`) is looked up with
  `WindowFromPoint`; if it is one of be-host's own windows (its mirror in the capture) nothing
  happens (`FocusError::OnHost`).
- The shell's `focus` window action does nothing on Windows (`win32_map::shell_focus`;
  decisions/windows_shell_focus_does_not_start_focus_mode.md).
- XR controller presses (`PointerId::is_controller`) are injected instead, since a laser has no
  physical cursor.

**What the person is told.** The entry's `KnownWindow::note` (the child's `notice`, shown in the
tab title, never over content; `win32_map::Notes`) says the way back while focused, a refusal until
next focused, or "Click to use this window" on hover. Each change bumps `revision()`, so
`WindowsWatch` is sent again. Only what must cover content is drawn on it (`EntryOverlay`): the
off-screen parts of a frame (`win32_map::off_screen`; the host does not move the window) and the
secure-desktop notice. Every toast is also a line on stderr.
