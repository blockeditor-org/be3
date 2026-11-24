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
including all of beui, the block stack with SQLite and rustls, symphonia and
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

Built natively (step 4a): the entry is `be3:root` (`crates/be-wasm-root-api`). The root exports
`frames.frame`, an async op the host starts and steps the root through until it answers, and
imports `host.deadline`, which answers once the frame's deadline passes, so the root races its
children against it. A step that leaves `frame` unanswered is `Pending`, and what it waits for is
the ops it left open. `crates/be-wasm-root-host`'s `Root` runs the root on the calling thread
(`be_wasm_host::App`, stepped inline), serves it the sandbox and `deadline`, and between steps
waits on a bell its instances, compiles and own wakes ring, up to the deadline, then up to
`STEP_TIMEOUT` past it for the root itself. `frame-done` carries the layer list in be-lib-layers'
encoding (`LayerList::encode`). `crates/be-wasm-root-testing` is the root its tests run, which
settles one echo child.

Built on the web (step 4d): the same crate's `web` half instantiates the root on the page from a
module compiled with `WebAssembly.compile`, and steps it inline with the stepping natively shares
(`Core`). `Root::frame`, called from `requestAnimationFrame`, steps the root as far as it goes, for
4 ms of the page's thread at most, and returns; the root's `Pending` is a microtask the page queues
whenever an instance's worker answers, a compile finishes or the root wakes itself, and a timer at
the deadline and at the patience past it, and a root with more to do past its 4 ms resumes as a
task of its own, after the page's input and rendering. The drawn frame is handed to a callback. `crates/be-wasm-root-web-testing` runs 4a's root
so in Chromium. be-host runs one in place of its own UI with `--root=APP` (Steps, 4e5c).

Built (step 4e5a): the root's `input` export, `input.deliver(window, events)`, the window's input
since the last call as be-lib-ui-types' `input::encode` (be-wasm crates may use only be-wasm
ones, so it is bytes, as the layer list is). `Root::input`, natively and on the page, starts it,
and the root takes it in with the frame being drawn, or else the next one, so it routes by the
layers it drew last, and answers once it has routed. be-wasm-root-testing's root routes it with a `Router` per
window over that list (be-sdk-root's `Windows`, since 4e5b), and sends its child what lands on its
screen, in the child's coordinates.

Built (step 4e5b): `crates/be-sdk-root`, what a root links to embed apps (Steps, 4e5b).

Its rules:

- It is be-host's own module, staged with it, never one from user bytes.
- It never blocks its main thread: no waiting on atomics or on locks another thread may hold.
  These are the browser main thread's rules, which beui already follows.
- Its own calls have no deadline; only waiting for children does (`FrameClock::deadline`). A slow
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
- be-root-accounts-ui.
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

Built (step 3): the list is `be-lib-layers` (`LayerList`, `WindowLayers`, `Layer`), naming
surfaces by handle (`Surface::Root`, `Screen`, `Anchored`, `Host`), with the transform from a
surface's pixels to the window's points, the clip in the surface's pixels, the rounding of the
surface's own corners, and `WindowLayers::damage`. It serializes (`LayerList::encode`), as the
root's frame carries it (step 4a). The host's own UI is its only producer
(`be-host-runtime/src/layers.rs`); the web builds the frame's list (screens under
`Surface::Root(0)`, anchored views and what lies over them above it) and applies it as CSS to
its canvases.

