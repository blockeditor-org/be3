# The host as a platform adapter

Depends on: plans/local_accounts.md.

be-host shrinks to what is the platform: windows and input, the GPU, the wasm engine and OS
services. Everything with a UI moves into wasm guests: a root guest the host calls synchronously,
which is the app, and below it the shell and apps as today. The block stack moves into the root
guest, and the host knows nothing of it. Items marked **(ask)** need a decision from the user;
Open questions, at the end, lists them.

Why:

- An app written once behaves the same everywhere only if the host does. With the compositor and
  shell in wasm, the host's own UI is identical on every platform.
- beui is compiled twice today: natively into be-host, and to wasm in the beui app library
  (guides/app_libraries.md), whose `.cwasm` is already built per platform. Afterwards only the wasm
  copy is left.
- A small native host is less code to port and check per platform.

In November 2026, be-host's native Linux dependency tree was 674 crates, 83 of them first-party,
including all of beui, FreeType and HarfBuzz, the block stack with SQLite and rustls, symphonia and
be-server (`be-host-stack/src/embedded_server.rs`).

This plan adds no security boundary. Every guest is trusted, as every app is today, and the host
keeps only the per-app rules it already enforces (guides/apps/host_services.md), such as a
revision history editor being unable to change a block.

## The shape

```
host (native, per platform)
 └─ root guest: the compositor, or Space while it is on, calling the compositor   synchronous
     ├─ the block stack, on the root guest's own threads
     └─ the shell (workspace-ui, or the desktop app under --desktop) and apps     asynchronous
```

The compositor is a fixed root. workspace-ui stays the shell, as an editor the compositor runs,
rather than becoming the root itself.

## What the host keeps

- The platform adapters: winit, DRM, Android and the web page, with window state, input
  normalization, clipboard and IME. They use the types the beui platforms already use
  (`WindowRequests`, `WindowReports`, the input vocabulary), but no widgets, text or fonts.
- A wgpu device every guest draws on, as apps do today, the GPU proxy (`be-wasm-gpu-host`) and a pass that
  composites the layer list (below).
- The wasm engine, the linker (`be-wasm-link`) and the sandbox service (below).
- Capabilities guests are granted:
  - files in the data directory, and the lock that keeps a second be-host off it
  - WebSocket and HTTP
  - timers and the frame clock
  - randomness
  - opening URLs
  - push tokens and notifications
  - sound output as PCM
  - password checks (PAM in the login session)
  - read-only statistics (process memory, step times) for the debug panels
- The AccessKit platform adapters, fed the one tree the root guest publishes.
- OS desktop integration: be-host-wayland, be-host-services and be-host-win32*.
- OpenXR: the runtime, session, frame loop, poses, swapchains and composition layers
  (beui-layer-xr without its beui `Layer` wrapper).

Until the root guest commits its first frame, the host shows a plain background colour. It has
no text and no fallback UI of its own.

## The root guest

The root guest is the app. If it traps, be-host exits as it would on a panic today, so this is no
worse than now. Apps below the root are restarted by the
compositor as the host restarts them today.

The host calls it inline on its frame thread rather than stepping it on a worker:

```
(XR)  xrWaitFrame, xrBeginFrame, locate the head and controllers
      guest.input(events)
      guest.frame(predicted display time, poses)
        records and submits its GPU work; returns the layer list and an accessibility update
(XR)  xrEndFrame
```

Its rules:

- It is be-host's own module, staged with it, never one from user bytes.
- It never blocks its main thread: no waiting on atomics or on locks other threads hold. Its own
  background threads may, and the block stack runs on them. These are the browser main thread's
  rules, which beui already follows.
- Each call has a deadline, enforced with wasmtime's epoch interruption. On a miss the host keeps
  the previous frame (with a headset, the runtime reprojects it) and calls again on the next
  frame.
- It never waits for its children inside a call. It reads their latest frames.
- While Space is on, Space is the root and calls the compositor inline in the same call.
- On the web it runs on the page's main thread, called from `requestAnimationFrame` or the XR
  session's frame callback: WebXR runs only there, and a page cannot call into a worker
  synchronously. The page becomes thin JS plus this guest.

What moves into the compositor:

- `crates/be-host/src/compositor`: app views, anchored views, desktop windows, editors, the camera
  and the palette.
- `crates/be-host/src/ui`: the lock screen, onboarding, saving, keep changes, organization,
  dialogs, toasts and the debug panels.
- be-host-accounts-ui.
- be-host-runtime's orchestration, as a guest library: pacing, settle, restarts and grafting the
  accessibility trees. How much of input routing moves depends on open question 1.
