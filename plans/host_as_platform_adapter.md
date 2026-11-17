# The host as a platform adapter

Depends on: none.

be-host shrinks to what is the platform: windows and input, the GPU, the wasm engine and OS
services. Everything with a UI moves into wasm guests: a root guest the host calls synchronously,
which is the app, and below it the shell and apps as today. The root guest runs the block stack
as an instance of its own, and the host knows nothing of it.

Why:

- An app written once behaves the same everywhere only if the host does. With the compositor and
  shell in wasm, the host's own UI is identical on every platform.
- beui is compiled twice today: natively into be-host, and to wasm in the beui app library
  (guides/app_libraries.md), which already runs on every platform. Afterwards only the wasm copy
  is left.
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
 └─ root guest: the compositor, or Space while it is on, driving the compositor    synchronous
     ├─ the block stack: its own instance and thread, spawned by the root guest
     │   └─ type modules: an instance each, spawned by the block stack
     └─ the shell (workspace-ui, or the desktop app under --desktop) and apps     asynchronous
```

The compositor is a fixed root. workspace-ui stays the shell, as an editor the compositor runs,
rather than becoming the root itself.

## What the host keeps

- The platform adapters: winit, DRM, Android and the web page, with window state, input
  normalization, clipboard and IME. They use the types the beui platforms already use
  (`WindowRequests`, `WindowReports`, the input vocabulary), but no widgets, text or fonts.
- A wgpu device every guest draws on, as apps do today, the GPU proxy (`be-wasm-gpu-host`) and
  the pass that composites the layer list (below).
- Routing input between surfaces (Input, below).
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

The host has no text and no fallback UI of its own. A window opens at once, so the user sees
something happening, and shows a plain background colour until the root guest commits its first
frame. Where be-host already owns what the screen shows before then, it keeps that instead:

- **Android:** the splash screen Android shows at launch stays until the first frame, held with
  `setKeepOnScreenCondition`, which `MainActivity` does not call yet.
- **The login session (DRM):** the display keeps what it showed, such as the display manager's
  last image, until the first frame.
- **Web:** the page may show DOM text while the root guest loads.

A failure before the first frame is logged and the window stays blank: the host has no way to
render text, so it shows no error of its own.

## The root guest

The root guest is the app. If it traps, be-host exits as it would on a panic today, so this is no
worse than now. Apps below the root are restarted by the compositor as the host restarts them
today.

The host calls it inline on its frame thread rather than stepping it on a worker:

```
(XR)  xrWaitFrame, xrBeginFrame, locate the head and controllers
      guest.input(events)
      guest.frame(predicted display time, poses)
        lays out and asks its children for frames; returns Pending while it waits for them
      the host waits on the children's answers, up to the frame's deadline, then calls again
      guest.frame(...)
        paints and submits its GPU work; returns the layer list and an accessibility update
