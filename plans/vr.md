# VR

Depends on: none.

What remains of the plan that took the space layer (`crates/beui-layer-space`,
`BEUI_LAYERS=space`; guides/beui/space.md covers it) from a
simulator on a flat screen to OpenXR headsets. The first headset is the
Bigscreen Beyond on Windows through SteamVR, driven with a mouse and keyboard
(no tracked controllers), then SteamVR and Monado on Linux. Items marked
**(ask)** need a decision from the user during the work.

guides/beui/space.md (the space layer, headsets and controller lasers) and
guides/desktop.md (program windows) describe what is built. The simulator
stays a first-class backend throughout: every space test keeps running without
a headset.

## OpenXR follow-ups

`BEUI_LAYERS=xr` shows space on a headset through `crates/beui-layer-xr` on Linux
and Windows (guides/beui/space.md covers headsets), checked in CI against Monado's null
compositor and compiled for Windows by `./scripts/verify`. What remains:

- **Hardware.** Untried on a real headset: SteamVR on Windows with the
  Bigscreen Beyond first, then SteamVR and Monado on Linux.
- **Multiview.** Each eye is its own pass and swapchain; one pass with
  wgpu's `multiview_mask` and `@builtin(view_index)` into a two-layer
  swapchain would halve the draw calls.
- **Waiting on hardware.** `BEUI_LAYERS=xr` waits for a runtime and
  headset (`Link`), checked only against Monado started late and stopped:
  whether SteamVR returns `FORM_FACTOR_UNAVAILABLE` with the Beyond
  unplugged, and quits with an instance loss rather than `EXITING`, is
  untried. Finding one reopens the GPU device as a lost GPU does, so
  whatever be-host keeps on the old device (app textures, dmabuf imports)
  must follow `App::renderer_replaced`.
- **Layers on hardware.** Composition layers (`Space::layers`) are checked
  only against Monado's null compositor, which accepts them but draws
  nothing; how SteamVR shows them under the projection layer (hole edges
  while the head moves, cylinders) is untried. A panel that is carried or
  resized loses its layer and its swapchain is made again when let go;
  layer swapchains have one mip level.
- **Other platforms.** beui-layer-xr is compiled for Linux and Windows, but
  `./scripts/verify` cross-checks only `windows_x86_64`; `windows_arm64` is
  untried. Elsewhere the `Link` fails at once (Android is M9).
- **Blocking.** `xrWaitFrame` blocks the main loop, which also runs apps and
  the Wayland server; waiting on another thread and waking the loop would
  free it.

## Deferred milestones

The user has deferred these three; do not start them without asking.

### M9. Quest standalone

The Android OpenXR loader in the APK, pacing in beui-platform-android, the
manifest, and the buck APK plumbing. First check whether today's APK runs as a
2D panel app. Android reads none of the `BEUI_*` variables
(`from_environment` in `crates/beui-host/src/runner.rs` returns `None` there), so
space cannot be turned on there today.

About 1.5k lines plus build work.

### M10. Comfort, passthrough, hands

Each 0.3-1k lines:

- Comfort: snap and smooth turning, a recenter that moves the layout, seated
  and standing.
- Passthrough: `XR_FB_passthrough`, or an alpha-blended environment.
- Hand tracking: a pinch is a trigger press of that hand's laser, a poke is
  touch.

### M11. WebXR

The riskiest: wgpu has no WebXR binding, the WebGL route needs an
`xrCompatible` WebGL2 context, and space must build for wasm and be wired into
`run_web` (`crates/beui-host/src/web.rs`, which launches with no layers).

2k lines or more. **(ask)** whether WebXR is in scope.

## Smaller follow-ups

- **Keyboard panel.** It is placed only when it opens; moving the focus to a
  field on another panel leaves it where it was. A keyboard docked to the
  off hand (following its grip pose) is untried.

- **Placements across restarts.** Panels reopen in place only within a run.
  Keeping placements must live in be-host's session (the shell's view
  block, `RootSettings::ensure_session`), not in beui or the space host: no
  file under the config folder, so a plain beui app such as beui-demo keeps
  nothing. It needs the `Placed` reports for `WindowId::MAIN`
  (`Context::window_placement`) saved in the session, and a canonical way to
  hand a placement back for the main window, which space opens on its own
  before the session is known (a `WindowRequest` that places `MAIN`, say).
  Floating dock windows' placements now reach the workspace dock
  through the app protocol (the window region's `placement`), which keeps them only in
  memory (`DockState` holds none); they join the shell's saved `DockState`.

- **App windows.** Outside space they stay overlays inside their region's
  window; making them OS windows there is deferred. Drags an app starts on a window's bar reach be-host
  a frame or two late, so a press let go before `BeginMove` arrives moves
  nothing (space drops it). Drag and drop over an app window, and a
  window's accessibility (its nodes show in the frame's tree at the window's
  own coordinates), are not handled yet.

- **Wayland in space.** The compositor still gives every toplevel its
  rectangle in its own window's coordinates and the main window's screens, so
  toplevels overlap in the global space Xwayland and popup constraints see,
  and a drag over a popup's panel maps through its toplevel's rectangle.
  A card dialog anchored in another window (a rename card on a tab popped
  out) is still an overlay of the main window, drawn on the wrong panel.
  Fullscreen covers the main window's area.

- **Scroll copy** (`moved`) is used only by the wgpu window renderer: the web
  canvas always repaints everything, app surfaces fold moves into damage,
  and DRM screens do not copy. App surfaces matter most, for be-host.
- **Restored focus.** A restored session focuses the last window reopened,
  even when it is out of view.
- **Untried on hardware.** winit's lock release on focus loss (X11, macOS,
  Windows) and the web lock re-request in a real browser.
- **Deferred: a high-resolution inset**, rendering only the visible part of a
  close-up panel past `scale::LARGEST_TEXTURE`. Whatever does it must not
  render the same window twice.