- beui's inspector and drive layer.

## The layer list

Each frame the root guest returns, for each OS window, its layers in z-order. Each layer has a
surface handle, a 2D affine transform, a clip (a rectangle or rounded rectangle), an opacity and
whether it is shown. The surfaces are:

- the root guest's own, of which it may have several, such as one below its children and one
  above;
- each instance's screens and anchored overlays;
- surfaces the host owns, such as program windows;
- later, web documents (iframes).

A layer may instead carry a 3D pose, as a quad or a cylinder. Only Space uses that, for headset
composition layers.

- **Native:** a wgpu pass draws the layers. OS compositor planes may take this over later.
- **Web:** the page applies the list as CSS to the canvases it already places
  (`be-host-runtime/src/web/canvases.rs` and `overlay.rs`). A child's `OffscreenCanvas` goes to
  its worker when it is spawned, and the root guest names it only by handle.
- **The root guest arranges child surfaces and never reads their pixels.** On the web it cannot,
  since each worker has its own GPU device, so the rule holds everywhere. Nothing in the host
  blurs; an app that wants a blurred backdrop draws one from its own content.
- **Damage:** each surface reports damage rectangles and layers keep stable ids. The host
  transforms each surface's damage and adds layers that moved, for e-paper's `FB_DAMAGE_CLIPS`.
- **OS windows:** the root guest opens, sizes and closes them (one per display, dock tabs floated
  out, popups that escape their window) with `WindowRequests`, and the host answers with
  `WindowReports`.

## The sandbox API

A new WIT interface the host serves to the root guest, and later to apps:

- `compile(bytes) -> module`, hashed and cached.
- `spawn(module, config, imports) -> instance and channel`. The config sets the memory limit, fuel
  or epochs, threads, determinism (canonical NaNs, no threads) and libraries.
- There are two ways to spawn:
  - **A staged app**, by app id. The host loads it with its manifest and binds the imports it
    serves to that app, as it does today.
  - **A module from bytes.** It gets only the imports its spawner serves.
- The channel speaks the existing app protocol (`be_wasm_guest_host::Channel`), with the spawner
  as the other end.
- On the web, spawning makes a Web Worker from the page.

An app's platform calls (clipboard, drag and drop, file dialogs, opening URLs) are served by the
host directly for that app, and the compositor can turn them off for it.

Other users:

- The root guest spawns type modules for the block stack.
- Once apps may use it, the Games app can run game modules on wasmtime natively, instead of on
  wasmi inside its own module (tabletop-host). The web has no canonical NaNs, so it may keep
  wasmi.
- The input remapper (`be-host-remap-host`, on wasmi today) runs through it.

## The GPU

- **Devices:** natively every guest draws on the host's device, as every app does today
  (guides/apps/framework.md), so an instance's presented texture is already on it and never
  leaves the GPU. On the web each worker keeps its own device. A device per guest natively is
  set aside in plans/deferred/gpu_devices_per_guest.md.
- **Headsets:** the host creates its device through OpenXR, which a headset requires, only when
  `BEUI_LAYERS=xr` is set.
- **Late latching:**
  - The host owns small buffers that the root guest's pipelines bind.
  - For an XR frame it holds the root guest's final submit, writes the latest predicted poses
    into those buffers, then submits.
  - Space draws laser beams in controller space from the latched pose. It finds where each beam
    meets a panel in a compute pass that reads the same buffer, so both the beam's angle and its
    length are current.
  - Deciding which panel a press goes to uses last frame's pose.
- **Headset swapchains:** their images are textures on the host's device. A panel's composition
  layer names an app's surface, and the host copies that surface into the panel's swapchain
  itself, so the layered panels never pass through the root guest.

## The block stack in the root guest

The root guest runs the block stack in-process, on threads of its own, as the web build already
runs it in `web/be-worker.js`: a second instance of the module on the page's shared memory. The
host does not know it is there; it only grants the root guest capabilities.

be-block-client, be-host-base and be-host-stack already run as wasm on the web: SQLite on OPFS,
and the browser's WebSocket and `fetch`. They gain a backend over the host's capabilities and drop
the native tokio, rustls and tungstenite path, so each has one transport.

- **Files:** a WASI preopen natively, OPFS on the web.
- **Network:** WebSocket and HTTP. Natively the proxy credential header is allowed; the web
  refuses it, as a browser does.
- **Apps' block calls** reach the root guest, which answers them on the block stack's threads, so
  a slow query never holds up a frame, and keeps the per-app rules the host enforces now.