Built (step 4b): natively each window is drawn from its list in a pass of its own
(`beui_draw::compose`, beui-renderer-wgpu's `composite.rs`; guides/apps/runtime.md, The layer
list). The window's renderer paints the host's UI into textures, `Surface::Root(0)` and one above
each anchored view it shows (`Root(1)` above the first), for what is painted after that view. Drawings list the
surfaces they show (`Draw::layers`), program windows as `Surface::Host` (Wayland's `WindowDraw`,
win32's `TextureDraw::hosted`), one list per renderer, so per OS window and per display, with no
`window` set yet. Screens and program windows are still drawn in place in the UI's paint order,
so nothing changes visibly: they blend over the UI beneath them and inside its groups and
filters. The pass composites only the roots and the anchored views between them, in paint order, and draws
again only where `WindowLayers::damage` says, which is the e-paper display's damage.
`texture_quad` takes a radius per corner, and restacking damages only where two layers that
swapped order overlap. Cost: a texture the window's size, and one more for each anchored view
that shows, and a pass over the damaged area drawing one quad per root and anchored view; a
frame also walks the displays that hold drawings and their ancestors (on the CPU), and prepares
the drawings that were damaged or moved. Keeping the list across frames and patching it from the
tree's changes would remove the walk.
Known limit: an anchored view nested in a group or filter only avoids the worst case. The roots
split after the group, so the view ignores the group's opacity and filter, and what the group
paints after it lands below it; the renderer says so once on stderr. The compositor in the root
(4g) should place anchored views outside groups, or the list grow a group of its own.
Compositing a screen apart from the UI, rather than in place, waits for the root guest (4g),
which decides what lies under its screens (a hole, as on the web, or a root split around them).

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
  (`Router::intercept`), which go to it first. Since the root takes every raw input before beui
  sees it, the compositor gives its own router its chords, and be-host keeps no list of them:
  `be3:root` carries none. A press focuses the surface it
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

Built (step 3): the router is `be-lib-input-router` (`Router`, over a `HitTest`), and
`WindowLayers` is its 2D test (each layer's `LayerInput`: hit and gesture areas, `captures`,
`focuses`, `placer`). It routes pointer capture and hover, gestures outermost first, a finger
landing for an outer claim, keys with the compositor's chords, focus on press, the pointer lock,
handing over a press and drag and drop.

Built (step 4c): the router is the only place these rules live. beui-core's forwarding catchers
route through it (`interact::forward`): each pointer event asks it who holds the pointer
(`Router::press`, `moved`, `release`, `cancel`, one hold per pointer and button), a wheel or
touchpad gesture who takes it (`gesture`, a held one latched to the catcher it began on), and
the end of the frame who heard the pointer leave and the focus move (`entered`, `hover`,
`focus`). Its hit test (`forward::Catchers`) is beui's own: the catcher beui's hit test finds
under the point, after its overlays, scrims, claims and the camera's gesture areas, and whether
a middle press pans (`HitTest::pans`). `Router::route` builds the same steps into routing a
window's raw `Input`, for the host once it routes between surfaces. Where the router's rules and
the catchers' differed, the catchers' were kept, since every host test relies on them: a
mouse's or pen's press holds its catcher for its other buttons, and each finger is its own
pointer. The one change is latching: a held scroll or touchpad gesture now stays with the catcher
it began on. (A key's release went to the focus then; since 4g4's review it goes where its press
went, the compositor for a chord it took, as beui holds a key for what took its press, so the
release of an Escape that closed a root's dialog never reaches the surface the focus went back
to. Only `Router::route`, the root's routing, routes keys.) The IME area, accessibility actions by node id and drive's presses already take
this path, since they reach catchers through beui's document.

Built (step 4f1): beui hands a program the platform's raw input (`Program::input`), and
be-host's `--root` hands all of it to the root, which routes it with `Router`; the root names
the IME area of its focused surface (`frame-done`'s `ime`).

Built (step 4g1): what the root asks of the window and what its input carries. `frame-done`
asks for every sample of the pointer's moves, a cursor shape (by its CSS name) and the pointer
lock, which be-sdk-root answers from what its apps' screens ask (`Embedding::requests`: the
cursor of the screen under the pointer, the lock while the focused screen holds it, every
sample while a screen shown wants them) and be-host sets as beui's window requests; each input
`input.deliver` carries has the frame clock's time it was sampled at, when the platform said
(`Program::input` gained it), which an embedded app hears as its input's time.

What is left, with the compositor in the root (4g): the host's UI routes through the root's
`Router` as each area of it moves in, each declaring its layers' `LayerInput` (an app view's
hit and gesture areas and placer, a scrim's `captures`, a popup's `focuses: false`) and the
chords it takes along the focus (its actions, the palette, Tab and F6 between surfaces) as the
router's intercepted chords. The host's own beui UI is never routed through `Router` while it
still lives in be-host (4f2 merged into 4g).

After a resize, input is routed against the list drawn at the old size until the next frame
draws, though a press already held still reaches its holder.

On the web, a focused cross-origin iframe keeps its keys, so the compositor's chords reach it only
once the user presses outside it, as in a browser. An iframe placed in a pan-and-zoom editor sits
under a shield the host lays over it: gestures and taps on it go to the canvas until a tap
activates it, which lifts the shield, and a press outside puts the shield back. An iframe that
posts its gestures back to the host could feel closer to native, but that is not planned.

## The sandbox API

A new WIT interface the host serves to the root guest, and to apps for their workers
(guides/adding_an_app.md, Workers):

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
`app({app, module})`, and the channel its `send` and `next`; `be_wasm_host::sandbox` does the work
and `crates/be-wasm-sandbox-host` serves it to a guest (guides/apps/runtime.md, Backends). A
compile turns the memory a module defines into an import, so one compile serves instances given
different memory limits. A staged app's module, its entry point or one of its workers, is found by
a loader the host gives the `Server` (be-host-runtime's `staged_app`), and runs with its libraries
and WASI but no display. Every app is served one already, for its workers, which spawns only its
own app's modules and compiles none (guides/adding_an_app.md, Workers).
`crates/be-wasm-root-host` serves the root one that may spawn any staged app (step 4a). On the web
the same `Server` spawns an app's workers, each in a web worker of its own, and compiles a module from bytes on the page with
`WebAssembly.compile`, once per page for the same bytes, with its memory imported as natively,
refusing one that imports more than its transport; each instance of it is a plain worker the page
posts the `WebAssembly.Module` to, with a memory capped at the instance's limit (step 4d). The
browser cannot cap a staged app's worker's memory.

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

Built (step 4e1): natively a staged app the root spawns draws on the host's device, so what it
presents never leaves the GPU. `be_wasm_host::sandbox::Config::display` gives it the device, which
the root's sandbox passes (`Server::with_display`, from `Root::start`) and an app's sandbox never
does. Each `Instance` keeps the texture each of its surfaces presented last (`Presenting` reports
them as it steps, `PresentedTexture`, as it does for be-host-runtime's apps), and the root names one
in its layer list as `Surface::Screen { instance, surface }`, `instance` being its `instance`
resource's op, which `Root::surface` resolves on the host's side.

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
3. Built: the layer list and the forwarding router, and how native and the web realize them,
   with the host's current UI as their only producer, so nothing changes visibly (The layer list
   and Input say what is left for step 4).
4. The synchronous, resumable root entry and the compositor in it, in sub-steps that each land
   on their own and keep be-host working, with nothing changing visibly:
   - 4a. Built: the root entry natively (The root guest): `be3:root`, `Root` running a root
     inline and serving it the sandbox and the frame's deadline, and the layer list as the
     frame's answer, tested with a root that settles one child. be-host runs no root yet.
   - 4b. Built: the host draws the layer list in a pass of its own: its UI painted to a texture as
     `Surface::Root(0)` (and `Root(1)` for what lies over anchored views, as the web stacks
     `#layers`), app surfaces composited from the list, one list per OS window, program windows
     as `Surface::Host`, the e-paper damage from `WindowLayers::damage`, a radius per corner in
     the texture quad, and restacking damaging only the pairs that overlap and swapped order.
   - 4c. Built: the router replaces beui's forwarding over beui's catchers (Input, Built
     (step 4c)): beui-core's forwarding catchers route through `Router`, with beui's hit test
     as its `HitTest`, so its rules live in one place; nothing changes visibly.
   - 4d. Built: the web's half of the sandbox and of the root entry: the page compiles with
     `WebAssembly.compile` and spawns a worker per instance, and calls the root inline from
     `requestAnimationFrame`, resuming it as its workers answer; tested with 4a's root in
     Chromium.
   - 4e. A surface a guest names, and the app host in a guest, in sub-steps that each land on
     their own:
     - 4e1. Built: a surface a guest names, natively. A staged app the root spawns draws on the
       host's device (`Server::with_display`, which only the root's sandbox has; an app's
       workers still get no gpu), and its presented textures are named to the root only by
       handle: `Surface::Screen { instance, surface }`, whose `instance` is the op of the
       root's `instance` resource, which both sides know it by, and which `Root::surface`
       resolves to the texture on the host's device. The web's half, a child's
       `OffscreenCanvas` named by handle, waits until the page runs a root.
     - 4e2. Built: the host half of the app protocol as a library, `be-sdk-app-host` (no beui
       or block stack, denied both in buck/cargo/layers.bzl; in the be-sdk family, since a
       be-lib crate may not depend on be-sdk-api): be-host-runtime's `Wire`, `calls`, `sent`,
       `session`, the docks' state (`Docks`; be-host-runtime keeps the one it holds) and the
       published interfaces' lookups moved into it, with the platform's be3:desktop offer
       passed in (`Wire::offering`), and be-host-runtime uses it. A sandbox instance keeps a
       surface's texture only until its app lets the surface go: the gpu host reports what was
       released (`Produced::released`). A be-sdk app lets go of every screen surface its new
       layout no longer names (`Surfaces::place` calls `release_surface`), so its instance holds
       only what its layout names; any other guest is trimmed only as far as it drops its own
       surfaces, and then holds no texture its guest does not.
     - 4e3. be-sdk-test on the library, and an editor's watches and a block's content in it, in
       two sub-steps:
       - 4e3a. Built: be-sdk-test's in-process host (`BeuiTest`) runs the editor's wire as
         be-host's runtime does (`Wire`), so its own host-side connection goes: what the wire
         serves itself (published, child-status, dock, block-type and web-view watches, the
         block watches' `next`s, replica streams and sends) it no longer duplicates, and it
         answers the rest from what the wire hears (`Heard`), through the wire's own calls
         (`answer`, `answer_watch`, `relay`, `accepted`, `stream_item`) and at once, as
         be-host's runtime answers them. `Wire` takes any `Sandbox` (be-host-runtime's
         `AppSandbox` over `Server`, an editor test's in-process workers) and tells one published
         watch its value (`set_published`). The generated calls name their op (`Call::op`).
         `link.rs` stays be-sdk-test's store, now answering through the wire.
       - 4e3b. Built: the store trait: be-sdk-app-host gains `blocks`, the host side of
         `be3:blocks` (`Subject`, a watch's dedup, a dispatcher, `call`, `ended` and `answers`,
         over a `BlockStore` trait, and the answers to watches that wait), and every
         `be3:blocks` call goes through it. be-host-runtime's `Instances` implements it over
         be-host-stack, keeping its scope, permission and event policy in its implementation
         (`admit` translates and refuses before dispatch), and be-sdk-test's `link.rs` over its
         `ContentStore`, so its own watch bookkeeping goes. Where it differs: the watches and
         replicas an editor holds are a library type (`EditorBlocks`) that the store keeps where
         it likes (be-host on each instance's entry) and hands the dispatcher (`held`), and the
         store says which instance holds an op (`holder`), since be-host already keeps that map
         for its previews. Two replicas of one block in one editor: frames go only to the
         latest (a host holds one participant per editor and block, so fanning out would mix
         two replicas' acknowledgements), and when it ends the earlier is started afresh from
         a snapshot (`replica_resumed`); a replica ends only once no replica holds its block. be-sdk-test's watches now drop a value they were
         answered already, as be-host's did.
     - 4e4. be-host-runtime's orchestration in the library, with be-host-runtime driving it, in
       sub-steps:
       - 4e4a. Built: pacing and settling, `be_sdk_app_host::pacing`. `Pacing` decides when an
         app is asked for a frame (`ask`, which records the request and says whether to send
         the `Draw` now, later, or once the app's input is taken in) and what its layouts and
         presents count for (`laid_out`, `presented`, `answered`); `FrameClock` keeps the
         display's presents and the host's composites, gives one deadline per host frame
         (`deadline`) and settles against it (`settle`, over a `Settling` the host implements
         with its runtimes and its bell), recording the time waited and the frames over
         budget; `held_behind` orders placers before their children, over any id. The library
         reads no clock: real time comes from the host as a `Duration` since an origin it
         chooses (`Settling::now`, and the deadline it hands back to `Settling::wait`), so
         wasm32 hosts and tests drive it. be-host-runtime keeps which runtimes are asked, their
         placers, its bell and its real-time origin, and sends the `Draw`. Its pacing
         tests moved with it. No geometry was involved.
       - 4e4b. Built: the layout and present holds, `be_sdk_app_host::holds`. `Holds` keeps
         what of a layout commit waits for the present that follows it (`heard_layout` splits
         the commit, with the host saying which children only move; `Laid`), the answers heard
         after it, and the counts that release them (`release_presented`,
         `waits_for_children`); `Shown` keeps the anchored views a layout closed until the
         frame that closed them is drawn (`place`, `closed`) and the last frames of views left
         out (`keep_left_out`, `release_kept`, `MOST_KEPT`), for a host that holds the
         textures; a present taken in (`presented`) or a frame shown (`frame_shown`) says what
         may go (`Release`), rather than the host asking; `Presents` the damage of the
         presents since the last frame shown. be-host-runtime drives them, and keeps the frame
         held for children, the presenter's state and the instances they consult. These use no
         geometry either, only be-sdk-api's placements.
       - 4e4c. Built: grafting the accessibility trees, `be_sdk_app_host::accessibility`.
         `Graft` (beui-core's `Embedded`, moved) keeps one screen's AccessKit tree as its app
         describes it, cuts it into a tree, renumbers it into a document of the host's ids
         (`DOCUMENT_SHIFT`, which be-host-runtime checks against beui's at compile time), and
         sends what changed under a pane placed where the screen is drawn (`grafted`, from a
         `Placing` of be-lib-geometry's `Rect` and `Vec2`; `Grafted`), taking the host's ids
         back to the app's for actions (`app_node`). be-host-runtime keeps one per screen,
         makes beui's `Fragment` of what it sends (`Fragment::new`) and publishes it, and keeps
         the actions, titles and which screens report. A restart now empties each screen's
         pane until the new run describes it (`restarted`), where before the old run's tree
         stayed. be-lib-geometry needed nothing new: the graft makes AccessKit's `Affine` and
         `Rect` itself. Its tests moved from beui-host, checking the nodes rather than a dump.
       - 4e4d. Built: restarts, `be_sdk_app_host::run`. `Run` is one run of an app as its host
         keeps it: the session, the calls queued for it, the holds, the shown frames, the
         presents, the pacing, the screens and paintings asked for, and what it was told
         (`tell`: the fonts once, then whether to describe, the preferences and the UTC offset
         whenever they change, all but the preferences once it runs). `restart` is the one call
         that says what a restart keeps, resends and drops, given the new session, the host's
         `Docks` and what its views described (`Descriptions`, which be-host-runtime's
         `Instances` implements over each screen's `Graft` and each view's actions); the pieces'
         own `Holds::clear`, `Shown::clear`, `Pacing::reset`, `Docks::restarted` and
         `Graft::restarted` are the library's own now. be-host-runtime keeps one `Run` per app
         and drives it. What stays with the host: the wire, which lives with the backend because
         it is bound to the worker and sandbox the backend starts and is made afresh with each
         start; the presenter's textures, the frame held for children and the surfaces drawn,
         which hold the GPU's; the font fallbacks, which read the host's fonts; and
         `Instances::reopen`, which takes back what the block stack, the input forwarders and
         the revision previews gave the old run. The library tests a restart over a `Run` and a
         stand-in for the host's views; be-host-runtime's restart tests stay, since they test
         what `Instances` sends again, and restart through `Run::restart` as `Runtime` does.
         Left for 4e5, so a root built on the library does not copy them, and done in 4e5a:
         `Runtime::layout` and the pattern of sending calls and asking for a frame if any were
         sent.
     - 4e5. A root built on the library embeds an editor without the host's compositor, shown by
       a host test, and the root gets its `input` export, routed by `Router` over its last
       list. be-host runs it through a run option that starts a root module in place of its
       own UI: the path 4g turns on and makes the default, not a test-only fork 4g replaces. In
       sub-steps:
       - 4e5a. Built: what a root must not copy, and the root's input. `Run` keeps the app's
         layout (`Run::layout`, which `place`, `lay_out`, `placement` and `retained_surfaces`
         read with `Shown`; a restart empties it), and sends: `send_for_frame` asks the app for a
         frame if it sends anything, readied for the wire by the host's closure (be-host
         translates ids and holds what waits for a report, or drops all of it for an app that
         failed), `queue` sends without asking, and both answer what to send now, everything
         queued once the session runs (`deliverable`). be-host-runtime's `need_if` and
         `send` pairs are its `send_for_frame`. `be3:root` gains `input.deliver(window,
         events)`, the window's input as be-lib-ui-types' `input::encode` bytes (be-wasm crates
         may use only be-wasm ones, as the layer list's bytes already are), which `Root::input`
         starts natively and on the page; the root takes it in with the frame being drawn, or
         else the next one, so it routes by the layers it drew last, and answers once it has
         routed (the host keeps each call until then). be-wasm-root-testing's
         root routes it with a `Router` per window over that list and sends its child what lands
         on the child's screen, in the child's coordinates; its test presses on the child and
         drags off it, and the child hears both, the drag held by the press.
       - 4e5b. Built: a root built on the library embeds an editor. `crates/be-sdk-root` is the
         library a root links (a be-sdk crate, since be-sdk-app-host is one; a root linking it
         is an app's module or a be-sdk test crate's, so the be-wasm layer rule needed no
         change, and like be-sdk-app-host it may not use beui or the block stack). For each app
         it spawns, `Embedded` holds a `Wire` over the instance's channel, which a task on the
         root's executor hears, and a `Run` with its session, pacing and holds: it queues the
         editor's opening and `Screens` until the hello, asks for a frame when the pacing says
         one is due (`Embedded::frame`, which the root races against `host.deadline`; one still
         in flight is waited for), and names each screen it painted by handle in its layer
         list (`Embedded::layer`, scaled from the surface's pixels to the screen's points). The
         pacing alone says whether a frame is in flight (`Pacing::in_flight`), so one unanswered
         past `FRAME_TIMEOUT` is asked again. When the last `Embedded` of an app goes
         (`Embedding::remove` lets go of the embedding's), the task hearing it lets go of the
         instance, which stops it, even if the app sends nothing more.
         `Windows` keeps a `Router` per window over the last list; input for a window the list
         leaves out routes against the layers it had last, and one never drawn against none,
         never dropped, and a window's router goes once the list leaves it out and no press on
         it is held (`Router::holding`), and the surface it had focused hears `Focus(false)`.
         `Embedding` forwards what lands on a screen as `AppCall::Input` in the screen's points
         (a screen that went still hears what ends a press or the focus, so its app holds
         nothing pressed), and answers what lands on the root's own surfaces. It refuses at
         once what it serves no app yet (block calls, host services, the network, debug,
         published calls and the shell but `tell`; `Wire::refuse_unserved`), and restarts
         nothing: an app that stops reports why (`Embedded::failure`). `Wire::send` now sends
         `Outbound`s for every host. be-sdk-root-testing is a root built on it; be-wasm-root-host's
         test spawns the triangle (whose `connect` reads nothing) on a stand-in device and shows
         its screen after the hello, `Screens`, `Draw` and the commits, by the handle
         `Root::surface` resolves to the texture it presented, at the size asked for and at a
         raster of 2 (the layer scaling it by half), answers the next frame at once with no
         frame due, and stops the app once the root lets go of it. `Embedding::input` reaching
         an app is tested only through `Windows` until step 5 gives an editor content that
         answers input visibly.
         be-wasm-root-testing's root routes with `Windows` too. An editor that answers input
         visibly needs content, so a store in the root (be-sdk-app-host's `BlockStore` over type
         modules it spawns) waits for step 5; until then 4e5's "input routed by `Router`" is
         shown only on be-wasm-root-testing's root. Left for later: the fonts, preferences and
         UTC offset an app is told (`Run::tell`), children an embedded app places (the holds'
         layouts are let go unapplied), restarts, and the workers an app starts (its wire
         serves it no sandbox).
       - 4e5c. Built: be-host's run option that starts a staged root module in place of its
         own UI, `--root=APP` (`crates/be-host/src/root.rs`), on be-host's startup path and
         window (`beui_host_window::run_program`, as its own UI runs). `APP` is a staged app whose
         manifest says `"root": true` (`AppManifest::root`: no editors, block types or host
         components; decisions/a_root_is_a_staged_app_flagged_root.md), whose entry point
         loads as a pure guest and starts as `Root` on the window's device, spawning the apps
         it embeds as be-host finds every app (`staged_app`). Each host frame hands it the
         window's input since the last (`Root::input`; through beui's catcher and `Replay`
         until 4f1 gave it the platform's own), asks it for a frame by the deadline be-host's own UI keeps
         (`FrameClock`, so with drive's clock stopped `STOPPED_CLOCK_FRAME_BUDGET`), and draws
         each `Surface::Screen` or `Anchored` of its list from the texture `Root::surface`
         resolves, with `compose::quad` in a drawing filling the window; the root's own
         surfaces are the window's plain background. A root that fails to start or stops
         closes the window, and be-host exits saying why.
         The host draws on demand: `frame-done` gained `next`, the frame clock's time the root
         wants its next frame for (none while it wants none), and `owed`, whether it owes work
         now, which be-sdk-root answers from its apps (`Embedding::wants`: an app starting or
         shutting down owes its hello or its going and wants a frame by `FRAME_TIMEOUT`; one
         that runs and draws owes a frame due or in flight, wants one at once while one is
         due or it animates, and otherwise when its pacing says; one with no surface, closed
         or stopped wants and owes nothing). `Root::on_wake` tells the host whenever the
         root's bell rings between frames, so what a child sends is stepped at once. be-host
         asks for the repaints `next` says and is busy only while the root owes something, so
         drive settles on an animating root as on an animating app.
         The host test `a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds` stages
         be-sdk-root-testing as a root (an `APPS` entry may name a package's label) with the
         triangle, drawn through lavapipe, and reads the window's pixels (`HostTest::shot`):
         the background where the root drew nothing, and the triangle's colours where its
         screen lies.
         `--root` runs none of the host's core: no data directory or its lock against a second
         be-host, no block stack, workspace or accounts, no screen lock, no OS services
         (be-host-wayland, be-host-services) and no drive commands beyond beui's own. 4g must
         bring back under this program what the host keeps (What the host keeps) before it
         makes the root the default; until then `--root` is refused as a login session
         (`BEUI_PLATFORM=drm`), which would have no lock screen.
         Left for later:
         - A painting of what the root shows (Other moves, Paintings), so the test compares
           pixels rather than a snapshot.
         - The frame clock measures no composite time (`FrameClock::settle` is never called),
           so its deadline assumes the default.
         - A replaced device stops the root rather than moving it, so a replaced renderer
           (device loss) quits be-host under `--root`; and no host ships a root.
   - 4f. The router over the layer list:
     - 4f1. Built: raw input to the root. beui's `Program` gained `input(window, &Input)`, which
       the runner offers each of the platform's inputs, in its window's points, after the
       layers' and before the intake makes the document's events of it, on every platform the
       runner drives (winit, the web, Android, DRM, headless, drive's taps); what the program
       takes no document hears. be-host's `--root` takes all of it and hands it to the root by
       window (`Root::input`, the main window as none), so the root routes the platform's own
       input with `Router`: IME pre-edit and composition, raw touches and pens, hover and
       presses held outside the window, every window's. Its catcher, `Replay` and focusing the
       catcher on show went; keys the root does not route reach nothing in be-host, and beui's
       own chords (the inspector's) do not reach beui under `--root`. `frame-done` gained
       `ime`, the input method's area in the window's points (`ImeArea::encode`), which
       be-sdk-root answers from the focused screen's window requests through its layer
       (`Embedding::ime`, `Windows::focused`) and be-host sets as the window's. The host test
       `a_root_in_place_of_the_hosts_ui_hands_its_editor_keys_presses_and_touches` presses,
       types and taps on the triangle's screen, which tints itself by the input it heard
       (`Heard::tint`), and beui-host's
       `a_program_takes_the_platforms_input_before_its_document_hears_it` tests the hook;
       be-sdk-root tests the IME area's mapping (`ime_in_window`) and the focus, but no test
       shows an app's field reaching the window, since no app a root embeds edits text before
       step 5. (Its inputs' timestamps and every sample came with 4g1.)
     - 4f2. Merged into 4g. It would have routed be-host's own beui UI through `Router` over
       the last committed `WindowLayers` while that UI still lives in be-host, which 4g then
       deletes, so most of it would be thrown away. Instead each area of the host's UI declares
       its layers' `LayerInput` (hit and gesture areas, `captures`, `focuses`, `placer`) and the
       chords it takes as it moves into the root, whose `Router` already routes all raw input
       (4f1).
   - 4g. The compositor moves into the root: `src/compositor`, `src/ui`, the camera and
     be-root-accounts-ui, talking to the native block stack by the messages it uses today
     (`ChangeWatch`) until step 5 moves the stack. drive's commands split as The root guest
     says. be-host runs the root through 4a's entry, draws its layer list (4b) and routes by
     its declared input, and its own beui UI goes. Each area brings its declared input as it
     moves (4f2 merged into this step). In sub-steps, an area at a time:
     - 4g1. Built: what the root needs before any of the host's UI can move into it.
       - The root's own surfaces. A pure guest may be given the host's device
         (`be_wasm_host::Host::with_display`; be-wasm-host offers it the gpu imports then,
         never fast ones, since its memory is its own), and the root draws on it as an app
         draws its screens, through `be3:gpu`'s surfaces; its layer list names each as
         `Surface::Root(id)`, which `Root::own_surface` resolves to the texture it presented
         last (its generation changing only when it presents again), and be-host's `--root`
         draws it as it draws a screen. `Root` keeps what each own surface showed when last
         presented, as the sandbox keeps a child's, so one configured afresh (resized) shows its
         last frame until it presents again, and one never presented shows nothing. be-sdk-root's `OwnSurface` configures one, draws it
         (`fill`, or `acquire` and `present`) and names it as a layer. be-sdk-root-testing
         fills its own with `ROOT_COLOR`, under the triangle's screen; be-wasm-root-host's
         `a_root_draws_its_own_surface_on_the_hosts_device` and the host test
         `a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds` check it, the second by
         the window's pixels: the root's colour beside the screen and through its clear
         corner, the window's background past the root's surface. The root draws with
         be3:gpu's raw bindings (be-wasm-gpu-abi), not wgpu: a root linking wgpu needed it
         built for a pure guest, which 4g2 settled by linking a root as an app is.
       - The gpu shim got a plan and a platform of its own (`wasm32_bindgen`, the `bindgen`
         constraint): in the shared wasm32 plan its `wgpu` feature reached be-wasm-gpu-abi, so
         a root that called the gpu linked wgpu and web-sys, whose wasm-bindgen imports a pure
         guest cannot load.
       - `be3:root`'s input pieces, each the smallest form the protocol takes: `frame-done`
         gained `every-sample`, `cursor` (a CSS cursor name, be-lib-ui-types'
         `CursorIcon::name`) and `pointer-locked`; `input.deliver`'s events are
         be-lib-ui-types' `Timed`, each with the frame clock's time it was sampled at, in
         nanoseconds, when the platform said, which beui's `Program::input` now hands a
         program. be-sdk-root answers them from its apps (`Embedding::requests`; the pointer
         lock granted to the focused screen that asks in `Embedding::drew`, through
         `Windows::lock`), sends an app's input with its time (`TimedInput::time_micros`) and
         every sample while its screen asks, and be-host sets them as beui's window requests
         after the document it shows the frame in. A sample's time is real time from the frame
         clock's origin, so under drive's stopped clock it can be later than the frame's. A lock
         the platform released (Esc, or the system taking it back) or a focus change took away
         is granted again only once the screen stops asking and asks anew, as a browser wants a
         fresh request, so the stale request of an app that has not laid out since does not
         take the lock back from the user (`Windows::lock`). There is no intercepted-chord list in
         `be3:root`: the root takes every key before beui sees it, so its own `Router` holds
         its chords (Input, Keys). be-sdk-root tests the times, the lock and the requests
         (`each_delivery_carries_the_time_its_input_was_sampled`,
         `the_focused_layer_that_asks_holds_the_pointer_lock_until_it_stops_asking`,
         `a_window_asks_for_what_the_screens_it_shows_ask_of_theirs`), and be-wasm-root-host
         that they cross `be3:root` with be-wasm-root-testing's root, which asks for the
         crosshair over its child, the lock while its child has the focus and every sample
         while it shows it, and marks the time of the last input it routed to the child.
       Left for later: no test shows be-host honouring the cursor, the lock or every sample,
       since no app a root embeds asks for them before step 5 gives one content (the triangle
       asks none); and the web's half of a root's own surfaces, since no page runs a root yet.
     - 4g2. Built: beui in the root. A root is a linked guest, built and loaded as an app is
       (decisions/a_root_links_beui_as_an_app_does.md): `app_module` in the `wasi-guest` plan,
       linked against base and beui, which be-host loads with `be_wasm_host::Host::new` on the
       window's device (sharing each library's compile with the apps) and `Root` steps inline as
       before; only be-wasm-root-testing, which draws no UI, stays a pure guest. A pure root was
       rejected: it would be a second build of beui and wgpu for a triple no app uses.
       `crates/be-sdk-beui-root`'s `BeuiSurface` draws a beui document on one of the root's
       `OwnSurface`s: beui's runner runs it as a program whose one window is the surface, beui's
       wgpu renderer draws it over `be3:gpu`'s `custom` backend into the surface's texture
       (`be_wasm_gpu_guest::texture`), and it hears what the router lands on the surface
       (`Embedding::input` and `drew`'s `own`) in its points. `BeuiRoot` is what a root's surfaces share: the device, the
       fonts and the host's clocks, pinned as beui's now each frame, so a document reads no
       clock of its own; a surface draws when it heard input, was woken or a frame it asked for
       is due (`BeuiSurface::next`). `be3:root` gained what any framework needs, nothing beui's:
       `host.fonts`, the fonts the host bundles as the app protocol's own `Fonts` (moved into
       be3:ui, which beui shares, so beui-font-skrifa's `FontSources::of_faces` reads it), encoded
       (be-host's `be_host_runtime::bundled_fonts`, which it tells its apps; `Root::with_fonts`),
       which the root asks once and draws its text in; `frame-request`'s `wall-clock`, the
       host's wall clock (drive pins it);
       and the export `paintings`, what its own surfaces
       painted in beui-paint-format's `ScreenPaintings` keyed by surface id, as `be3:debug`'s
       `paintings` is for an app, which `drive paint` under `--root` asks for
       (`Root::ask_paintings`, `take_paintings`; be-host is busy until it is answered) and places
       where the layers show those surfaces. be-sdk-root-testing fills its own surface with
       `ROOT_COLOR` and draws a strip over it with beui's styled components, `TEXT` in the
       host's fonts above the triangle's screen (styled components take theme roles, never a
       raw colour, so the exact colour the tests read is the root's own fill);
       be-wasm-root-host's `a_root_paints_its_own_surface_with_beui_in_the_hosts_fonts` reads its
       glyphs from its painting, and
       `a_roots_document_draws_again_when_the_frame_it_asked_for_comes` that a timer on the host's
       frame clock redraws it when the frame it asked for comes, and the host test
       `a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds` reads the text's pixels and
       snapshots the painting.
       Left for later, and what it changes for 4g3 onward:
       - Missing characters (an app's `MissingCharacters`, and a `BeuiSurface`'s own) have no
         host call yet: a `host.fallbacks(characters)` would answer from the system's fonts, as
         be-host-runtime's `Fallbacks` does. (The apps a root embeds are told the faces
         `host.fonts` gave since 4g3.)
       - `drive paint` leaves out the app screens a root embeds: their paintings come through the
         root (Other moves, Paintings), which needs `Embedded` to ask its apps for theirs.
       - `drive tree`, `find` and pressing by name do not reach the root's beui document: its
         accessibility tree is not published through `be3:root` yet.
       - `BeuiSurface`'s window offers no clipboard, file dialogs or input method: each area that
         moves (dialogs in 4g4, the lock screen's password field in 4g6) needs the host's half
         of it through `be3:root` or the sandbox, as be-host-runtime serves an app's. (4g4
         built the clipboard and the input method; file dialogs are still left.)
       - A frame of the document is drawn whole (`Repaint::Region` of the surface), not by its
         damage: the surface keeps no record of which older frame its texture holds.
       - On the web, a linked root needs the page to link its libraries on the main thread with
         `dylink.js` and `wasi.js`, which only an app's worker does today; no page runs a root
         yet.
     - 4g3. Built: toasts, shown over everything, on a layer of the root's own that is hit only
       where a toast is and never focuses; they need nothing from the block stack. What they
       show is the host's own notices (be-host-base's `notices`: what of its own failed, or why
       it refused something), which `be3:root` carries as data, nothing beui's: `host.notices`
       is a watch, answering the notices (each its id, message, whether it says something
       failed, and its actions' keys and labels) once they differ from the generation the root
       saw, so the root hears each change rather than asking again; `host.answer-notice` hands
       back what the person chose, an action's key or none to dismiss it, which the host does,
       since copying a report and opening a link are the platform's (be-wasm-root-host's
       `Root::show_notices` and `take_answers`). be-host's `--root` hands the root
       be-host-base's notices whenever they changed (`notices::generation`, which be-host's own
       UI reads too; a notice shown wakes the window, through the host's waker it now installs)
       and dismisses, copies or opens what the root answers; the keys and
       labels are be-host-base's (`Notice::actions`, `COPY_REPORT`, `OPEN_LINK`), which
       be-host's own UI reads too. be-sdk-beui-root's `BeuiSurface` gained `overlay_area`,
       where its document's overlays take the pointer (beui's `Document::overlay_rects`), in
       the surface's pixels: as a layer's hit area, a document showing only overlays over a
       clear window is hit there alone; and `enter`, which sets the signals a document reads
       from outside. be-sdk-root-testing draws its host's notices as styled `Toasts` at the top
       right of a surface of its own (`TOAST_SURFACE`), listed as `TOAST_LAYER` over
       everything only while one shows, with `overlay_area` as its hit area and
       `focuses: false`; a toast dismissed, or gone when its time is up, it answers with no
       action. be-wasm-root-host's
       `a_root_shows_its_hosts_notices_as_toasts_hit_only_where_they_are` shows a failure with
       a report and a refusal, checks the layer is on top, hit at the toasts and not past them
       (where the strip of text under them takes a press), and never focuses, snapshots the
       toasts, presses the failure's action and close button through the root's router, which
       answer its key and a dismissal, and sees the refusal answered as dismissed once its time
       is up.
       With it, of 4g2's leftovers: the apps a root embeds are told the fonts `host.fonts` gave
       (`Spawn::fonts`, told once the app runs, with UTC as the offset since the root has no
       offset from its host yet).
       Left for later:
       - The apps a root embeds are told UTC as their offset: `be3:root` carries no UTC offset
         from the host yet.
       - No host test shows a notice under `--root`: nothing under it reports one yet, since
         it runs none of the host's core. A notice's account (one the server keeps, which
         dismissing dismisses on every device) is not dismissed on the server under `--root`,
         which signs in no account.
       - The slow save's toast is the block stack's (saving, 4g6), and the desktop's
         notification toasts are the desktop app's own.
       - `drive tree`, `find` and pressing by name still do not reach the root's documents, so
         a toast's live region reaches no screen reader under `--root`, and tests press its
         buttons only by position; and `drive paint` still leaves out the app screens a root embeds. Both
         need `be3:root` to carry the root's accessibility tree and its apps' paintings.
     - 4g4. Built: dialogs, a scrim layer that `captures` presses and dismisses, the dialog's
       layer above it, Escape among the chords the root intercepts while one is open, and the
       platform's own dialogs (files) still the host's. With them, of 4g2's leftovers, the
       host's half of the clipboard and the input method for a root's own documents. Escape is
       the dialog's own while it has the focus, so a popup inside it takes the key first, and
       the root's, intercepted, while the dialog is open and the focus is elsewhere.
       `be3:root` carries the clipboard as data, nothing beui's: `host.copy` puts a copy's
       items on the host's clipboard (each the app protocol's `ClipItem`, encoded, as `fonts`
       is), and `host.paste` asks for the first of some formats it holds, which the host
       answers once its platform read it, in a later frame if need be
       (be-wasm-root-host's `Root::take_copies`, `take_pastes` and `answer_paste`); the host
       trusts its root with the clipboard as it trusts no app, and a root never relays these
       calls for an app it embeds. `host.paste-shortcut` says whether the host delivers Ctrl+V
       as a paste itself (a web page does), so a root's document does not ask again
       (`Root::with_paste_shortcut`, `BeuiRoot::set_paste_shortcut`). The
       input method needed nothing new in `be3:root`: its events already reach the root as
       input, which the root's router hands its focused layer, and `frame-done`'s `ime` names
       the focused field's area. be-host's `--root` puts what the root copies on the window's
       clipboard (`Context::copy`) and reads each paste there (`Context::read_clipboard`).
       be-sdk-beui-root's `BeuiSurface` window copies and pastes through those calls, says the
       host shows the keyboard, and answers its focused field's input method area in the
       window's points (`BeuiSurface::ime`). be-sdk-root's `Embedding` gained `intercept`
       (the chords its own layer takes ahead of the focus, in every window, new ones too),
       `focus` (moving the keyboard to a layer, or from all, telling both; a layer not drawn
       last cannot take it) and `focused`; a focused layer a list leaves out loses the focus,
       so a later layer with its id never takes keys it was not told it has.
       be-sdk-root-testing opens a dialog on F2, which it always intercepts: a scrim
       (`SCRIM_LAYER`, its own surface filled with `SCRIM_COLOR`) hit nowhere, capturing
       every press for `ROOT_LAYER`, its placer, and never focusing, and over it the dialog
       (`DIALOG_LAYER`), a styled `DialogBody` with a name field in a card that fills its
       surface, which closes on Escape; Escape is intercepted while it is open and the focus
       is outside it; once drawn it takes the focus and
       hands its host the field's input method area, and closing (the scrim, Escape, Cancel
       or Rename, which shows the name in its strip) gives the focus back to the layer that
       had it. be-wasm-root-host's
       `a_dialog_shows_over_its_scrim_which_captures_a_press_beside_it_and_closes_it`,
       `escape_closes_a_dialog_that_has_the_focus` and
       `typing_through_the_input_method_reaches_a_dialogs_field_and_the_clipboard_is_the_hosts`
       check it through `be3:root`, and the host test
       `a_root_in_place_of_the_hosts_ui_opens_a_dialog_on_the_hosts_clipboard_and_input_method`
       through be-host, with drive's input method and the window's clipboard, a painting of
       the dialog over its scrim, and a key reaching the triangle once the dialog gave its
       focus back.
       Left for later:
       - A lazy clip representation (`ClipData::Lazy`) is not copied: the host never asks
         the root back to render one. A root that copies one would need a `render-clip`
         export the host calls when a paste asks for it.
       - Files: a root's document cannot choose or save one yet, since `be3:root` carries
         no file dialogs; the platform's stay the host's, so they want a host call each, as
         be-sdk-beui-platform's `pick_file` and `save_file` are for an app.
       - be-sdk-root-testing's scrim layer captures presses across the whole window (it is
         hit nowhere and captures, so any press that misses the dialog goes to its placer),
         but dims only its fixed `SIZE`, as its own surface does. A root that follows its
         window's size (4g7) sizes its scrim's surface with it.
       - The dialog's surface has a fixed size its content fits (`DIALOG_SIZE`), not its
         content's own height: `BeuiSurface` cannot measure its document yet, which a
         surface sized to a card would need.
       - Intercepted chords are the same in every window (`Embedding::intercept`): a dialog
         in one window takes Escape in all of them while the focus is outside it. Chords per
         window come with the compositor's own windows (4g7).
       - `drive paint` must place a root's surfaces with their layer's opacity before any root
         fades a layer; until then a root draws transparency into its surface, as the scrim's
         fill does.
       - A clip of `be3/blocks` reaches the root, and the host from it, with its block refs
         untranslated: a root's own documents name no blocks before the block stack is the
         root's (4g6, step 5), which then translates them as be-host-runtime does an app's.
     - 4g5. Built in part: the debug panels' Performance panel, over the read-only statistics
       the host grants (What the host keeps). `be3:root` carries them as data, nothing beui's:
       `host.watch-statistics` says whether the root shows them, and the host measures what
       only they need (its memory) only while it does; `host.statistics` is a watch, as
       `host.notices` is, answering the host's statistics once they change from the generation
       the root saw: its performance rows (the full frame, and each group it times, with the
       latest, average and peak of each) and memory rows, each a heading or a measurement as
       the host words it (`statistic-row`, be-host-base's `PerformanceRow`), and the file its
       memory statistics are written to (be-wasm-root-host's `Root::watching_statistics` and
       `show_statistics`); turning the watch off answers a call still waiting. be-host's
       `--root` samples its memory only while the root watches (a sampler that parks on a
       condition variable while nothing does, and writes no file, unlike the desktop's, which
       samples for good into `memory.tsv`), whose samples wake the window, and hands the root
       be-host-base's performance rows each frame and its memory rows once a new sample came,
       which the root hears only once they changed. be-sdk-root-testing opens and closes a panel of them on F3, which it always
       intercepts (`STATISTICS_LAYER`, drawn with beui on `STATISTICS_SURFACE` at the window's
       right), watching them while it is open and never taking the focus.
       be-wasm-root-host's
       `a_root_shows_its_hosts_statistics_in_a_panel_watching_them_while_it_is_open` shows it
       rows and snapshots the panel, and the host test
       `a_root_in_place_of_the_hosts_ui_shows_the_hosts_statistics_in_a_panel` opens and closes
       it in be-host.
       Carried from 4g4's re-review: a key's repeats and release go where its press went
       (be-lib-input-router's `Router`), so the release of an Escape that closed a root's
       dialog never reaches the layer the focus went back to; the release of a key held when
       the window lost the keyboard goes nowhere; and be-sdk-root's `Windows::drew` forgets a
       layer its list leaves out, so the focus, a lock, a press or a key it held goes to no
       layer that later takes its id.
       Built after 4g6: the memory graph (be-host's `MemoryGraph`). `host.memory-history` is a
       watch of its own, apart from `host.statistics`, whose rows change every frame while the
       graph's points change once a minute: `memory-minutes` carries the minutes measured, each
       `memory-point` its time and resident, anonymous, shared and wasm bytes at the minute's
       peak, and when the latest sample was taken (be-wasm-root-host's
       `Root::show_memory_history`); turning `watch-statistics` off answers a call still
       waiting, as for the rows. be-host's `--root` hands it the points of be-host's own
       `memory::view` once a new sample came. be-sdk-root-testing's statistics panel graphs
       them above its rows as be-host's own does (each line in a colour role, the top the peak
       resident size rounded up to 64 MiB, a gap of more than two minutes breaking a line),
       watching them while the panel is open; the statistics test shows it minutes with such
       a gap and snapshots the graph.
       Left for later:
       - The Block stack panel, whose lines are the block stack's: the root's once the block
         stack is (step 5).
       - The Running apps panel and its kill and crash commands: the apps a root embeds are
         the root's own (`Embedding::apps`), so it lists them itself, once it runs the apps
         be-host runs today (4g7).
       - The panels as the shell's host panels (`HostPanel`), docked beside its editors: they
         move with the compositor (4g7); until then a root shows them as it likes.
     - 4g6. The lock screen, onboarding, saving, keep changes, organization and
       be-root-accounts-ui, over the block stack's messages and the password check the host
       grants.
       How the rest reaches the root (the design, before the slices). Onboarding, saving,
       keep changes, organization and be-root-accounts-ui show what be-host decides today,
       which `App::view` gathers into one `HostUi` each frame and whose commands come back as
       `UiCommand`s. Two kinds of it go two ways, all data and nothing beui's:
       - The platform's own, which stays the host's after step 5, gets a capability of its own,
         as notices, the lock and statistics have: keep changes (the display settings waiting
         to be kept or reverted) is a watch and a keep-or-revert call. Displays never enter
         the core's view.
       - The core's, which leaves be-host only with the block stack in step 5 (its accounts and
         sign-in, the keys, its organizations and sessions, the block stack's status), crosses
         as one watch, `host.core(seen)` answering a `core-view` record with a field per area
         (each the plain data `HostUi` holds for it, block ids as UUIDs, no labels the root can
         word itself), and one command call, `host.ask-core(command)`, a `core-command` variant
         with a case per `UiCommand` an area sends; what it changes reaches `core`. One pair
         rather than a pair per area, since the core moves whole: step 5 makes it the root's
         own and the pair goes.
       What changes often gets a watch of its own, apart from `core-view`: saving (whether a
       change is unsaved, the saves finished), which moves at the rate the person types, is
       `host.saving`. Were it a field of `core-view`, every keystroke would resend the whole
       view (the accounts, the organizations, each one's sessions) and wake every area's
       document; apart, a change of saving costs only itself, and `core-view` changes only at
       the rate its slow areas do. The root splits `core-view` into one memo per area, so an
       area whose field did not change does not render again. A secret the person types (the
       recovery phrase) crosses as `host.unlock`'s password does, once, inside a command,
       zeroed by the host, never in the view. Each area adds its field and its commands with
       the slice that moves it; be-host's `--root` answers what it has, which is nothing before
       it runs the core (4g7): nothing to save, no display waiting, no accounts. be-host's own
       `SlowSave` is a copy of the root's slow save's toast, and be-root's
       `MemoryGraph` and `draw_memory` a copy of be-host's memory graph, until the root becomes
       the default (4g7), which deletes be-host's UI and the copies with it; a crate shared
       for interim duplicates is not worth it. The slices: 4g6b saving (the slow save's toast); 4g6c keep
       changes (the display's countdown); 4g6d the core's view and commands with the changes
       not yet sent and discarding them (the unsent dialog); 4g6e accounts and adding one
       (onboarding); 4g6f organizations, sessions and their dialogs; 4g6g recovery and the
       phrase.
       - 4g6b. Built: `host.saving`, a watch of whether the person's changes are saved
         (`saving-state`: whether some change is unsaved, the saves finished so far, the changes
         not yet sent, and the sync's state), from which the root shows the slow save's toast
         as be-host's own UI does (be-wasm-root-host's `Root::show_saving`). be-host's `--root`
         runs no block stack, so it stays as `be3:root` starts it: all saved. The count of
         unsent changes is a `u32`, which 4g7's host saturates from its `usize`.
         be-sdk-root-testing shows the slow save's toast among its notices once changes stay
         unsaved for `SLOW_SAVE` by the host's frame clock, a finished save starting the wait
         afresh, until they are saved or it is dismissed, and hides it with every toast while
         locked; be-wasm-root-host's
         `a_root_shows_the_slow_saves_toast_while_its_hosts_changes_stay_unsaved` checks the
         wait, a save restarting it, a dismissed toast staying dismissed while still unsaved,
         and the toast hidden while locked, and snapshots the toast.
       - 4g6c. Built: keep changes, as the platform's own capability: `host.displays` is a watch,
         as `host.notices` is, of whether the host asks the person to keep display settings
         that just changed (`display-state`'s `asking`, a `display-question`: its round and
         `revert-at`, the frame clock's time the host reverts them at unless kept, whether or
         not the root answered, as `lock-state`'s `retry-at` is a time the root counts down
         to), and `host.answer-display(round, keep)` answers it; be3:root drops an answer to a
         round it does not ask now, so the host hears only answers to its question
         (be-wasm-root-host's `Root::show_displays`, `take_display_answers`). be-host's `--root`
         manages no displays (the desktop's), so it asks nothing and an answer changes nothing.
         be-sdk-root-testing asks in its dialog's place while the host asks (`KEEP_DISPLAY_TITLE`,
         an alert with Keep and Revert over the scrim, taking the focus), counting down on its
         clock to `revert-at` and answering not kept when it is reached, a courtesy the host
         does not wait for. It keeps the question it heard last, so one asked while the session
         is locked shows once it is unlocked if still asked and not answered; a question
         replaces an open rename dialog, which closes without renaming. Escape and a press on
         the scrim revert at once here, the shortest path for a test root. The real root
         chose the same (4g7a2): be-host's own prompt reverts on Escape and on its scrim
         (`Modal`'s `on_dismiss`), so be-root's does too, though reverting on a stray press
         is harsh and some desktops make the scrim of such a question inert. The host withdrawing the question
         closes it unanswered. be-wasm-root-host's
         `a_root_asks_to_keep_display_settings_its_host_changed` keeps with Enter, reverts with
         Escape and at `revert-at`, sees a withdrawn question go unanswered and only the round
         asked answered, shows a question asked while locked once unlocked, sees a question
         replace a rename dialog without renaming, and snapshots the question.
       - 4g6d. Built: the core's view and commands, starting with the changes not yet sent:
         `host.core` is a watch of a `core-view`, its one field so far `unsent`, the blocks
         whose changes this device could not send (each `unsent-edit`: the block, its name,
         whether the person may edit it again, why it was set aside as a `set-aside` the root
         words, none when access was lost, and the edits that never became a commit), and
         `host.ask-core` takes a `core-command`, so far `restore-unsent` and `discard-unsent`
         (be-wasm-root-host's `Root::show_core`, `take_core_commands`). Whether the list is
         open and which block's discard is being asked are the root's, so they are not in the
         view: be-host's `Unsent(bool)`, `DiscardUnsent`, `ConfirmDiscardUnsent` and
         `CancelDiscardUnsent` become the root's own state, and only the confirmed discard
         crosses. be-host's `--root` runs no block stack, so it shows no unsent changes and
         drops what is asked (`refuse_unserved`, with the lock's and the displays'). 4g7's host
         maps be-host-stack's `UnsentEdit` (its `be_block_client::Aside` rather than words)
         onto it, saturating `kept-edits`. be-sdk-root-testing lists the unsent edits in its
         dialog's place on F7 (`OPENS_UNSENT`, `UNSENT_TITLE`), splitting the view into a memo
         for its area, with Restore and Discard per block; Discard asks first
         (`DISCARD_UNSENT_TITLE`, an alert in the list's place), and a block that leaves the
         list takes its question with it. be-wasm-root-host's
         `a_root_lists_its_hosts_unsent_edits_and_asks_before_discarding_one` restores one,
         cancels and confirms a discard, sees a question whose block left discard nothing,
         and snapshots the list and the question.
       - 4g6e. Built: accounts and adding one (onboarding), as fields of `core-view` and
         cases of `core-command`. The view carries whether the block stack waits for the
         person to open or add an account (be-host's `Screen::Accounts`; 4g6f1 makes it
         `waits-for`'s `account`), the
         `accounts` (each `account-row`: the host's key for it, its name, email, its server or
         none for one kept on this device alone, whether it remembers its organization,
         whether it is open, its `organizations`), `signing-in` (an account being added, with the
         `browser-sign-in`'s page and code once the server gave them) and `account-error`, in
         words as be-host has it. The commands are `open-account` and `choose-organization`
         (be-host's `OpenAccount`, `ChooseOrganization`), `forget-account` (its `LogOut` and
         `ConfirmRemoveAccount`: the root asks first for an account kept on this device alone,
         whose blocks go with it, so be-host's `removing_account` is the root's), `add-account`
         with a `new-account` (`local(name)`, or `server` with its address and the gateway
         token, a `password`: be-host's `SubmitAccount` and its `AccountForm`), and
         `open-sign-in-page`, `check-sign-in` and `cancel-sign-in` (its `OpenLoginBrowser`,
         `CheckLoginNow`, and `RetryAccount` and `CloseAddAccount`, which both drop the sign-in
         and its error). Whether the dialog is open and which step it is on are the root's
         (be-host's `add_account_open`, its generation and the form). The gateway token is a
         secret: be-wasm-ops-bindgen's `secret` option now also makes every type holding one
         derive `Debug` alone, neither copied nor compared, as `Password` is, and `ask-core`'s
         arguments are zeroed once decoded as `unlock`'s are. Reauthenticating an account
         (be-host's `ReauthView`) waits for 4g6f, since it opens over the organizations.
         be-host's `--root` runs no block stack, so it waits for no account. be-sdk-root-testing
         shows the accounts while the stack waits for one (`ONBOARDING_LAYER` on
         `ONBOARDING_SURFACE`, be-root-accounts-ui's `AccountsScreen`, over the app and under its
         dialog, taking every press and, once drawn, the focus, given back when they go), and
         adds an account in its dialog's place (`ADD_ACCOUNT_TITLE`: where to keep it, then the
         server's address and its gateway token, or a name; then `SIGN_IN_TITLE` with the code,
         Check now and Open browser), clearing the token's field as it hands it over, starting
         afresh when it opens, stopping the sign-in when it closes other than for an account
         added, and closing itself once an account it did not list is added; the host's
         question of keeping display settings replaces it without stopping a sign-in, and it
         reopens mid sign-in once the question goes if the sign-in goes on. Removing one kept
         on this device asks first (`FORGET_ACCOUNT_TITLE`). The accounts take back the focus's
         earlier holder from when they took it. At 4g7 be-root-accounts-ui's `AccountsScreen`
         and `ErrorText` move to a crate a root may use (be-host's own UI goes then), rather
         than the root reaching into a crate named for be-host. be-wasm-root-host's
         `a_root_shows_its_hosts_accounts_and_adds_one_handing_over_only_what_was_typed` opens
         an account both ways, asks before removing a local one, hands over a server's address
         and gateway token as typed, opens the sign-in's page, stops the sign-in when closed,
         keeps a sign-in going under the display question and shows it again after, adds a
         local account and sees the dialog close for it stopping nothing, sees the
         accounts go once one is open, and snapshots the accounts, the removal question, the
         dialog and the browser sign-in.
       - 4g6f. Organizations, sessions and their dialogs, in three slices: 4g6f1 what the block
         stack waits for, the organizations and the sessions; 4g6f2 reauthenticating an
         account, the question before discarding unsaved changes (be-host's
         `PendingDestructiveAction`), and the organization's dialogs (inviting, the
         invitations, members and agents, whose links and credentials are secrets the root
         shows); 4g6f3 contacts, share links and leaving a block. Decided for 4g6f2: a secret
         the host hands the person (an invite link made, an agent's credentials) comes back
         once, as the answer to a call of its own (`make-secret`, whose `secret-command`
         names what to make, so the type says which requests wait for a secret), never in
         `core-view`, which carries only
         what is not secret about it (that a link exists, whose, when it expires); the root
         holds it only while the dialog showing it is open, and drops it, zeroed, as it
         closes.
         - 4g6f3. Built: contacts, share links and leaving a block. `core-view` gains
           `contacts` (a `contacts-area`: each `contact-row`, its account, name, email, whether
           verified and its `contact-mismatch`, `older` or `unreadable`; the `contact-card`
           offered by a contact link opened, the one just connected with, busy and error),
           `share-links` (a `share-links-area`: the `share-preview` offered and the one opened,
           each the block's name, its organization, its `share-access` and whether the person
           joins as a guest, all data the root words; busy and error) and `leaving` (the block
           an editor asked to leave, its name and the block it sits in, whether leaving is
           busy and why it was refused: be-host's `LeavingView`, `leave_busy`,
           `leave_refused`). The commands: `open-contact-link` and `open-share-link`, each
           with the pasted link as a `password` (be-host's `OpenContactLink`,
           `OpenShareLink`), `accept-contact`, `accept-share-link`, `forget-contact-link` and
           `forget-share-link` (its `DeclineContact` and `DeclineShareLink`, which also put
           aside what was connected or opened and why it failed, as its `CloseContacts` and
           `CloseShareLinks` did: whether the dialogs are open is the root's, so it starts each
           afresh with them), `remove-contact` (its `ConfirmRemoveContact`; which contact is
           asked about is the root's), and `confirm-leave` and `cancel-leave`. A contact link
           made is `make-secret`'s `contact-link`, answered with `core-secret`'s `link`; the
           contacts' dialog holds itself open while one is being made, as the agents' does, so
           no live link is left unheld. A share preview's name is its maker's untrusted text,
           shown as plain text only.
           be-sdk-root-testing asks before leaving in its dialog's place while the host asks,
           after the discard question and before signing in again (`LEAVE_TITLE`: Leave, which
           stays busy until the host stops asking, or Stay, Escape and closing it, which is
           `cancel-leave`); F12 (`OPENS_CONTACTS`, `CONTACTS_TITLE`) lists the contacts, with
           Remove asked first (`REMOVE_CONTACT_TITLE`), the contact link offered to connect
           with, a contact link to make, shown once, and one to paste; F8 (`OPENS_SHARE_LINKS`,
           `SHARE_LINK_TITLE`) opens a pasted share link, asks before opening what it shares,
           worded as be-host's own dialog words it, and says what it opened. A link opened from
           the page's address (be-host's `open_from_address` on the web) reaches the root with
           4g7, which opens its dialog. be-wasm-root-host's
           `a_root_asks_before_leaving_a_block_and_opens_its_hosts_contact_and_share_links`
           leaves and stays, sees the question stay while leaving and go once left, makes a
           contact link, connects and declines, removes a contact asked first, hands over a
           share link as typed, opens what it shares, and snapshots the leave question, the
           contacts, the contact offered, the share link offered and what it opened.
         - 4g6f2b. Built: the organization's members and agents. `core-view` gains `members`
           (a `members-area`: whether `listed`, each `member-row`, its account, name, email,
           `member-role`, whether it is the person, an agent, or added by the server unsigned;
           busy and error; be-host's `MembersView`, as data the root words) and `agents` (an
           `agents-area`: whether listed, each `agent-row`'s account and name, busy and error).
           The commands: `list-members`, `set-member-role` (a `role-change`),
           `remove-member` (be-host's `ConfirmRemoveMember`: which member is being asked about
           is the root's, so its `RemoveMember` and `CancelRemoveMember` are), `list-agents`
           and `remove-agent`. Making an agent is `make-secret`'s `create-agent`, answered
           with `core-secret`'s `agent`, its `agent-credentials` (name, server, organization,
           and the token and recovery phrase as `password`s): be-host's `AgentsView::created`
           leaves the view. be-sdk-root-testing's F10 (`OPENS_MEMBERS`, `MEMBERS_TITLE`) lists
           the members, each but the person and agents with a change of role and Remove, which
           asks first in the list's place (`REMOVE_MEMBER_TITLE`); F11 (`OPENS_AGENTS`,
           `AGENTS_TITLE`) lists the agents with Remove and a name to make one, the credentials
           answered shown once with Copy and Done, as be-host's own dialog does: the JSON
           bundle a program reads from `BE3_AGENT_CREDENTIALS` (server, organization, recovery
           phrase, token), shown and copied whole. Credentials cannot be had again, so none may
           be lost on either side: be3:root drops a `make-secret` the root cancels before the
           host took it, so the host never makes it, and the agents' dialog neither closes
           (Close, its close button, Escape, the scrim) nor gives way to a question of the
           host's while one is being made, until the answer comes. A question of keeping
           display settings that comes meanwhile waits too, so the host may revert at its
           `revert-at` before the person sees it: the safe way for displays, but the real root
           (4g7) either defers no longer than making an agent can take or lets that question
           go over a dialog held this way. A secret the root no longer
           waits for when the host answers is lost for good: `answer_secret` answers false,
           and 4g7's host then removes the agent it just made and says so in `agents`' error.
           The host refuses `set-member-role` for the person's own role, or one leaving no
           administrator, in `members`' error; the testing root offers no change of the
           person's own role. The secret a dialog shows is now any
           `core-secret` (`Held`, an `Rc` compared by identity), handed by `secret_asker` to
           whichever dialog asks for secrets. be-wasm-root-host's
           `a_root_lists_its_hosts_members_and_agents_and_shows_an_agents_credentials_once`
           changes a role, keeps and removes a member, makes an agent from a typed name and
           shows its credentials, copies them, sees them gone once reopened, sees the dialog
           stay open and a question of the host's wait while one is being made, removes an
           agent,
           and snapshots the members, the removal question, the agents and the credentials.
         - 4g6f2a. Built (4g6f2 split again: 4g6f2b is members and agents): signing in again,
           the question before discarding unsaved changes, and invitations, with the rule for
           secrets the host hands the person. `core-view` gains `reauthenticating` (the account
           whose sign-in ended: its key, name, whether busy and why it failed; be-host's
           `ReauthView`), `discarding` (a `discard-reason`, `switch-organization` or `close`:
           be-host's `PendingDestructiveAction`, as data the root words) and `invitations`, an
           `invitations-area` (whether `listed` since asked, each `invitation-row` as data: its
           id, `invitation-state`, inviter, accepter, the times it was made, opened, expires and
           ended, and whether its link can be had again, never the link; busy and error). The
           commands: `reauthenticate` (its `ReauthSubmit`; its `ReauthLogOut` is
           `forget-account` with the key and its `ReauthClose` is `switch-account`),
           `switch-organization`, `confirm-discard` and `cancel-discard` (its `Discard`,
           `CancelDiscard`), `list-invitations` (its `OpenInvitations`, whose open state is the
           root's) and `revoke-invitation`. Making a link is `make-secret`, a call of its own
           taking a `secret-command` (`invite-member`, `invitation-link`; 4g6f2b adds the
           agent's, 4g6f2b), answered once made with an `option<core-secret>` (`link`, a `password`), or
           nothing if making it failed, why reaching `invitations`' error; `ask-core` answers at
           once, as before. be-wasm-root-host's `Root::take_secret_requests` hands each request
           with the id `Root::answer_secret` answers it by; an id no longer waited for drops the
           secret, and the root is stepped only when one was. be-host's `--root` answers each
           with nothing (`refuse_unserved`). be-sdk-root-testing asks the host's questions in its
           dialog's place while the host asks them, once no dialog is open (the discard before
           the sign-in; the display question still replaces any): `DISCARD_TITLE` (Keep, Escape
           or closing it is `cancel-discard`, the destructive action `confirm-discard`, and one
           the host withdraws closes answering nothing) and `REAUTH_TITLE` (Continue in browser,
           Log out; closing it is `switch-account`, as be-host's is). Signing in again shows as
           the dialog adding an account resumed, which now also closes once a sign-in it showed
           ends with no error. F9 (`OPENS_INVITATIONS`) lists the invitations
           (`INVITATIONS_TITLE`), asking `list-invitations` as it opens: each row worded from
           its data, Link and Revoke while open, New link, asking for one link at a time so an
           answer is the latest request's; a link answered shows in the dialog's place with
           Copy (the host's clipboard) and Done, held as a `Password` behind an `Rc` compared by
           identity, and dropped, zeroed, with Done or the dialog.
           be-wasm-root-host's
           `a_root_asks_its_hosts_questions_and_shows_an_invitations_link_only_once` keeps and
           discards changes, sees a withdrawn question answer nothing, signs in again through
           the sign-in and sees it close stopping nothing, closes the question to switch
           account, lists the invitations, answers New link with a link, sees no second request
           while one waits and a second answer to the same id change nothing, copies it, drops
           it with Done, asks a row's link again and sees a link not made show none, revokes
           one, sees the link gone once reopened, and snapshots
           the discard question, the sign-in question, the invitations and the link.
         - 4g6f1. Built: `core-view`'s `waits-for`, a `core-wait` (`account`, `organization`,
           `session`; none once an organization's blocks are open), in place of 4g6e's flag:
           be-host's `Screen::Accounts`, `Organizations` and `Sessions` (its `Recovery`,
           `Unlock` and `Error` join with 4g6g). `organizations`, an `organizations-area`:
           `listed` (`loading`, `loaded` with each `organization-row`'s id and name, or
           `failed`), the `invitation` opened (`joining`: its organization, inviter, the
           `problem` in words and whether the account may join), and `busy` and `error`.
           `sessions`, a `sessions-area`: whether `loaded`, the organization's sessions of
           every type (each `session-row`: id, name, its `session-type` and whether used
           last), the `session-kinds` a new session can be with their names, the type this
           device opens, and the `profiles`. The commands: `switch-account` (be-host's
           `SwitchAccount` and `ManageAccounts`; logging out of the account open is
           `forget-account` with its key), `reload-organizations`, `open-organization`,
           `choose-session-in` (its `ChooseSession`), `create-organization`,
           `open-invitation` with the pasted link as a `password`, since it is a bearer
           secret (its `OpenInvite`), `join-invited`, `cancel-join`, `choose-session` and
           `close-sessions` (its `SwitchSession`, `CloseSessions`), `open-session`,
           `new-session` (its `OpenNewSession`), `use-profile` and `new-profile`. Showing
           sessions of every type is the root's own choice, so be-host's `EverySessionType`
           and its filtering are the root's: the view lists them all. be-host's `created`
           count, which cleared its form, is the root's too. `core-view`,
           `organizations-area`, `organizations-listed` and `sessions-area` derive `Default`
           (nothing waited for, nothing listed). be-sdk-root-testing's `ONBOARDING_LAYER` on
           `ONBOARDING_SURFACE` (4g6e's accounts' layer, renamed) shows whichever the stack
           waits for, a memo per area off the one view: the accounts, the organizations
           (`ORGANIZATIONS_TITLE`: each with Sessions and Open, Try again when listing failed,
           the invitation with Not now and Join, a name to create one, an invitation link
           handed over as a secret and cleared, though shown as typed so the person sees what
           they pasted, Switch account) or the sessions
           (`SESSIONS_TITLE`: those of this device's type, or every type, each to open, a new
           one, the profiles to use and a new one, Close). be-wasm-root-host's
           `a_root_shows_its_hosts_organizations_and_sessions` clicks each and checks the
           command, the link handed over as typed, the root's every-type choice asking
           nothing, the onboarding gone once nothing waits, and snapshots the organizations,
           the invitation, a failed listing and the sessions.
       - 4g6g. Built: recovery, unlocking and pairing, and the block stack's failure, which
         end 4g6's areas. `core-wait` gains `recovery`, `unlock` and `failure` (be-host's
         `Screen::Recovery`, `Unlock` and `Error`). `core-view` gains `recovery` (a
         `recovery-area`: how many words the phrase has, the `checked` positions the person
         types back, whether it `replacing` one, busy and error; be-host-stack's
         `RecoveryView`, without its words), `unlock` (an `unlock-area`: the account's name,
         whether a pairing code is out, error; its `UnlockView`, without the code), `pairing`
         (each `pairing-request` another device made: its id, device name and why approving
         was refused; be-host's `PairingRow`) and `failure` (a `core-failure`: the message and
         the changes not saved; its `ErrorView`, whose pending action is the root's). The
         phrase's words and a pairing code are secrets the host hands the person, so they come
         from `make-secret` (`recovery-phrase`, which the host keeps until confirmed, so it may
         be had again, and `pairing-code`), answered with `core-secret`'s `phrase` and `code`;
         what the person types crosses as a `password` inside a command:
         `confirm-recovery` (the checked words, in order, separated by spaces; be-host's
         `ConfirmRecovery`), `unlock-with-phrase` and `approve-pairing` (a `pairing-approval`,
         the request's id and the code its device shows). The rest: `new-recovery-phrase`,
         `cancel-recovery`, `cancel-pairing`, and after a failure `restart`,
         `delete-client-database` (be-host's `ConfirmErrorAction`; its `AskErrorAction` and
         `CancelErrorAction` are the root's) and `exit`. be-host's `--root` waits for none of
         them. be-sdk-root-testing's onboarding shows the recovery phrase (`RECOVERY_TITLE`:
         asked for as it shows, numbered, held only while it shows; a field for the checked
         words; Confirm, New phrase, and Keep the old one while replacing), unlocking
         (`UNLOCK_TITLE`: the phrase to type, Unlock, Pair with another device, whose code shows
         with Cancel) and the failure (`FAILURE_TITLE`: Restart, Quit, and Delete local copy,
         asked first with the changes it loses). Secrets the onboarding asks for reach it as
         the dialogs' do, through `secret_asker`, held by its own `Closing`. Another device's
         request is a question of the host's (`PAIRING_TITLE`), after leaving a block: the code
         typed approves it, and Not now puts it aside, the root asking about it no more.
         be-wasm-root-host's
         `a_root_shows_its_hosts_recovery_phrase_and_unlocks_and_pairs_handing_over_only_what_was_typed`
         answers the phrase asked for, hands over the checked words, the phrase and a pairing
         approval's code as typed, shows a pairing code answered, restarts, quits and asks
         before deleting, puts a request aside, and snapshots the phrase, unlocking, the code,
         the failure, its question and the pairing request.
       - 4g6a. Built: the lock screen, over the screen lock and password check the host grants.
         The lock is the host's (the platform's, a login session's): `be3:root` carries it as
         data, nothing beui's. `host.lock` is a watch, as `host.notices` is, answering the lock
         (`lock-state`: whether the host can lock, whether it is locked, whether a password is
         being checked, why the last was refused, whose session it is, the power actions the
         lock screen offers, and `retry-at`, the frame clock's time the host's wait after failed
         checks ends at, which the root counts down itself; until the host shows the lock again
         with none, once the wait is over, a password is dropped unchecked) once it changes; `host.lock-session` asks the
         host to lock it; `host.lock-power` is a power action chosen on it; and `host.unlock`
         hands the host what the person typed, answering nothing: how the check went reaches
         the root only through `lock`. The host takes one password at a time: while one is
         handed over and not taken, being checked, or while the lock has a `retry-at`, or while
         the session is not locked, `be3:root` drops one unchecked. 4g7's host shows the lock
         again with no `retry-at` when its wait expires. The host rate-limits failed
         checks with a growing wait, as the login session's lock does (`login_session::lock`'s
         `FREE_ATTEMPTS` and penalty), which is the guard against guessing, since a root can
         always lock the session itself. The password is be-wasm-password's `Password`, the one
         type be3:root's `password`, the login session's lock and PAM share: zeroed when
         dropped and never printed, so the generated call's `Debug` shows nothing of it. What
         holds of the secret: the host zeroes its decoded copy once checked and nothing of it
         crosses back; be-wasm-ops zeroes the arguments of an accepted call taking a
         secret once decoded (be-wasm-ops-bindgen's `secret` option, `be_wasm_ops::scrub`; no
         other call pays for it) and the root host each frame it received; the guest's own
         buffers (its text field and what encoded the call) and, on the web, the page's copy
         are not scrubbed, so a root clears its field as it hands it over and keeps no copy
         (be-wasm-root-host's `Root::show_lock`, `take_lock_asked`, `take_unlock`,
         `take_lock_powers`). be-host's `--root` runs no screen lock, being refused as a login
         session, so the lock stays unavailable and no password reaches it; the login session's
         lock (`login_session`'s `ScreenLock`, PAM) reaches the root once `--root` runs as one
         (4g7). be-sdk-root-testing shows a lock screen while locked (`LOCK_LAYER`, beui on
         `LOCK_SURFACE`, hit everywhere, capturing and focusing): over everything, toasts
         hidden, taking the focus for its password field once drawn and giving it back once
         unlocked, the root intercepting none of its own keys meanwhile, only the F5 test
         fixture, and opening no dialog;
         F4 (`LOCKS_SESSION`) asks the host to lock, and F5 (`UNLOCKS_TWICE`), a fixture,
         hands over a stray password twice as a root gone wrong might. be-wasm-root-host's
         `a_root_shows_its_hosts_lock_screen_and_hands_over_only_what_was_typed` checks a
         password dropped while unlocked, while one is checked and while the host makes the
         person wait, the toasts hidden, a press over the app landing on the lock screen, the
         input method on its field, the password handed over as typed and the field cleared,
         the countdown (snapshotted with the refusal), and the focus back in the dialog's field
         once unlocked.
         Left for later in 4g6: onboarding, saving, keep changes, organization and
         be-root-accounts-ui, which need the block stack's messages through `be3:root`; the
         recovery phrase (`UnlockWithPhrase`), which crosses as a password does (all since
         built: 4g6b to 4g6g, and 4g5's memory graph).
     - 4g7. The compositor itself: app views (each declaring its layer's hit and gesture
       areas and placer), anchored views placed outside groups (The layer list, Built
       (step 4b)), editors, desktop windows, the camera and the palette, the chords along the
       focus (Tab and F6 between surfaces, the palette's), and drive's commands split as The
       root guest says. Then be-host brings back under `--root` what the host keeps, and the
       root becomes the default. A real root shows a lock layer per window or display,
       each the size of it, as be-host's lock screen does per display today.
       The slices, agreed. The compositor is about 7,000
       lines of `src/compositor` (app views 2,200, editors and the camera 2,400, desktop
       windows 1,200, anchored views 500, the palette 500), over be-host-runtime's
       orchestration; each slice moves one area and its tests, and be-host keeps running its
       own until the last.
       - 4g7a. The real root's crate (decisions/the_real_root_is_its_own_crate_not_the_testing_root.md). be-sdk-root-testing is a test fixture and stays one:
         its keys (F2 to F12) and its fixed 152-point dialog exist for tests. The real root is
         a new guest crate (`crates/be-root`, a cdylib on be-sdk-root and be-sdk-beui-root),
         which be-host's `--root` runs when it is given no other, and which starts with what
         4g6 built in the test root, moved rather than copied: the lock screen, notices and
         the slow save's toast, keep changes, the core's areas and their dialogs, and the
         statistics panel, worded and laid out as be-host's own UI (its `src/ui`), on
         be-host-accounts-ui moved into a crate a root may use. be-sdk-root-testing keeps
         only what its own tests check (a dialog, the lock, a notice, the statistics), each a
         fixture of its own rather than the product's, and
         be-wasm-root-host's area tests move to the real root's crate, run on the same host.
         In two parts:
         - 4g7a1 (built). `crates/be-root`, a family of its own (`be-root-*` in
           buck/cargo/layers.bzl, which may use be-sdk, beui, be-lib and be-wasm, and which
           be-host may use), holds 4g6's pieces moved from be-sdk-root-testing as they were:
           its notices, the slow save's toast, the lock screen, keep changes, the core's
           onboarding, areas and dialogs, and the statistics panel with its memory graph.
           Its F-keys are gone: a menu at the window's top right (`MENU_LAYER`, beui's
           `MenuButton` on `MENU_SURFACE`, shown while nothing waits, over the statistics
           panel and under the dialog, hit only at its button, `MENU_BUTTON`, and where the
           menu it opens shows) opens what be-host's organization menu does: unsent edits,
           contacts, a share link, invitations, members, agents, sessions, switching
           organization or account, a new recovery phrase, locking the session, and the
           statistics. Its guest is split by area (`guest/onboarding.rs`, `dialogs.rs`,
           `lock.rs`, `notices.rs`, `statistics.rs`, `menu.rs`). be-host-accounts-ui is
           be-root-accounts-ui. be-sdk-root-testing is back to its fixture of before 4g6b
           (the triangle, the strip, the rename dialog, toasts, F2 to F5), with nothing of
           the product's in it: its lock screen is a password field, the host's refusal and
           the power buttons, and its statistics panel the rows as lines of text, no graph;
           the lock card and the memory graph are be-root's alone, their paintings moved
           with them. be-wasm-root-host's area tests run be-root in its own crate
           (`BE_ROOT_WASM`), their dialogs opened from the menu. be-host's `:native` stages be-root, and `--root`
           alone runs it (`be_root_in_place_of_the_hosts_ui_shows_its_onboarding`).
         - 4g7a2 (built). be-host's wording and layout. `frame-request` carries the host's
           window (`window`: its size in points and pixels per point) and time zone
           (`utc-offset`); be-root fills the window with every surface but the statistics
           panel (`STATISTICS_WIDTH` at its right edge) and resizes them as it changes, the
           fixture's `SIZE`, `DIALOG_SIZE`, `MENU_AREA` and the scrim layer gone. Its dialogs
           are be-host's own, beui's `DialogFrame` at be-host's width and kind for each
           (`Presented`) on a surface the window's size, whose scrim Escape and a press
           dismiss (the question of keeping display settings reverting, as be-host's does,
           and a dialog waiting for a secret that cannot be had again holding); every
           dialog's own Escape handler that only closed it is gone. Its titles, copy,
           labels and layouts are be-host's: the dialogs, the onboarding (the account card
           and menu, opening, joining and creating an organization, the settings profile and
           sessions, the recovery phrase checked a word a field, unlocking with the phrase or
           another device, the failure with Copy and Delete client database), adding an
           account (where to keep it, the build's default server first), and the lock screen
           with its clock at the host's offset. Its menu shows be-host's organization menu:
           how changes sync, unsent edits while there are some, the escrow warning, Stop
           hosting each kept block, Invite member for a company, Members and Invitations for
           an administrator, and Manage accounts naming who is signed in; Lock, which
           be-host's menu has not, is gone. `core-view` gained what that needs
           (`organization`, `company`, `administrator`, `escrow-warning`, the host's own
           reason, `hosting`, `default-server`; who is signed in is the current account) and
           `core-command` `stop-hosting`; be-host fills them since 4g7b2a. An agent's credentials
           cross as the bundles the host's block stack encodes (`agent-credentials`'
           `bundle`, `proxy-bundle` and `proxy-header`), which the root only shows and copies. Left to be-host until it hands them over (4g7g): About, Close be-host,
           its other debug panels and the inspector in the menu; the invite's QR code and
           Share; Invite member inside Members; and the web's note that a local account
           lives only in the browser.
       - 4g7b. App views: be-host-runtime's orchestration (pacing, settle, restarts, grafting
         the accessibility trees) as a guest library the root links, and `app_view`'s views
         in the root, each declaring its layer's hit and gesture areas and placer. Until step 5
         the host serves the editors of the apps the root embeds, through the root
         (decisions/a_roots_embedded_editors_are_served_by_its_host_until_step_5.md); step 5
         deletes the host's end (be-host-runtime's `Forwarded`, the `be3:root` calls below and
         `Wire`'s forwarding).
         - 4g7b1 (built). Serving through the root. `be3:root`'s `host` gained:
           - `editor-of(block)`: the staged app and entry point that edit it;
           - `open-editor(instance, block)`: the host opens the block's editor in the app the
             root spawned as that sandbox instance, which must run the app `editor-of` names
             (be-wasm-sandbox-host's `Server::app_of`). It answers with the editor's instance
             and the frames that open it;
           - `forward(instance, frames)` and `forwarded()`: be-wasm-ops frames, each as
             encoded, carrying what the app sends on the editor's ops and on those it starts
             under them, and what the host answers.

           The host's end is be-host-runtime's `Forwarded`: a `Wire` on a relayed connection,
           whose ops carry be-wasm-ops' `RELAYED_BIT`, and `Instances` with the app's
           manifest's rules. `Instances` takes the editor in with `open_forwarded`, and serves
           it each pass with no screen of the host's. The root's `Wire` routes by op id alone:
           - `take_forwarded` holds every frame on a relayed op or on an op the app started
             under one;
           - `relay_forwarded` sends the host's frames unread.

           be-sdk-root's `Embedded::spawn_block` spawns the app, has the host open the editor,
           relays the opening once the app says hello, and hears the host for every app it
           opened that way. Tests:
           - be-sdk-app-host's `an_editor_a_roots_host_opened_is_served_by_the_host_through_the_root`;
           - be-host-runtime's `a_blocks_editor_a_root_embeds_is_sent_its_content_through_the_root`,
             in which a counter block's snapshot reaches an app through a root's wire.

           be-host under `--root` runs no block stack yet, so it answers `editor-of` and
           `open-editor` with why. Left for later:
           - the host's `Answer`s to editor calls (`commit-creation`, `render-clip`) on the
             relayed wire;
           - closing an editor short of the app stopping.
         - 4g7b2a (built). be-host runs its core under `--root`. `RootShell` owns `HostCore`
           and steps it each frame, before the root's frame:
           - the data directory and its lock;
           - the block stack, accounts and session;
           - the dev workspace, since `--root` and `--dev-workspace` now go together.

           The core opens no editor of its own here. It keeps the session's block
           (`HostCore::ensure_session`) for the root to show. It fills `core-view`, does its
           commands and answers its secrets (`root/block_stack.rs`), which takes 4g7g's
           block-stack part.

           It serves the root's editors (`serve_editors`):
           - `editor-of` names the session's own editor for the session's block, else the
             registry's default for the block's type (`EditorRegistry::editor_app`);
           - `open-editor` opens it in a `Forwarded` for the root's instance, once that
             instance runs the app.

           Host test: `be_root_in_place_of_the_hosts_ui_shows_its_signed_in_block_stack`.
           be-host's `root::block_stack` tests make agents for the root on a harness server:
           one asked for while another is made waits and gets its own credentials, and one the
           root stopped waiting for is removed. An editor whose frames break the protocol is
           stopped toward the root: be-wasm-sandbox-host's
           `an_instance_its_host_fails_stops_and_says_why` and be-wasm-root-host's
           `an_app_its_host_stops_stops_toward_the_root_which_draws_on`.
         - 4g7b2b (built). The session's view, in be-root (`guest/session.rs`):
           - `core-view`'s `showing`, the session's block, is shown through
             `Embedded::spawn_block`, filling the window under the root's own UI and following
             it (`Embedded::show`);
           - a notice over it shows the app loading, or why it stopped or could not start, in
             be-host's own words; Reload starts an app that stopped again;
           - `editor-app` carries the app's name for those words.

           The root's `paintings` carry each embedded app's screen as the app painted it
           (`Embedded::paintings`), under the id of the layer showing it, and `drive paint`
           places them. be-root test:
           `a_root_shows_why_the_block_its_session_shows_cannot_be_opened`. The host test
           above paints the workspace app's frame.

           Left for later:
           - grafting the app's accessibility tree, which needs the root to describe its own
             tree to the host first (4g7f's `tree` and `find`);
           - building `core-view` only when the stack or one of the core's areas changed
             (`stack_changes`, the areas' own state), rather than each frame, and comparing it
             with the last view handed (`root/block_stack.rs`'s `view`).
         - 4g7b3. Children: their placements, focus, the keyboard and drags, through
           the root's layers. Desktop windows stay with 4g7e. In parts:
           - 4g7b3a (built). Block children placed. be-sdk-root's `Embedded` keeps the
             children each view placed (`Embedded::placed`), those that come or go with the
             present that draws their holes (be-sdk-app-host's holds), owing the root a frame
             until it reads them, and `Children` shows them: each block child placed to run its
             editor (passive, active, live or in place) is the block's editor in an app of its
             own (`Embedded::spawn_block`, whose `editor-of` and `open-editor` now take a
             `shown-block`: the block with the editor and view block its placement names), asked
             for one screen (`CHILD_SCREEN`) the size of its place at its placer's raster, in a
             frame of its own when placed with one. Its layer goes under its placer's, or over
             it for `ChildLayer::Above`, clipped to what shows of it, and the placer is hit
             everywhere but where a child that takes input is (be-lib-geometry's
             `Rect::subtract`, moved from be-host-runtime's `pieces`); a passive child is hit
             nowhere. Their own children are shown the same way, and a block open above is
             refused in be-host's words. `be3:root`'s `host.show-children` tells the host how
             each view's children are (ids only: the editor showing each, whether it takes
             input, why it shows nothing); be-host's `child_status` makes each child's status
             from the manifest of the app it opened there, and `Forwarded::show_children`
             tells the placer (`Instances::set_forwarded_statuses`). be-root's session shows
             its editor's children, waits for those still opening before it settles, and
             paints their screens. Tests: be-sdk-root's `children::tests`; host test
             `be_root_in_place_of_the_hosts_ui_shows_the_children_its_session_places`. Left for
             later: a block's revision error (`be_host_stack::revisions::error`) reaches the
             placer's status only when the root shows its children again, not as the revision
             changes.
           - 4g7b3b (built). The keyboard. `Children` reads each editor's highest
             `keyboard-ask` from the placements it settles (`hear_asks`); one above every serial
             acted on for that editor is a new ask, and `Children::keyboard`, after the frame's
             layers are drawn, moves the keyboard into the child's layer if its app runs and its
             layer is drawn, or else out of the placer's own screen (`KeyboardMove::Out`), for
             the child to take once it shows while nothing else took it; only ever from within
             the editor's views and the children they show, and with no keys held
             (decisions/keys_are_never_held.md). be-root's session applies it before its own
             dialog, lock screen or accounts take the focus (`keyboard_into_children`). Each
             placer above the child the keyboard goes into, pressed or moved there, hears
             `child-focused` (be-root's `follow_focus`, `Children::focused_into`, `be3:root`'s
             `host.tell-placer`, `Forwarded::tell_placer`). A press anywhere but in an active
             child, or Escape in its placer's view, takes it back (`Routed::pressed`,
             `Children::revoke`), and its placer hears it is no longer active. `Forwarded`
             answers what an editor tells the shell and refuses what it asks (the root's views
             take none of it yet). Tests: be-sdk-root's `children::tests` and `tests`; host test
             `be_root_in_place_of_the_hosts_ui_hands_a_childs_keys_to_it`. Left for later: an
             ask acted on only while no popover or keyboard-taking layer of the editor's is
             open (they are inside its screen, so the root sees none), and the press that
             promotes a child replayed in it.
           - 4g7b3c (built). A child that takes its placer's frame, and docks that join across
             apps. An active or live child placed without a frame of its own in its editor's
             content view, when that view has a frame and the child shows its screen
             (`framing`, `content_view`), is asked for its placer's screen with the placer's
             frame drawn and its placed rect as the frame's `content` (`framing_screen`), shown
             over the placer, which is asked for its view with no chrome meanwhile
             (`Children::withdraw_chrome`, the outermost placer's too); it is hit but where its
             content shows outside its place (`framing_hit`, from the frame reports `Embedded`
             now keeps), and a press there or Escape in the placer takes the frame back from
             every child of that view that could take it (`frame_revoked`), as be-host's
             `revoke_frame_child` does. The outermost `Children` holds one be-sdk-app-host `Docks`:
             each editor registered with its block and app, what each placer places in dock tab
             bodies and the child taking its frame (`docked_children`, `Docks::placed`), what
             each app's docks said (`Embedded::take_docks`, from its layout commits and the
             presents that release them), and what the docks have for each editor handed to
             the host through `be3:root`'s new `host.dock` (the app protocol's
             `list<dock-update>`, encoded), which `Forwarded::dock` answers the editor's
             dock watch with. A docked child is asked for its frame `docked` and stays open,
             unplaced, while its dock is joined; the bodies of a joined dock's tabs and bars
             (`ChildContent::DockTab`, resolved through `Docks::resolve`) are each another
             screen of the owning editor's app, its view placed `Dock` and framed where it holds
             the content, shown where the outermost dock places it, with the owner's children
             in it, and an ask for the keyboard lands in the view holding the content. Tests:
             be-sdk-root's `children::tests` (a frame taken over has no painting: be-host's UI
             makes no child that takes a frame and shows under the root, since a canvas
             previews a card until focused and the root shows no previews yet); host test
             `be_root_in_place_of_the_hosts_ui_shows_a_joined_dock`, which makes a beui demo
             under be-host's own UI and restarts under the root (`HostTest::restart_as_root`),
             since nothing opens a block from the root's views yet; for it be-host asks its
             stack to list the blocks the root asks editors of (`Owner::Root`) and waits for
             that listing, where it refused any not listed yet once started on kept data.
           - 4g7b3d (built). Previews, opening blocks from the root's editors, and names:
             - A child placed as a preview runs its editor (`runs_editor`), asked for its
               preview view (`PREVIEW_SCREEN`, `preview_screen`, capped at 2048 pixels as
               be-host's `preview_scale` caps it) and shown under its placer, hit nowhere.
               `be3:root`'s `shown-child` says how (`shown-preview`: `view`, or `label` for an
               editor whose manifest names no preview view, which `editor-app.preview` tells
               the root before it spawns anything), and be-host's `child_status` answers
               `ChildPreview::Composited` once its editor opened (`Waiting` before, `Label` for
               one that draws none), available at once as be-host's own previews are.
               be-sdk-beui's `ChildBlock` leaves a hole for a preview the host says it
               composites, rather than drawing a picture; `Composited` is a variant of its own,
               since `Hosted`, the default with no status, still draws a preview's picture.
               Promoted (a canvas focusing its card), the child keeps its editor, asked for its
               main view.
             - Opening a block: under `--root` `HostCore::instance_heard` keeps the
               `open-block`s of the root's editors (`root_opens`), and be-host's `open_blocks`
               hands each to the nearest of the asking editor and those placing it (which
               `host.show-children` names) whose manifest accepts show requests, else the editor
               showing the session's block, as `shell_route::handler` does
               (`Forwarded::show_block`).
             - Names: an app reads block types' display names and icons from its
               `block-types-watch`, which it opens on its own connection, so under a root the
               root's wire answered it and nothing fed it: `be3:root`'s new
               `host.block-types` hands the root the host's catalog (with the programs'
               editors), which be-sdk-root tells every app it runs (`tell_types`). Names read
               "Untitled" only where a block had no name of its own.

             A child placed in a preview view is a preview too (`in_view`), as be-host-runtime
             makes it, and an editor shown only as a preview has no dock: be-host keeps previews
             out of its docks, and the root's `Docks` looped between it and the docked editor of
             the same block (a slide in the filmstrip and on the stage).

             - The camera of a placed child: a child placed by its parent without a frame of
               its own (a card, or one taking its placer's frame) keeps no camera, as under
               be-host, so `be3:root`'s new `host.place-content` tells the host where its content
               shows in its main view (`placed_content`: the whole view, or its placed rect while
               it takes the frame) and the size it wants, which the root hears from the editor's
               own `tell` (`Embedded::intrinsic`); be-host hands a pan-and-zoom editor that rect at
               its scale over that size (`view-changed`, `Forwarded::set_view`), as its `Camera`
               does, so the world stays where the card was.

             Tests: be-sdk-root's `children::tests`; host tests
             `be_root_in_place_of_the_hosts_ui_shows_a_presentations_slides_as_previews`,
             `..._hands_a_canvas_frame_to_the_editor_on_it` (the frame-takeover painting, a pan
             and zoom editor on a canvas) and `..._opens_a_block_its_files_open`. An embedded
             app asked for no screens (a child shown as its label) is asked for no frame and owes
             the root none (`Embedded::draws`): it presents nothing, so a frame asked of it waited
             out the root's stopped-clock budget. Left for later:
             - a search hit's place handed to the block it opens (be-host's `reveals`);
             - each preview is an app of its own rather than drawn once at its largest (4g7b3e).
           - 4g7b3e. A block in several places sharing one editor (one app per staged app
             rather than per child, a twin for a placement naming another editor, and a block
             previewed in several places drawn once, at the largest, each place showing its
             part), occluders, `fit`, and a child in place at its zoom, rotation and opacity.
           - 4g7b3f. Drags out of and into children (`drag-block`, `drag-accepted`), and a
             joined dock's tab menus, picks and details with the rest of the editors' shell
             messages the host's `Forwarded` refuses or drops today (`menu`, `details`,
             `child-menu-pick`, `leave-frame`); the docks' `take_picks` and `take_details` in
             `Children::hear_docks` are dropped until then. `intrinsic-size` is the root's
             already (`Embedded::hear_tell`, handed on with `host.place-content`): an editor's
             `tell` is not under its relayed op, so the root answers it and the host never sees
             it, and routing it to the host as well would read it twice.
           - 4g7b3g. The other kinds of child: web views, host panels, pickers, creations,
             artifact settings and rename forms, which the placer hears are unavailable until
             then.
       - 4g7c. Anchored views placed outside groups (`anchored`).
       - 4g7d. Editors and the camera (`editors`, `editors/camera`), and the chords along the
         focus: Tab and F6 between surfaces, and the palette's (`palette`, `intercept`).
       - 4g7e. Desktop windows (`desktop`): program windows as tabs, over the Wayland and
         win32 hosts' surfaces, which stay the host's.
       - 4g7f. drive's commands split as The root guest says: `tree`, `find`, `paint`,
         `settle`, `synced` and `offline` to the root as messages, the clock and the
         platform's in the host. The root describes its own tree, and grafts into it the trees
         of the apps it embeds (be-sdk-app-host's `Graft`).
       - 4g7g. be-host brings back under `--root` what the host keeps, each answering the
         capability 4g6 gave it (the block stack's `core-view`, commands and secrets came
         with 4g7b2a): the login session's lock and PAM (with its
         triggers: Super+L, idle and suspend lock through `host.lock`, since be-root's menu,
         like be-host's, has no Lock entry, and a person on the root has no other way to
         lock), the displays' keep changes, and the clipboard and notices already wired; then, once the
         real root passes the host tests that exercise be-host's own
         `src/ui` today, with their paintings compared, and only then, the root
         becomes the default and be-host's `src/ui` and `src/compositor` go, with the copies
         noted above (`SlowSave`, the memory graph).
5. The block stack as an instance the root guest spawns, behind capabilities, building on the
   web's stack worker with a memory of its own, and sending smaller updates.
6. Space as a guest, offscreen windows, and the host's OpenXR part, with late latching.
7. Strip the host: drop beui's widgets, text and fonts, the block stack, be-server and audio
   decoding. Update guides/apps/runtime.md, guides/desktop.md, guides/running/web.md,
   guides/running/drive.md, guides/beui/space.md, guides/pan_and_zoom.md and
   guides/testing_a_gui.md.
