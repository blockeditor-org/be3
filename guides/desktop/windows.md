# The Windows host

Today `--desktop` on Windows is the desktop shell alone (guides/desktop.md): block-app's
`wayland` module (`crates/block-app/src/wayland.rs`) uses `wayland/unsupported.rs` there, and
`be3:desktop` is not published (`plugin_host/published.rs`), so every watch is answered
`unsupported` and the shell shows none of the controls they back. `--session` and
`--install-session` are refused (`src/arguments.rs`).

The Windows host is being added, in three crates of its own, so the logic can be tested on Linux
with fakes behind small traits and only the Win32 calls are Windows-only:

- `crates/be-win32`: the windows (`EnumWindows`, and `SetWinEventHook` on a thread of its own),
  filtered as Alt+Tab does, with their owners, icons, titles, app ids and frame bounds, folded
  into a list of entries with stable ids and a revision; monitors; Start Menu programs; power;
  and detecting elevated windows and the secure desktop.
- `crates/be-win32-capture`: a capture of one monitor or window through
  Windows.Graphics.Capture, its latest frame handed over as a `wgpu::Texture` on wgpu's Vulkan
  device without leaving the GPU.
- `crates/be-win32-input`: low-level input hooks, injecting pointer, keys and text into a
  window or monitor, and mapping points in an entry's rect to virtual-desktop coordinates.

Windows code is compile-checked on Linux: `buck/dev/verify.bxl`'s `_CROSS_CHECKS` lists the
targets `./scripts/verify` checks for `windows_x86_64`. Running it needs a real Windows machine;
Wine has no Windows.Graphics.Capture.

## The design

- XR is the main target: each shown entry is a panel of its own, as program windows are where
  `opens_windows()` is true. A windowed `--desktop` is a development view: with no virtual
  display, injected input raises real windows above block-app on the real monitor.
- Nothing opens by itself. The host lists every candidate, monitors (a `display` entry, sized
  as the monitor and titled with its name) and top-level windows, in `be3:desktop`'s `windows`
  interface, each with `shown`, which the host keeps so a restarted shell brings back what was
  shown. The person picks what to show from the shell's picker (`show` and `hide` window
  actions); hiding stops an entry's capture, and `close` closes the real window.
- Capture: whole monitors first, then single windows, both through Windows.Graphics.Capture
  (`GraphicsCaptureItem` from an HMONITOR or HWND), never DXGI desktop duplication, with the
  border and the cursor left out (the shell draws its own). Frames are BGRA8 D3D11 textures,
  copied into a texture shared with an NT handle and a shared fence, imported into Vulkan
  (`VK_KHR_external_memory_win32`, `VK_KHR_external_semaphore_win32`) and wrapped as a
  `wgpu::Texture` through wgpu-hal, as crates/be-dmabuf does for dmabufs. The D3D11 device is on
  wgpu's adapter (matched by LUID). block-app's own window is left out of capture
  (`WDA_EXCLUDEFROMCAPTURE`).
- Input has one path: block-app never relies on being the foreground window. Low-level hooks
  (`WH_MOUSE_LL`, `WH_KEYBOARD_LL`) on a thread with its own message loop, which must never block,
  swallow physical input while the shell owns it and hand it to beui, which routes it as
  anywhere. When it reaches a Windows entry the host raises that window and injects it
  (`SendInput` in absolute virtual-desktop coordinates, `KEYEVENTF_UNICODE` for text no key
  types).
- What the host cannot reach is shown, never silently ignored: over a window whose process runs
  at a higher integrity level (UIPI keeps hooks and injection out) the host draws a notice to run
  Block as administrator, and while the secure desktop is up (a UAC prompt, Ctrl+Alt+Del) it
  shows a notice over every display entry and a toast asking the person to take off the headset
  and answer it.
- Programs are the Start Menu's `.lnk` files (per-user and all-users), resolved with
  `IShellLinkW`, launched with `ShellExecuteExW` and matched to their windows by process id.
- `displays` comes from `EnumDisplayMonitors` and `GetMonitorInfoW` (`GetDpiForMonitor` for the
  scale), read-only. `power` offers lock, suspend, restart, power off and log out. Explorer owns
  the tray, notifications, media and Wi-Fi, so those stay unsupported and the shell hides them.