- **Local accounts** need no server (plans/local_accounts.md), so be-server leaves be-host.
- **Without a UI:** for Android push and background sync the host starts the root guest with no
  window, and it runs the block stack alone.
- **Native builds:** the crates stay portable, so their native tests, be-server and the CLIs keep
  building natively.

Costs:

- **Build time:** `./scripts/verify` gets slower, since the block stack and SQLite are compiled to
  wasm and precompiled on top of the native builds the tests need.
- **Run time:** wasm is slower, especially for hashing and encryption, which have no AES or SHA
  instructions. Measure a large blob import and the first sync of a large workspace.

## Other moves

- **Paintings:** `drive paint` and the host tests compare beui paintings of the whole window. The
  compositor assembles that painting from its own and its children's paintings, placed by its
  layer list, so paintings stay independent of the machine. `drive tree`, `find` and pressing by
  name read the root guest's accessibility tree.
- **Pan and zoom:** the camera (`src/compositor/editors/camera.rs`) moves into the compositor,
  which keeps fitting and per-block "moved" memory. decisions/pan_and_zoom_camera_belongs_to_the_host.md
  becomes "the camera belongs to the compositor". An editor still never keeps its own camera.
- **Accessibility:** the compositor grafts the apps' trees, taking the host's renumbering and
  repair code with it. The host passes the one tree to its AccessKit adapter and routes actions
  back.
- **Sound:** decoding (rodio in `be-host-runtime/src/audio.rs`) moves into guests, and the host
  offers PCM output. Hardware video decoding into textures, if it comes, stays in the host.
- **The clock:** the host owns the frame clock and hands it to every guest directly. drive's
  stopped and hurried clock, its pinned wall clock and `STOPPED_CLOCK_FRAME_BUDGET` apply to all
  guests alike. With a headset, the clock is `xrWaitFrame`'s predicted display time.
- **Memory:** the compositor decides which idle apps to stop. The host enforces hard limits and
  reports memory pressure.
- **Locking:** after a resume or a lock request, the host shows nothing new until the root guest
  commits a frame it marks as locked.
- **Desktop interfaces:** be3:desktop's shell-only interfaces (`shell_only` in
  `be-host-runtime/src/published.rs`) are served to whichever child is the shell.

Decisions to rewrite when this lands:

- decisions/pan_and_zoom_camera_belongs_to_the_host.md
- decisions/beui_product_specific_components_live_in_the_product.md (`LockScreen`,
  `KeepChanges` and the notices move from be-host to the compositor)
- decisions/beui_facade_excludes_the_engine.md (the app runtime uses beui-host's engine)
- decisions/app_tests_run_as_wasm_guests.md (the host's own crates paint natively)

## Steps

Each step lands on its own.

1. The sandbox API for modules from bytes. Move the remapper onto it.
2. Spawning staged apps. A test guest embeds one editor without the host's compositor.
3. The layer list and how native and the web realize it, with the host's current UI as its only
   producer, so nothing changes visibly.
4. The synchronous root entry with deadlines. The compositor takes over `src/compositor`,
   `src/ui`, the camera and the orchestration.
5. The block stack in the root guest, behind capabilities.
6. Space and the XR capability as above, with late latching.
7. Strip the host: drop beui's widgets, text and fonts, the block stack, be-server and audio
   decoding. Update guides/apps/runtime.md, guides/desktop.md, guides/running/web.md,
   guides/running/drive.md, guides/beui/space.md, guides/pan_and_zoom.md and
   guides/testing_a_gui.md.

## Open questions

1. **(ask)** **Who routes input.** The direction is that the host routes every event to the
   surface it is for, so that a future iframe web app gets real events the user started, which
   the browser grants rights from (fullscreen, the clipboard, playing sound), rather than events
   relayed through `postMessage`. Since some surfaces need the host to route, the host would
   route for all of them, one way for every surface. What needs working out:
   - What the root guest declares in its layer list for the host to hit-test: each layer's input
     region, and which layers take no input.
   - Gestures the compositor claims ahead of the surface under them: wheel, pinch and pan-drag go
     to the outermost view's camera today (guides/pan_and_zoom.md, Gestures), and a press on a
     scrim dismisses.
   - Handing a pointer from an app to the compositor mid-press: `pan_drag()`, and a drag that
     leaves an app to drop elsewhere.
   - Focus, capture, the keyboard and IME: which surface has them, and how the compositor moves
     them, for its own keys (Escape stops a presentation) and the command palette.
   - Pointer lock, the cursor shape, and hover across surfaces.
