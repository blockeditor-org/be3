# VR

A plan to take the space host (`crates/beui-layer-space`, `BEUI_PLATFORM=space`;
guides/beui.md, "Space: the VR simulator host") from a simulator on a flat
screen to real headsets. Each milestone says what it needs first and roughly
how big it is. Items marked **(ask)** need a decision from the user during the
work.

## Where it stands

Space already shows every window as a panel the viewer can carry, face, roll,
scale, bend and resize through host chrome, with a drawn 3D cursor, a held
pointer, per-window rasters that change without relayout, and placements
(scale and bend included) that reopen a window where it was within a run
(PR #691); nothing keeps them across restarts yet. It draws through one
eye: `Head` has a single pose and a fixed symmetric projection
(`head::FOV_Y`, `Head::view_projection`), and `scale::wanted` sizes rasters
from that one view. Input is one mouse pointer and a keyboard.

The simulator stays a first-class backend throughout: every space test keeps
running without a headset.

## M0. Viewers in space

Space renders N views, each an off-axis projection built from asymmetric field
of view tangents (left, right, up, down), from a pose source: the simulator's
mouse, WASD and Q/E steering, or XR's `xrLocateViews` against a `LOCAL_FLOOR`
or `STAGE` space. `scale::wanted` takes the most any view needs. Code that
reads `self.head` keeps working off the centre eye. No behaviour change.

About 0.8k lines. Blocks M1, M3 and M11.

## M1. OpenXR on Linux

A new `beui-xr` crate on the `openxr` crate's `loaded` feature (dlopen the
loader; no C++ build).

- **Device.** Widen `beui_renderer_wgpu::present::OpenDevice` so XR creates
  the VkInstance and VkDevice (`XR_KHR_vulkan_enable2`) and hands them to wgpu
  through `wgpu::hal::vulkan::Instance::from_raw`, `expose_adapter` and
  `device_from_raw`, merging the extensions `be_dmabuf::open_device` adds in
  its `open_with_callback`.
- **Swapchains.** Import swapchain images as `be-dmabuf` imports buffers
  (`create_texture_from_hal`).
- **Pacing.** A host pacing hook: `xrWaitFrame` drives frames, winit runs in
  `Poll`, the mirror presents without vsync, and the predicted display time
  is `HostView::now` and the frame clock.
