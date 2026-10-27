# The Windows host

`--desktop` on Windows shows the person's Windows monitors and windows inside the desktop shell
(guides/desktop.md). XR is the main target: each shown entry is a panel of its own, as program
windows are where `opens_windows()` is true. A windowed `--desktop` is a development view: there
is no virtual display, so a focused window comes up on the real monitor above block-app.
`--session` and `--install-session` are refused (`src/arguments.rs`).

All of it is compile-checked only: `buck/dev/verify.bxl`'s `_CROSS_CHECKS` checks be-win32,
be-win32-capture, be-win32-input, beui and block-app for `windows_x86_64`, and the logic behind
the Win32 calls is plain Rust tested on Linux with fakes. Running it needs a real Windows
machine; Wine has no Windows.Graphics.Capture.

## The crates

- `crates/be-win32`: the windows and the session. `Watcher::spawn(wake)` runs a thread of its own
  (per-monitor DPI aware, with COM) with `SetWinEventHook` (out of context, no low-level hooks),
  a hidden window for display changes and a 1 s tick for "not responding". `Model` folds what it
  reads into a `Snapshot`: entries with stable `EntryId`s, the displays first (one per monitor,
  sized as the monitor and titled with its name), then the top-level windows Alt+Tab would show
  (with owner, dialog, icon, title, app id, frame bounds, minimized and maximized, popups,
  `Control::Elevated` for a process of higher integrity), the monitors, and the input desktop (`secure_desktop()`). It
  keeps each entry's `shown`, so a restarted shell brings back what was shown. It also lists
  the Start Menu's `.lnk` programs, launches them (`launch`, `exec`, which block in
  `ShellExecuteExW`) and matches their windows by process id (`Watcher::launched`), and offers
  lock, suspend, restart, power off and log out, and closes, resizes and maximizes windows
  (`close_window`, `resize_window`, `set_maximized`). `CaptureExclusion::start()`, on block-app's
  UI thread, sets `WDA_EXCLUDEFROMCAPTURE` on that thread's top-level windows now and as they
  appear (a WinEvent hook on that thread), so captures never show block-app.
- Threads: no thread of ours that the UI thread may wait on (the watcher, the captures) sends a
  message to one of our own windows, since the UI thread answers only between frames. The watcher
  reads no title or icon (`WM_GETTEXT`, `WM_GETICON`) of our own process's windows (`Desktop::title`,
  `icon` and `app_user_model_id` are only asked for other processes' windows), and moves windows
  only with `SWP_ASYNCWINDOWPOS` and `ShowWindowAsync`. The UI thread never waits on the watcher:
  requests are queued, and answers (`set_hotkey`'s `FocusEvent::HotkeySet`/`HotkeyRefused`) come
  back as focus events with the wake. `Watcher::spawn` and `Capture::start` wait only for their
  threads to set up, which needs nothing of the UI thread.
- `crates/be-win32-capture`: Windows.Graphics.Capture of one monitor or window, its newest frame
  handed out as a `wgpu::Texture` on wgpu's Vulkan device (shared D3D11 texture and fence,
  imported with `VK_KHR_external_memory_win32` and `VK_KHR_external_semaphore_win32`; the
  device extensions are `DEVICE_EXTENSIONS`, which block-app's Windows `run_shell` opens wgpu
  with). One `Device` per wgpu device; a `Capture` per entry; the border is off and the cursor
  is captured only while `set_cursor(true)`.
- `crates/be-win32-input`: `inject` puts pointer, key and text events into an entry at a point
  of its panel (`SendInput` in absolute virtual-desktop coordinates), after
  `be_win32::bring_forward`. It refuses an elevated window and the secure desktop from
  be-win32's snapshot (`target`), the one place either is decided.

## In block-app

