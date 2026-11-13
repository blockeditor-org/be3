# The host as a platform adapter

Depends on: none.

be-host shrinks to what is the platform: windows and input, the GPU, the wasm engine and OS
services. Everything with a UI moves into wasm guests: a trusted top-level guest the host calls
synchronously, and below it the shell and apps as today. The block stack moves into a second
trusted guest. Items marked **(ask)** need a decision from the user; Open questions, at the end,
lists them.

Why:

- An app written once behaves the same everywhere only if the host does. With the compositor and
  shell in wasm, the host's own UI is identical on every platform.
- beui is compiled twice today: natively into be-host, and to wasm in the beui app library
  (guides/app_libraries.md), whose `.cwasm` is already built per platform. Afterwards only the wasm
  copy is left.
- A small native host is less code to port and check per platform, and a smaller trusted base
  beside the sandbox.

In November 2026, be-host's native Linux dependency tree was 674 crates, 83 of them first-party,
including all of beui, FreeType and HarfBuzz, the block stack with SQLite and rustls, symphonia and
be-server (`be-host-stack/src/embedded_server.rs`).

## The shape

```
host (native, per platform)
 ├─ top-level guest: Space while it is on, calling the compositor; else the compositor
 │   │                                                        synchronous, trusted
 │   └─ the shell (workspace-ui, or the desktop app under --desktop) and apps
 │                                                            asynchronous, sandboxed
 └─ data guest: blocks, sync, accounts, keys, storage         asynchronous, trusted
```

## What the host keeps

- The platform adapters: winit, DRM, Android and the web page, with window state, input
  normalization, clipboard and IME. They use the types the beui platforms already use
  (`WindowRequests`, `WindowReports`, the input vocabulary), but no widgets, text or fonts.
- One wgpu device, the GPU proxy (`be-wasm-gpu-host`) and a pass that composites the layer list
  (below).
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
- The AccessKit platform adapters, fed the one tree the top-level guest publishes.
- OS desktop integration: be-host-wayland, be-host-services and be-host-win32*.
- OpenXR: the runtime, session, frame loop, poses, swapchains and composition layers
  (beui-layer-xr without its beui `Layer` wrapper).
- A watchdog that restarts guests. When it restarts the top-level guest, it drops every hold an
  instance had: pointer lock, fullscreen, the cursor and idle inhibit.

Until the top-level guest commits its first frame, the host shows a plain background colour. It
has no text and no fallback UI of its own.

## The top-level guest

The host calls it inline on its frame thread rather than stepping it on a worker:

```
(XR)  xrWaitFrame, xrBeginFrame, locate the head and controllers
      guest.input(events)
      guest.frame(predicted display time, poses)
        records and submits its GPU work; returns the layer list and an accessibility update
(XR)  xrEndFrame
```

Its rules:

- Only a staged, hash-checked, first-party module may be top-level.
- It never blocks its main thread: no waiting on atomics or on locks other threads hold. Its own
  background threads may. These are the browser main thread's rules, which beui already follows.
- Each call has a deadline, enforced with wasmtime's epoch interruption. On a miss the host keeps
  the previous frame (with a headset, the runtime reprojects it). Repeated misses, or a trap,
  restart the guest.
- It never waits for its children inside a call. It reads their latest frames.
- While Space is on, Space is top-level and calls the compositor inline in the same call. Both are
  trusted and first-party.
- On the web it runs on the page's main thread, called from `requestAnimationFrame` or the XR
  session's frame callback: WebXR runs only there, and a page cannot call into a worker
  synchronously. The page becomes thin JS plus this guest.

What moves into the compositor:

- `crates/be-host/src/compositor`: app views, anchored views, desktop windows, editors, the camera
  and the palette.
- `crates/be-host/src/ui`: the lock screen, onboarding, saving, keep changes, organization,
  dialogs, toasts and the debug panels.
- be-host-accounts-ui.
- be-host-runtime's orchestration, as a guest library: pacing, settle, restarts, input routing and
  grafting the accessibility trees.
- beui's inspector and drive layer.

## The layer list

Each frame the top-level guest returns, for each OS window, its layers in z-order. Each layer has
a surface handle, a 2D affine transform, a clip (a rectangle or rounded rectangle), an opacity and
whether it is shown. The surfaces are:

- the guest's own, of which it may have several, such as one below its children and one above;
- each instance's screens and anchored overlays;
- surfaces the host owns, such as program windows;
- later, web documents (iframes).

