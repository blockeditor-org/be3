# VR

A plan to take the space host (`crates/beui-adapter-space`, `BEUI_ADAPTER=space`;
guides/beui.md, "Space: the VR simulator host") from a simulator on a flat
screen to real headsets. Each milestone says what it needs first and roughly
how big it is. Items marked **(ask)** need a decision from the user during the
work.

## Where it stands

Space already shows every window as a panel the viewer can carry, face, roll,
scale, bend and resize through host chrome, with a drawn 3D cursor, a held
pointer, per-window rasters that change without relayout, and placements
(scale and bend included) that reopen a window where it was within a run
(PR #691); nothing keeps them across restarts yet. A plugin's windows (the
workspace dock's floating windows, linux-desktop's) are windows of block-app,
and so panels of their own in space. It draws through any
number of views, each a pose with an off-axis field of view (`head::View`,
`head::Fov`): the simulator's one, or tracked views an XR host hands
`Space::set_views` and draws with `Space::draw` (guides/beui.md, "Views"), and
`BEUI_XR=openxr` shows it on an OpenXR headset (`crates/beui-xr`, "Headsets").
Input is a mouse pointer, a keyboard, and a laser per controller that acts as
a finger, with grips that carry, scale and turn panels (guides/beui.md,
"Lasers"), fed from OpenXR's controller actions or `drive xr`.

The simulator stays a first-class backend throughout: every space test keeps
running without a headset.

## M1 follow-ups: OpenXR

`BEUI_XR=openxr` shows space on a headset through `crates/beui-xr` on Linux
and Windows (guides/beui.md, "Headsets"), checked in CI against Monado's null
compositor and compiled for Windows by `./scripts/verify`. What remains:

- **Hardware.** Untried on a real headset: SteamVR on Windows with the
  Bigscreen Beyond first, then SteamVR and Monado on Linux.
- **Multiview.** Each eye is its own pass and swapchain; one pass with
  wgpu's `multiview_mask` and `@builtin(view_index)` into a two-layer
  swapchain would halve the draw calls.
- **A hidden mirror.** The runner skips a frame while the platform window has
  no size (`Renderers::physical`), so minimising the mirror stops the
  headset's frames; a paced host should keep running then.
- **Blocking.** `xrWaitFrame` blocks the main loop, which also runs plugins and
  the Wayland server; waiting on another thread and waking the loop would
  free it.
- **Late poses.** The eyes are located when input begins, before the app's
  frame; locating them again just before drawing would cut latency.
- **A lost session** falls back to the simulator for the rest of the run;
  nothing opens a new one when the runtime comes back.
- **Body offset.** The simulator's Ctrl+Shift look and walk do nothing on a
  headset; turning and walking (M10's snap turn and recenter) need an offset
  between the tracking space and the world.

## M5. Keyboard panel

The on-screen keyboard (`SoftwareKeyboard`) as a window that never takes the
focus (a flag or role on `WindowRequest::Open`), placed near the focused field
or panel, or by a hand. Space reports no keyboard attached while controllers
are active.

About 0.5k lines.

## M8. Composition layers

A quad or cylinder OpenXR layer per panel (radius and angle from `bend`), for
sharper text and idle panels that cost nothing to redraw. Layers have no depth
between them, runtimes cap how many there are, and cylinder support varies, so
fall back to the eye buffer.

About 1k lines. Needs M1.

## M7. Wayland toplevels and popups as panels

Each Wayland toplevel is a panel; popups become `Over` windows. Plugin
windows (`EditorRegion::Window`, block-app's `RegionWindows`) show the way:
linux-desktop's `Window`s already become panels in space.

About 0.7k lines.

## M9. Quest standalone

The Android OpenXR loader in the APK, pacing in beui-adapter-android, the
manifest, and the buck APK plumbing. First check whether today's APK runs as a
2D panel app. Android reads none of the `BEUI_*` variables
(`from_environment` in `crates/beui/src/window.rs` returns `None` there), so
space cannot be turned on there today.

About 1.5k lines plus build work. Needs M1, ideally M8.

## M10. Comfort, passthrough, hands

Each 0.3-1k lines:

- Comfort: snap and smooth turning, a recenter that moves the layout, seated
  and standing.
- Passthrough: `XR_FB_passthrough`, or an alpha-blended environment.
- Hand tracking: a pinch is a trigger press of that hand's laser, a poke is
  touch.

Needs M1.

## M11. WebXR

The riskiest: wgpu has no WebXR binding, the WebGL route needs an
`xrCompatible` WebGL2 context, and space must build for wasm and be wired into
`run_web` (`crates/beui/src/web.rs`, which launches with `host: None`).

2k lines or more. **(ask)** whether WebXR is in scope.

## Suggested order

M5, M8, M7, M9, M10, M11.

## Smaller follow-ups

- **Placements across restarts.** Panels reopen in place only within a run.
  Keeping placements must live in block-app's session (the shell's view
  block, `RootSettings::ensure_session`), not in beui or the space host: no
  file under the config folder, so a plain beui app such as beui-demo keeps
  nothing. It needs the `Placed` reports for `WindowId::MAIN`
  (`Context::window_placement`) saved in the session, and a canonical way to
  hand a placement back for the main window, which space opens on its own
  before the session is known (a `WindowRequest` that places `MAIN`, say).
  Floating dock windows' `Placed` reports now reach the workspace dock
  through the plugin protocol (`window-reported`), which keeps them only in
  memory (`DockState` holds none); they join the shell's saved `DockState`.

- **Plugin windows.** Outside space they stay overlays inside block-app's
  panel; making them OS windows there is deferred. Drags a plugin starts on a window's bar reach block-app
  a frame or two late, so a press let go before `BeginMove` arrives starts a
  carry that ends at the next click. Drag and drop over a plugin window, and a
  window's accessibility (its nodes show in the frame's tree at the window's
  own coordinates), are not handled yet.

- **Scroll copy** (`moved`) is used only by the wgpu window renderer: the web
  canvas always repaints everything, plugin surfaces fold moves into damage,
  and DRM screens do not copy. Plugin surfaces matter most, for block-app.
- **Restored focus.** A restored session focuses the last window reopened,
  even when it is out of view.
- **Untried on hardware.** winit's lock release on focus loss (X11, macOS,
  Windows) and the web lock re-request in a real browser.
- **Deferred: a high-resolution inset**, rendering only the visible part of a
  close-up panel past `scale::LARGEST_TEXTURE`. Whatever does it must not
  render the same window twice.

## Open questions

- **(ask)** Which headset and runtime first: Linux PC VR, or Quest?
- **(ask)** Is WebXR in scope? (M11)