`crates/block-app/src/desktop_host.rs` is the host facade; on Windows it is `desktop_host/win32.rs`, with
the mapping to `be3:desktop`'s types in `desktop_host/win32_map.rs` (tested on Linux).
`plugin_host/published.rs` offers `windows`, `programs` and `power` there; the other interfaces,
`displays` among them (it describes the displays the shell is drawn on, and on Windows that is
block-app's one window), stay unsupported, so the shell hides what they back.

- The watcher starts when the desktop shell is first shown (`set_owned(true)`), with block-app's
  window (`beui::NativeWindow`) as the host window and `Hotkey::DEFAULT` (Ctrl+Alt+Space)
  registered; a hotkey that cannot be registered is a toast. The host prints one line to stderr
  for each rare event (the watcher's first listing, the hotkey, the capture device's adapter,
  each capture started, lost or stopped, focus mode starting and ending) and for every failure,
  toast or not, since nothing else shows what a first run on a new machine did.
- `WindowsWatch` is the snapshot's entries (`kind`, `shown`); `show` and `hide` set `shown` in
  be-win32's model. `WindowSurface` starts a capture of its entry when it is mounted, which the
  shells do only for shown entries, and stops it when unmounted; a display entry draws its
  monitor's capture. Each capture's notify wakes the UI, and `before` hands the newest `Frame`
  to every surface showing it (`desktop_host/texture_draw.rs` draws it and keeps the `Frame` alive
  while drawn). The frames' format follows the surface (srgb or not). A capture that reports
  `Lost::Failed` is started again once; failing again is a toast (`win32_map::recovery`).
  `Lost::Closed` just ends it.
- Popups: be-win32 gives each entry its live popups (menus, tooltips: owned or same-thread
  `WS_POPUP` windows), bottom first, with their frames. `WindowSurface` captures each popup's
  HWND and draws it as a `Window` of role `WindowRole::Popup` placed `Over` the window the panel
  is in, at its offset from the toplevel's frame in the panel's scale (`win32_map::popup_rect`),
  as the Linux host shows a toplevel's popups (guides/desktop.md, Program windows). They come
  and go with the snapshot; where windows are hosted (space) a popup is opened again when its
  corner moves. A press on one focuses its toplevel with the cursor at that point
  (`popup_point`); controllers do not inject into popups. Focus mode's clip already covers them.
- Sizes (`desktop_host/win32_shape.rs`, `Shaping`, tested on Linux), as guides/desktop/linux.md
  does for floating windows: `HostWindow::size` is the size the window last chose itself (its
  frame bounds in its monitor's points), not one it took because block-app asked, so a window
  shown at its own size keeps following its own size. When a panel's placed size differs from
  where the window is heading (by more than 2 px), because the person resized the panel or docked
  it, the host asks for it: `be_win32::resize_window` (frame size in physical pixels, the panel's
  points times the monitor's scale; it adds the invisible borders, the window rect less the DWM
  frame bounds, and calls `SetWindowPos` with `SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE |
  SWP_ASYNCWINDOWPOS` on a per-monitor-aware thread context). The last few sizes asked are kept,
  and a frame matching one is that answer; any other frame is the window's own choice and clears
  them. Displays, minimized, maximized, fullscreen and elevated windows are never resized.
- Fullscreen: Windows has no request for another program's window to go fullscreen, so the
  `fullscreen` action maximizes it (`be_win32::set_maximized`, `ShowWindowAsync`, which also
  activates it) and leaving restores it. `HostWindow::fullscreen` is the area the shell is shown
  in (the `rect` `before` is given, block-app's window, as Linux reports in a window) for a window
  the shell maximized while it stays maximized (restoring it by hand ends it) and for any window
  whose frame covers a whole monitor (a game, F11 in a browser); the shell then fills that area
  with its panel.
- Focus mode is the mouse and keyboard path: a press on an entry's panel, or the `focus` window
  action (the picker), calls `Watcher::focus(id, point)` at once, which raises the real window,
  makes it the foreground, moves the cursor in and clips it there, so the physical mouse and
  keyboard drive it natively, elevated windows included. The focused entry's capture shows the
  cursor, and its panel says which hotkey comes back; the hotkey, or block-app coming forward,
  ends focus. A refused focus (`FocusError`) is a notice over the entry until it is focused.
- XR controller presses (`PointerId::is_controller`) are injected instead (press, release and
  drags), since a laser has no physical cursor to clip.
- Notices are drawn by the host, never left silent: over an elevated window an injected press
  could not reach, "run Block as administrator"; over every display entry while the secure
  desktop is up, and as a toast when it comes up, "take off the headset to answer it"; a toast
  when Windows refuses to bring block-app back.

## Not there yet

Opening a block's file in a Windows program, packaged (Store) apps, injecting controller presses
into popups, the tray,
notifications, media, Wi-Fi, input devices and the lock screen (Explorer owns them). The deferred
input plans are in plans/windows_input.md.