A layer may instead carry a 3D pose, as a quad or a cylinder. Only Space uses that, for headset
composition layers.

- **Native:** a wgpu pass draws the layers. OS compositor planes may take this over later.
- **Web:** the page applies the list as CSS to the canvases it already places
  (`be-host-runtime/src/web/canvases.rs` and `overlay.rs`). A child's `OffscreenCanvas` goes to
  its worker when it is spawned, and the guest names it only by handle.
- **The guest arranges child surfaces and never reads their pixels.** On the web it cannot, since
  each worker has its own GPU device, so the rule holds everywhere. Nothing in the host blurs; an
  app that wants a blurred backdrop draws one from its own content.
- **Damage:** each surface reports damage rectangles and layers keep stable ids. The host
  transforms each surface's damage and adds layers that moved, for e-paper's `FB_DAMAGE_CLIPS`.
- **OS windows:** the guest opens, sizes and closes them (one per display, dock tabs floated out,
  popups that escape their window) with `WindowRequests`, and the host answers with
  `WindowReports`.

## The sandbox API

A new WIT interface the host serves to trusted guests, and later to apps:

- `compile(bytes) -> module`, hashed and cached.
- `spawn(module, config, grants) -> instance and channel`. The config sets the memory limit, fuel
  or epochs, threads, determinism (canonical NaNs, no threads) and libraries.
- There are two ways to spawn, because they differ in trust:
  - **A staged app**, by app id. The host checks it against its manifest and binds host-served
    imports to that app's identity. The spawner can narrow those grants but never widen them.
  - **An arbitrary module**, from bytes. It gets no host-served imports, only what its spawner
    serves.
- A grant can point at another instance's service, and the host connects the two channels
  directly. So an app's block imports go straight to the data guest, bound to that app's
  identity, and never pass through the compositor.
- The channel speaks the existing app protocol (`be_wasm_guest_host::Channel`), with the spawner
  as the other end.
- On the web, spawning makes a Web Worker from the page.

Other users:

- The data guest spawns type modules.
- Once apps may use it, the Games app can run game modules on wasmtime natively, instead of on
  wasmi inside its own module (tabletop-host). The web has no canonical NaNs, so it may keep
  wasmi.
- The input remapper (`be-host-remap-host`, on wasmi today) runs through it.

## The GPU proxy

- **Sharing textures:** an instance's presented texture is a handle the host composites from the
  layer list, on the same device, so it never leaves the GPU.
- **Late latching:**
  - The host owns small buffers that guest pipelines bind.
  - For an XR frame it holds the guest's final submit, writes the latest predicted poses into
    those buffers, then submits.
  - The guest draws laser beams in controller space from the latched pose. It finds where each
    beam meets a panel in a compute pass that reads the same buffer, so both the beam's angle and
    its length are current.
  - Deciding which panel a press goes to uses last frame's pose.
- **Headset swapchains:** their images are proxy textures. A panel's composition layer names an
  app's surface, and the host copies that surface into the panel's swapchain itself, so the
  layered panels never pass through the guest.

## The data guest

be-block-client, be-host-base and be-host-stack already run as wasm on the web: SQLite on OPFS,
the browser's WebSocket and `fetch`, and the block stack in `web/be-worker.js`. They gain a third
backend over the host's capabilities and drop the native tokio, rustls and tungstenite path, so
each has one transport.

- **Files:** a WASI preopen natively, OPFS on the web.
- **Network:** WebSocket and HTTP. Natively the proxy credential header is allowed; the web
  refuses it, as a browser does.
- **Block access:** apps' block imports are granted to it directly (above), and it enforces block
  access per app identity, as the host does now.
- **Without a UI:** it runs without the compositor, for Android push and background sync.
- **Native builds:** the crates stay portable, so their native tests, be-server and the CLIs keep
  building natively.

Costs:

- **Build time:** `./scripts/verify` gets slower, since the block stack and SQLite are compiled to
  wasm and precompiled on top of the native builds the tests need.
- **Run time:** wasm is slower, especially for hashing and encryption, which have no AES or SHA
  instructions. Measure a large blob import and the first sync of a large workspace.

## Other moves