(XR)  xrEndFrame
```

**Settling children.** Today the host waits for every app's answer before painting, up to the
frame's deadline (guides/apps/runtime.md, Settling), so a child the parent resized or moved is
drawn in its new place in the same frame. The root guest keeps that, without blocking its main
thread: its frame export is resumable. Inside, the compositor writes ordinary async Rust that
awaits its children's commits; the export returns `Pending` naming what it waits for, and the
host, which owns the children's channels, calls it again when they answer or the deadline
passes. The engine needs no async support of its own, so this works on V8 natively and on the web's
main thread alike. If that proves awkward, the fallback is two exports, `layout` and `present`,
with the host waiting between them.

Its rules:

- It is be-host's own module, staged with it, never one from user bytes.
- It never blocks its main thread: no waiting on atomics or on locks another thread may hold.
  These are the browser main thread's rules, which beui already follows.
- Its own calls have no deadline; only waiting for children does (`Host::frame_deadline`). A slow
  root frame is a slow frame, as a slow host frame is today.
- While Space is on, Space is the root and drives the compositor within the same host frame and
  on the same thread (Space and headsets, below).
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
  accessibility trees. Routing between surfaces stays in the host (Input, below).
- beui's inspector, and drive's commands. drive's socket (`beui-layer-drive/src/socket.rs`, a
  `UnixListener` served from std threads) cannot run in a guest, so it stays in the host. The
  host answers the clock controls (`clock`, `hurry`, `pause`, the pinned wall clock), since it
  owns the frame clock, and the platform's commands (`shot`, the dialogs drive stands in for).
  It passes the rest to the root guest as messages: `tree` and `find` (its accessibility tree),
  `paint` (the painting it assembles), `settle`, `synced` and `offline`. Presses by name come
  back as a point, which the host delivers as input (Input, below).

## The layer list

Each frame the root guest returns, for each OS window, its layers in z-order. Each layer has a
surface handle, a 2D affine transform, a clip (a rectangle or rounded rectangle), an opacity and
whether it is shown. The surfaces are:

- the root guest's own, of which it may have several, such as one below its children and one
  above;
- each instance's screens and anchored overlays;
- surfaces the host owns, such as program windows;
- later, web documents (iframes).

- **Native:** a wgpu pass in the host draws the layers. OS compositor planes may take it over
  later.
- **Web:** the page applies the list as CSS to the canvases it already places
  (`be-host-runtime/src/web/canvases.rs` and `overlay.rs`). A child's `OffscreenCanvas` goes to
  its worker when it is spawned, and the root guest names it only by handle.
- **Only the host composites app surfaces.** No guest reads another instance's pixels: on the
  web it could not, since each app worker has its own GPU device. A guest gets back only windows
  the host composited for it (Space and headsets). Nothing in the host blurs; an app that wants a
  blurred backdrop draws one from its own content.
- **Damage:** each surface reports damage rectangles and layers keep stable ids. The host
  transforms each surface's damage and adds the old and new bounds of every layer that moved,
  appeared or went, or whose opacity, clip or visibility changed, for e-paper's
  `FB_DAMAGE_CLIPS`.
- **OS windows:** the root guest opens, sizes and closes them (one per display, dock tabs floated
  out, popups that escape their window) with `WindowRequests`, and the host answers with
  `WindowReports`.
- **Offscreen windows:** the root guest may also open a window the host composites into a texture
  or a headset swapchain image instead of onto a screen. Only Space uses them.

## Input

The host routes every event to the surface it is for, from areas the root guest declares, as
beui routes inside a document today (decisions/beui_hit_areas_are_declared.md;
guides/beui_keyboard.md, One input path). It is one way for every surface, and the web forces it:
a parent can neither intercept nor deliver again an event inside a cross-origin iframe, so a
future iframe web app gets the real events the user started, which the browser grants rights from
(fullscreen, the clipboard, playing sound), rather than copies relayed through `postMessage`.

Each layer declares, beside its placement:

- its hit area: the whole surface, a region, or nowhere;
- its gesture area: where its placer's camera takes wheel, pinch and pan-drag first;
- whether it captures presses, as a scrim or a popup does, for the compositor to dismiss;
- whether a press focuses it (popups and the on-screen keyboard never take the focus);
- its placer, so a gesture reaches the right camera.

The host applies the rules beui applies to its forwarding catchers today:

- **Pointer:** one hit test against the last committed layer list, so a surface that moved this
  frame is hit where it was, as in a browser. Gesture claims go outermost first, as the camera's
  do now (guides/pan_and_zoom.md, Gestures): a pinch with one finger on a canvas and one on an
  editor placed on it zooms the canvas. A press captures the pointer for its surface until every
  button lifts. Hover and the cursor shape follow the surface under the pointer.
- **Touch:** a finger that lands for a gesture an outer claim takes, such as a pinch begun on an
  app, cancels the earlier fingers in the app and gives them all to the claimant.
- **Keys** go to the focused surface, except the chords the compositor declares
  (`WindowRequests::intercepted_keys` today), which go to it first. A press focuses the surface it
  lands on unless its layer says otherwise, and the compositor can move the focus (Tab leaving an
  app's last field, the palette closing).
- **IME:** the focused surface reports its field's area, which the host transforms by its layer.
- **Pointer lock** is granted only to the focused surface, and released when it loses the focus.
- **Accessibility actions** reach a surface by node id.
- **drive** presses at the window, so its input takes the same path.
- **Handing over a press:** the surface holding one may hand it to its placer (`pan_drag()`). The
  host cancels it in that surface and sends the rest to the compositor.
- **Drag and drop** between surfaces is a session the host owns: the source starts it with its
  payload, and the host sends drag-over events to the surface under the pointer and the drop to
  the one under the release.

The **forwarding router** is these rules, given a hit test, as a crate below beui. The host hands
it a 2D test over the layer list, and Space a ray test (Space and headsets). An app that places
children and wants to route to them itself can use it too. beui-core's routing inside a document
stays its own.

On the web, a focused cross-origin iframe keeps its keys, so the compositor's chords reach it only
once the user presses outside it, as in a browser. An iframe placed in a pan-and-zoom editor sits
under a shield the host lays over it: gestures and taps on it go to the canvas until a tap
activates it, which lifts the shield, and a press outside puts the shield back. An iframe that
posts its gestures back to the host could feel closer to native, but that is not planned.

## The sandbox API

A new WIT interface the host serves to the root guest, and later to apps
(plans/single_threaded_guests.md, step 1):

- `compile(bytes) -> module`, hashed and cached. It completes later: natively the host compiles
  on a thread of its own, and on the web with `WebAssembly.compile`, or in the spawned worker
  for a module only that worker runs, never with a synchronous `new WebAssembly.Module` on the
  page's thread. On the web the `module` it returns is the `WebAssembly.Module`, which the page
  posts to each worker it spawns.
- `spawn(module, config, imports) -> instance and channel`. The config sets the memory limit,
  threads and libraries.
- There are two ways to spawn:
  - **A staged app**, by app id. The host loads it with its manifest and binds the imports it
    serves to that app, as it does today.
  - **A module from bytes.** It gets only the imports its spawner serves.
- The channel speaks the existing app protocol (`be_wasm_guest_host::Channel`), with the spawner
  as the other end.
- On the web, spawning makes a Web Worker from the page.
- Guests run on V8 natively and in the browser, neither of which offers fuel or canonical NaNs
  (decisions/guests_run_on_v8.md), so a guest that must be deterministic stays on wasmi; a hung guest is
  stopped by terminating its isolate.
- It is one async interface for every caller, its calls async ops like every other guest call
  (decisions/guest_calls_are_async_ops_except_local_ones.md). The root guest awaits them from its
  resumable frame, which returns `Pending` until they complete. The root's frame entry is an
  export the host calls, not a guest call, so that decision does not cover it.

Built natively (steps 1 and 2): the WIT is `be3:sandbox` (`crates/be-wasm-sandbox-api`), where
`spawn` is the `instance` resource's constructor, taking a source, `module(module-id)` or
`app(app-id)`, and the channel its `send` and `next`; `be_wasm_host::sandbox` does the work and
`crates/be-wasm-sandbox-host` serves it to a guest (guides/apps/runtime.md, Backends). A compile
turns the memory a module defines into an import, so one compile serves instances given different
memory limits. A staged app is found by a loader the host gives the `Server`
(be-host-runtime's `staged_app`), and runs with its libraries and WASI but no display. The web's
half is not built: it lands with the first guest on the web that spawns (step 4, or
plans/single_threaded_guests.md).

An app's platform calls (clipboard, drag and drop, file dialogs, opening URLs) are served by the
host directly for that app, and the compositor can turn them off for it.

Other users:

- The block stack spawns type modules itself: the root guest grants it the sandbox API for
  modules from bytes. Each type module is an instance of its own, natively on a thread of its own
  and on the web in a worker, as be-block-type-host's V8 and web worker sources run them
  today (guides/type_modules.md).
- The Games app keeps running game modules on wasmi inside its own module (tabletop-host), since
  neither V8 nor the web has canonical NaNs.
- The input remapper (`be-host-remap-host`) runs through it (step 1).

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

## Space and headsets

Space (`beui-layer-space`) moves into a guest of its own. While it is on, it is the root and drives
the compositor. Neither the compositor nor any app knows about VR.

- **Driving the compositor:** they are two instances, and the host runs both on its frame
  thread. Space's frame asks for the compositor's frame through an import, an async op like
  any other, and returns `Pending` naming it. The host then calls the compositor's resumable
  frame export, which may itself return `Pending` while it waits for its children, and calls it
  again as they answer. When the compositor finishes, the host resumes Space's frame with the
  result. All of it happens within one host frame, so a compositor change still reaches the
  headset in the frame that made it.

- **Windows:** Space serves the compositor the same window interface the host serves, as its beui
  `Layer` does today (`WindowHosting::Any`): each window the compositor opens becomes a panel, and
  a dock tab popped out becomes a panel rather than an OS window. The simulator is one real
  window.
- **Pixels:** for each window the compositor opens, Space opens an offscreen window of the
  host's and answers the compositor with that window's id, so the ids the compositor uses are the
  host's own. The compositor returns its layer list for that window to the host, as for any
  window, since the host ran its frame. The host composites the list into the panel's texture,
  only when it has damage, and hands Space the texture by handle on the host's device. A panel
  the headset shows as a composition layer is composited straight into that layer's swapchain
  image, so nothing is drawn twice. Space draws the other panels into the eye
  buffer from their textures, with mipmaps, so distant text doesn't shimmer. It never reads an
  app's surface itself. WebXR is not part of this plan
  (plans/deferred/vr.md, M11).
- **Input:** Space's window takes all input, with the headset's controllers. Space ray-tests its
  panels for a window and a point, runs the forwarding router with a 2D test over that window's
  layer list, and has the host deliver each event to the surface it picks. Text comes from beui's
  on-screen keyboard, since the OS input method cannot show in a headset.
- **The host's part:** OpenXR, as beui-layer-xr without its `Layer` wrapper: the frame loop and
  poses, swapchains, and submitting the quad, cylinder and projection layers Space describes.
- **Menus and popovers** are windows of their own already (roles `Popup` and `Keyboard`,
  guides/beui/space.md), so Space places them as panels of their own. Drawing a window's layers
  apart in 3D is not planned.

## The block stack

The block stack runs on its own thread, as it does natively today (`be-host-stack/src/worker.rs`,
the peer's loop). The root guest spawns it through the sandbox API as an instance of its own, with
its own memory, so nothing is shared between it and the compositor: they talk only through messages,
the commands and change notices they use today (`ChangeWatch`). Neither ever spins. The block
stack's thread waits with `memory.atomic.wait`, which a Web Worker allows, and the compositor's main
thread never waits. The web's stack worker (`web/be-worker.js`) already has a memory of its own and
talks to the page by messages, and the page takes only std's locks, which it never finds held
(decisions/the_web_page_never_waits.md); this step makes that worker an instance the root guest
spawns, natively too. The host does not know what the instance is; it only serves the capabilities
the root guest grants it. If it traps, the root guest ends the app, as a panic does today.

be-block-client, be-host-base and be-host-stack already run as wasm on the web: SQLite on OPFS,
and the browser's WebSocket and `fetch`. They gain a backend over the host's capabilities and drop
the native tokio, rustls and tungstenite path, so each has one transport.

- **Files:** a WASI preopen natively, OPFS on the web.
- **Network:** WebSocket and HTTP. Natively the proxy credential header is allowed; the web
  refuses it, as a browser does.
- **Apps' block calls** reach the root guest, which passes them to the block stack as messages,
  so a slow query never holds up a frame. The block stack keeps the per-app rules the host
  enforces now.
- **Local accounts** need no server (guides/hosting.md, Local accounts and the default server), so be-server leaves be-host.
- **Without a UI:** for Android push and background sync the host starts the root guest through
  an entry point that spawns only the block stack and starts no UI. Measure its memory.
- **Smaller updates:** the web's page gets the block stack's state today as a copy of the
  worker's `Seen`, and an update sends whole what is not a persistent map, and a map's changed
  entry whole: an open block's `Content` on each of its edits, the outbox and the owing lists
  on each change, and an editor's queued frames. Whatever the root guest and the block stack
  exchange instead, send content as its edit (the block's own operations or a byte delta), or
  keep a value's `Arc` identity across updates so one the block stack did not replace is never
  sent again.
- **Native builds:** the crates stay portable, so their native tests, be-server and the CLIs keep
  building natively.

Costs:

- **Build time:** `./scripts/verify` gets slower, since the block stack and SQLite are compiled to
  wasm on top of the native builds the tests need.
- **Run time:** wasm runs roughly 30% slower, which native guests could later win back.

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
- **The HTML core:** on the web the compositor and shell still draw to canvases, as be-host does
  today. Moving them onto beui's HTML core (VISION.md, beui on the web is the DOM) is left to a
  later plan.
- **Starting early:** be-host starts a restored workspace's apps before its window
  (decisions/host_startup_overlaps_app_loading.md). Once the root guest spawns apps, the host
  starts the root guest that early too, so it can spawn them before the window opens. Until
  the window and the GPU exist, the root guest can only spawn and link apps: it cannot make
  surfaces or draw.
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
- decisions/app_frames_split_into_layout_and_present.md (the compositor runs the rounds in its
  resumable frame, not the host)
- decisions/beui_space_keeps_one_renderer_per_texture.md (the host composites Space's panels)
- decisions/host_startup_overlaps_app_loading.md (the root guest starts apps, so it starts before
  the window)

## Steps

Each step lands on its own.

1. Built: the sandbox API for modules from bytes, natively, and the remapper on it (its fuel
   became a time limit, past which its isolate is terminated).
2. Built: spawning staged apps by app id, with their manifest's libraries and WASI and no
   display.
3. The layer list and the forwarding router, and how native and the web realize them, with the
   host's current UI as their only producer, so nothing changes visibly.
4. The synchronous, resumable root entry. The compositor takes over `src/compositor`, `src/ui`,
   the camera and the orchestration. Embedding an editor without the host's compositor starts
   here, once the layer list can show a surface a guest names: it needs the host side of the
   app protocol in a guest (configuring screens, the editor's watches, a block's content;
   be-sdk-test's `ContentStore` and `link.rs` already run in wasm, in process), and a surface on
   the host's device that the spawner names only by handle (`App::attach_surface`).
5. The block stack as an instance the root guest spawns, behind capabilities, building on the
   web's stack worker with a memory of its own, and sending smaller updates.
6. Space as a guest, offscreen windows, and the host's OpenXR part, with late latching.
7. Strip the host: drop beui's widgets, text and fonts, the block stack, be-server and audio
   decoding. Update guides/apps/runtime.md, guides/desktop.md, guides/running/web.md,
   guides/running/drive.md, guides/beui/space.md, guides/pan_and_zoom.md and
   guides/testing_a_gui.md.