- **Drawing.** Multiview (wgpu 29's `multiview_mask`, `@builtin(view_index)`)
  with a two-pass fallback; MSAA resolves into the swapchain.
- **Mirror.** A desktop window mirrors the view and keeps keyboard focus.

Risks:

- hal interop is unsafe and breaks across wgpu upgrades;
- swapchain acquire and release must be ordered around wgpu's submits;
- XR picks the GPU, and dmabuf import (Wayland clients) needs that same device;
- `xrWaitFrame` blocks the main loop, which also runs plugins and the Wayland
  server.

About 3-4k lines. Needs M0.

CI: a smoke test against Monado's headless (null) compositor on lavapipe,
landed with M1.

## M3. Controllers as lasers

- **beui-core.** A ray pointer kind (`PointerId::Ray(TouchId)`, next to
  `Mouse`, `Touch` and `Pen`) that hovers, and presses like touch: a drag
  scrolls, a long press opens the context menu. Hover paths, scroll and
  cursors become per pointer (`WindowRequests::cursor` per pointer). This part
  can start before M0.
- **Space.** Per-laser state replaces the single `carry`, `resize`,
  `chrome.gesture`, `hovered` and `captured` on `Space`. Grip carries; two
  hands scale and rotate. Rays are smoothed (One-Euro) and frozen at a
  trigger press so the click lands where it was aimed.
- **drive.** `drive xr …` commands set controller poses and buttons.

About 2k lines. Needs M0. **(ask)** whether a laser acts as touch or as a
hovering mouse, and whether the second hand is a full pointer.

## M5. Keyboard panel

The on-screen keyboard (`SoftwareKeyboard`) as a window that never takes the
focus (a flag or role on `WindowRequest::Open`), placed near the focused field
or panel, or by a hand. Space reports no keyboard attached while controllers
are active.

About 0.5k lines. Needs M3.

## M8. Composition layers

A quad or cylinder OpenXR layer per panel (radius and angle from `bend`), for
sharper text and idle panels that cost nothing to redraw. Layers have no depth
between them, runtimes cap how many there are, and cylinder support varies, so
fall back to the eye buffer.

About 1k lines. Needs M1.

## M6. Windows from plugins

`beui_platform_plugin::launch` (`crates/beui-platform-plugin`) passes `host: None`, so
`opens_windows` is false in a plugin and every plugin `Window` (the workspace
dock, linux-desktop) is an overlay inside block-app's one panel.

- Move the framework-free window vocabulary (`WindowId`, `WindowPlacement`,
  `HostPlacement`, `WindowEdge`, `WindowRequest`, `WindowReport`, today in
  `beui_core::window`) to ui-types.
- block-plugin-api carries window requests and reports, and a window region
  kind.
- Each plugin window gets its own screen surface; textures stay on the GPU.
- beui-platform-plugin gets a `WindowHost` that forwards to the plugin
  protocol.
- block-app opens a beui `Window` holding a `PluginRegion` for each plugin
  window.
- Plugin windows' host placements then reach the dock, which can keep them
  across restarts (see "Placements across restarts").

About 3-4k lines. Independent of XR. **(ask)** whether plugin windows also
become real OS windows outside VR.

## M7. Wayland toplevels and popups as panels

Each Wayland toplevel is a panel; popups become `Over` windows.

About 0.7k lines. Needs M6.

## M9. Quest standalone

The Android OpenXR loader in the APK, pacing in beui-platform-android, the
manifest, and the buck APK plumbing. First check whether today's APK runs as a
2D panel app. Android reads none of the `BEUI_*` variables
(`from_environment` in `crates/beui/src/window.rs` returns `None` there), so
space cannot be turned on there today.

About 1.5k lines plus build work. Needs M1 and M3, ideally M8.

## M10. Comfort, passthrough, hands

Each 0.3-1k lines:

- Comfort: snap and smooth turning, a recenter that moves the layout, seated
  and standing.
- Passthrough: `XR_FB_passthrough`, or an alpha-blended environment.
- Hand tracking: a pinch is a ray press, a poke is touch.

Needs M1 and M3.

## M11. WebXR

The riskiest: wgpu has no WebXR binding, the WebGL route needs an
`xrCompatible` WebGL2 context, and space must build for wasm and be wired into
`run_web` (`crates/beui/src/web.rs`, which launches with `host: None`).

2k lines or more. Needs M0 and M3. **(ask)** whether WebXR is in scope.

## Suggested order

M0, M1, M3, M5, M8, M6, M7, M9, M10, M11.

## Smaller follow-ups

- **Placements across restarts.** Panels reopen in place only within a run.
  Keeping placements must live in block-app's session (the shell's view
  block, `RootSettings::ensure_session`), not in beui or the space host: no
  file under the config folder, so a plain beui app such as beui-demo keeps
  nothing. block-app has only its main panel in space today, so it needs the
  `Placed` reports for `WindowId::MAIN` (`Context::window_placement`) saved in
  the session, and a canonical way to hand a placement back for the main
  window, which space opens on its own before the session is known (a
  `WindowRequest` that places `MAIN`, say). Floating dock windows' placements
  join the shell's saved `DockState` once plugin windows are real (M6).

- **Plugin editors aren't raster-aware.** A plugin region carries one scale
  (`ScreenPlacement::scale_factor_millis`), so plugin content in space is as
  soft at a high raster as before. block-app's host must send each region's
  raster next to its scale.
- **`WindowReport::Scaled`** now means the raster only; consider renaming it
  (`Rastered`).
- **Chrome labels** are raw `Shape::Text` (`chrome::output`), stretched rather
  than rerasterised.
- **Scroll copy** (`moved`) is used only by the wgpu window renderer: the web
  canvas always repaints everything, plugin surfaces fold moves into damage,
  and DRM screens do not copy. Plugin surfaces matter most, for block-app.
- **Carry.** Letting go mid-turn snaps the rotation (`Panel::face`) without
  solving the in-front distance again; `Carry::aim` swings by the ray's change
  of world yaw during the 0.3 s turn.
- **Scene painting** (`picture.rs`) orders panels by centre distance, not per
  pixel depth.
- **Restored focus.** A restored session focuses the last window reopened,
  even when it is out of view.
- **Untried on hardware.** winit's lock release on focus loss (X11, macOS,
  Windows) and the web lock re-request in a real browser.
- **Deferred: a high-resolution inset**, rendering only the visible part of a
  close-up panel past `scale::LARGEST_TEXTURE`. Whatever does it must not
  render the same window twice.

## Open questions

- **(ask)** Which headset and runtime first: Linux PC VR, or Quest?
- **(ask)** Controllers as touch or as a hovering mouse, and is the second
  hand a full pointer? (M3)
- **(ask)** Is block-app the single XR shell, with other apps as Wayland or
  plugin windows? Should plugin windows (M6) also become real OS windows
  outside VR?
- **(ask)** Is WebXR in scope? (M11)