- **Pan and zoom:** the camera (`src/compositor/editors/camera.rs`) moves into the compositor.
  Gestures reach the outermost view through the guest's own hit path, and the `pan_drag` handoff
  stays inside the guest. decisions/pan_and_zoom_camera_belongs_to_the_host.md becomes "the
  camera belongs to the compositor". An editor still never keeps its own camera.
- **Accessibility:** the compositor grafts the apps' trees, taking the host's renumbering and
  repair code with it. The host passes the one tree to its AccessKit adapter and routes actions
  back.
- **The clock:** the host owns the frame clock and hands it to every guest directly. drive's
  stopped and hurried clock, its pinned wall clock and `STOPPED_CLOCK_FRAME_BUDGET` apply to all
  guests alike. With a headset, the clock is `xrWaitFrame`'s predicted display time.
- **Memory:** the compositor decides which idle apps to stop. The host enforces hard limits and
  reports memory pressure.
- **Locking:** after a resume or a lock request, the host shows nothing new until the top-level
  guest commits a frame it marks as locked.
- **Desktop interfaces:** be3:desktop's shell-only interfaces (`shell_only` in
  `be-host-runtime/src/published.rs`) are granted to whichever child is the shell.

Decisions to rewrite when this lands:

- decisions/pan_and_zoom_camera_belongs_to_the_host.md
- decisions/beui_product_specific_components_live_in_the_product.md (`LockScreen`,
  `KeepChanges` and the notices move from be-host to the compositor)
- decisions/beui_facade_excludes_the_engine.md (the app runtime uses beui-host's engine)
- decisions/app_tests_run_as_wasm_guests.md (the host's own crates paint natively)

## Steps

Each step lands on its own.

1. The sandbox API for arbitrary modules. Move the remapper onto it.
2. Spawning staged apps with identity-bound grants, grants that point at other instances, and
   texture sharing between guests. A test guest embeds one editor without the host's compositor.
3. The layer list and how native and the web realize it, with the host's current UI as its only
   producer, so nothing changes visibly.
4. The synchronous top-level entry with deadlines and restarts. The compositor guest takes over
   `src/compositor`, `src/ui`, the camera and the orchestration.
5. The data guest behind capabilities.
6. Space and the XR capability as above, with late latching.
7. Strip the host: drop beui's widgets, text and fonts, the block stack and audio decoding. Update
   guides/apps/runtime.md, guides/desktop.md, guides/running/web.md, guides/running/drive.md,
   guides/beui/space.md, guides/pan_and_zoom.md and guides/testing_a_gui.md.

## Open questions

1. **(ask)** **Paintings.** `drive paint` and the host tests compare beui paintings of the whole
   window, which a host without beui cannot paint. Either the compositor assembles the window's
   painting from its own and its children's paintings, placed by its layer list, which keeps
   paintings independent of the machine; or the tests compare screenshots.
2. **(ask)** **Input to surfaces the host owns:** Wayland and X11 program windows, and later
   iframes and native apps out of process. Either the compositor decides focus and the target of
   each event and the host carries it out, which costs no extra hop now that the top-level guest
   is synchronous; or the compositor declares input regions and the host routes, which brings
   hit-testing back into the host.
3. **(ask)** **The root and the shell.** Either a fixed compositor runs workspace-ui, or the
   desktop app under `--desktop`, as a child shell; or each shell is itself the compositor and
   shares a compositor library.
4. **(ask)** **The data guest.** Should the block stack move at all, given the slower verify and
   slower crypto in wasm? If it does, should it be a guest of its own, as above, or part of the
   top-level guest?
5. **(ask)** **The embedded be-server.** It serves local profiles natively. It could become a
   guest with a capability to listen on a socket, be reached by the data guest through a channel
   in-process, stay native, or local profiles could be dropped.
6. **(ask)** **A restarted compositor.** Do its children die with it, or does the host keep them
   for the new compositor to take back?
7. **(ask)** **Apps' platform calls:** clipboard, drag and drop, file dialogs and opening URLs.
   Either direct grants bound to the app's identity, which the compositor can deny, or every call
   goes through the compositor.
8. **(ask)** **Sound and video.** Decoding moves to guests and the host offers PCM output, or the
   host keeps its codecs. Hardware video decoding into textures can only be done by the host.
9. **(ask)** **The GPU device and headsets.** OpenXR wants the Vulkan device created through it
   (`beui-layer-xr/src/link.rs`), and every guest shares the host's device. Either the host
   creates it through OpenXR whenever a headset might be used, or guests restart their GPU work
   when a headset appears.
