# How the host runs apps

## Discovery and staging

The build stages every `be-app-*` crate beside be-host: its module built for
`wasm32-wasip1-threads` with the app profile (apps link wgpu with real
backends, pure guests only with the custom one), its manifest as
`<app id>.app.json`, its type module, its worker modules under
`workers/<app id>/` (guides/adding_an_app.md, Workers) and the shared libraries
(guides/app_libraries.md), only as `.wasm`: the host compiles each module as
it loads. `apps.json` lists the manifests.
They sit beside the executable natively, in the APK's assets on Android and
at the bundle root in the browser; native and Android share one loader
(`editors/app/discovery/bundle.rs`).

## Backends

Every platform runs the same host: `be-host-runtime/src/runtime.rs` owns the
runtimes, their instances, input, errors and restarts, and a platform only
implements a `Backend`: `wasm.rs` (V8, everywhere but the browser) or
`web.rs` (a worker). Both are a `be_wasm_guest_host::Channel` under a
`Worker`, which queues frames, steps the guest when it is ready and finds a
hung step. Neither side polls: everything an app says wakes the host, in
order, and the reverse.

`wasm.rs` runs the guest through `crates/be-wasm-host`, which embeds V8 (the
rusty_v8 `v8` crate) and runs every wasm guest outside the browser: apps and
their threads, type modules and the wasm app tests. V8 compiles a module when
it loads (Liftoff, then TurboFan on hot code); each module's bytes are
compiled once per process and shared between isolates as a
`v8::CompiledWasmModule`. Libraries are trimmed as they load to the exports
the staged apps import (`keep_exports`), since V8 wraps every export of an
instance. What TurboFan optimised is kept in a code cache
(`be-wasm-host/src/cache.rs`) under the data directory's `code-cache/`, or
`BE3_CODE_CACHE` (read-only with `BE3_CODE_CACHE_READ_ONLY`, as the host
tests read one their build made, crates/be-host-tests/apps.bzl). Each guest thread has an isolate of its own (a
`v8::SharedIsolate`, locked by the thread that runs it):

- **Imports.** The gpu and transport imports are generated from
  `gpu_local_imports!` and `transport_functions!`. Where the guest's memory
  is shared they are V8 fast-API calls, reached from wasm code with no
  JavaScript between (`--wasm-unsafe-fast-api-wrapper`); where the guest has
  its own memory they are ordinary V8 function calls.
- **WASI.** Preview1 is implemented in Rust (`src/wasi.rs`). An app gets no
  files, arguments or environment, as `wasi.js` gives it in the browser; the
  test harness also gets arguments, environment, stdout and its preopened
  directories.
- **Threads.** `thread-spawn` starts an OS thread with a new isolate, handed
  the shared memory as a serialized `WebAssembly.Memory`.
- **Linking.** The be-wasm-dylink `Plan` is carried out over the JavaScript
  WebAssembly API, called from Rust, as `crates/be-host/web/dylink.js` does
  in the browser.
- **Pure guests.** A type module is validated without threads, shared memory,
  relaxed SIMD, garbage-collected types or a second memory, and its memory's
  maximum is capped (`MEMORY_LIMIT`). The app host has no fuel and no
  canonical NaNs.
- **The sandbox.** `be_wasm_host::sandbox` is the native half of the sandbox
  API (plans/host_as_platform_adapter.md). `spawn` starts an `Instance` of a
  `Source`, on a thread and isolate of its own, with a memory capped at its
  `Config::memory_limit`; an `Instance` is a `Worker` over that thread's
  channel, and dropping it terminates its isolate. A source is either
  - a module from bytes: `compile` checks a pure guest's module and compiles
    it on a thread of its own, once per process for the same bytes (by
    SHA-256, held while anything holds the `Module`, never in the code cache),
    with the memory it defines turned into an import
    (`be_wasm_dylink::importing_memory`), so each instance gets a memory of its own
    size; or
  - a staged app's module (`Staged`): the app's entry point or one of its
    workers, and an `Open` that reads it and its libraries, loaded as the host
    loads that app, with WASI and, unless its `Config::display` gives it the host's device, no
    display (its gpu calls fail). An instance with a display keeps what each of its surfaces
    presented last (`Instance::surface`) until the app lets that surface go, which the gpu host
    reports with what it presented (`Produced::released`). A be-sdk app lets go of each screen
    surface its new layout drops (`Surfaces::place`); another guest, only of those it drops
    itself. An app run as `Presenting` reports what it presented
    each step (`PresentedTexture`), for an instance as for be-host-runtime's apps. A type module
    runs as a plain `App`, which presents nothing.
    be-host-runtime's `staged_app(app id, module)` is the one place that maps a
    module's name to a file: the manifest's `entry_point`, or one of its
    `workers` under `workers/<app id>/`; any other name is refused. Its own app
    starts load through `Staged` too.

  `crates/be-wasm-sandbox-host` serves it to a guest as `be3:sandbox`
  (`crates/be-wasm-sandbox-api`), whose `instance` constructor takes a
  `source`, `module(module-id)` or `app({app, module})`. Its `Server` is given
  a `Loader` for staged modules, which the host plugs in (`staged_app`), since
  finding staged apps lies above be-wasm. be-host-runtime serves every app one
  (`Wire::with_sandbox`, polled with the app's frames in `Backend::receive`,
  dropped when the app stops or fails): `Server::for_app` asks the loader
  (`staged_worker`, which answers only workers, never the entry point) only for
  the app's own id, which an empty `app` names, refuses compiles, caps each
  worker's memory at `WORKER_MEMORY_LIMIT` and runs at most `WORKERS` at once,
  so an app spawns only its own workers (guides/adding_an_app.md, Workers). A worker gets no gpu: a
  worker computes and sends its result back as bytes, which the app uploads
  itself, so no device is handed to it, and on the web, where each worker
  would need a device of its own, it needs none. `crates/be-wasm-root-host`
  serves the root guest one with `Server::new`, which may spawn any staged app
  and compile modules, and `with_display`, so the staged apps it spawns draw on
  the host's device: the root names what one presented in its layer list as
  `Surface::Screen { instance, surface }`, `instance` being the op of its
  `instance` resource, and `Root::surface` answers the texture for it, which
  never leaves the GPU. The input remapper runs on
  `be_wasm_host::sandbox` (guides/desktop/login_session.md, Input remaps). In
  the browser the same `Server` answers an app from be-wasm-sandbox-host's `web`
  half: each instance is a web worker of its own (`WebGuest`), started from the
  URL be-host-runtime's `worker_file` maps the name to (the native loader maps
  it with the same function), and linked by be-host's `web/worker.js`, which
  links it as `app.js` links an app but opens no gpu device. A module from
  bytes is compiled on the page with `WebAssembly.compile` (once per page for
  the same bytes, by the SHA-256 `crypto.subtle` takes, while anything holds
  the `Module`), with its memory imported as natively
  (`be_wasm_dylink::importing_memory`, which both share), and refused if it
  imports anything else but its transport. The rewrite runs on the page,
  synchronously, in one pass over the bytes; if modules from bytes grow large,
  move it to a worker. Each instance of it is a worker the
  page posts the compiled `WebAssembly.Module` to (`WebGuest::start_module`),
  with the pages of a memory capped at the instance's `memory_limit`, which the
  worker makes and instantiates it with. The browser cannot cap a staged app's
  worker's memory.
- **The root guest.** `crates/be-wasm-root-host`'s `Root` runs a root guest
  (plans/host_as_platform_adapter.md, The root guest) on the calling thread,
  stepping a `be_wasm_host::App` inline rather than on a thread of its own.
  `Root::frame` starts the root's `frames.frame` op (`be3:root`,
  `crates/be-wasm-root-api`) and steps the root until it answers; while the
  frame is unanswered (pending) it serves the root's calls (the sandbox, and
  `host.deadline`, held until the frame's deadline passes and answered at once
  outside a frame or after it) and waits on a bell
  that the root's instances, compiles and own wakes ring, giving up on a root
  still busy `with_patience` (`STEP_TIMEOUT`) past the deadline.
  `Root::input` starts the root's `input.deliver` op with the window's input
  (be-lib-ui-types' `input::encode`, since be-wasm crates use only be-wasm
  ones), which the root takes in with the frame being drawn (on the page, at
  its next resume), or else ahead of the next frame, so it routes by the
  layers it drew last; the host does not await the answer, which the root
  gives once it has routed. The stepping is `Core`, shared with the
  browser's half (`web`), where the page may never wait: `Root::load` compiles the root with `WebAssembly.compile` and
  instantiates it on the page, answering its transport itself, and
  `Root::frame`, called from `requestAnimationFrame`, steps it as far as it
  goes and returns. Whatever the root waits on queues a microtask that resumes
  it (its wakes name it by number, since a wake is `Send` and the page's
  values are not), as does a timer at the deadline and at the patience past
  it. A root steps for `SLICE` (4 ms) at most at once, at least one step, and
  one with more to do resumes as a task of its own (a `MessageChannel`
  message), so a busy or self-waking root never holds up the page's input and
  rendering. The frame goes to the callback `frame` was given, inline if the
  root answered at once. Only tests run a root today: natively
  be-wasm-root-host's, and in Chromium
  `//crates/be-wasm-root-web-testing:web-test`, which also serves a guest on
  the page the sandbox for modules from bytes. That page is the crate's
  `:web-module`, a program-role module through wasm-bindgen, and reads
  be-wasm-root-testing's constants as a library: only that crate's `:module`,
  built with its `module` feature, exports a guest's entry points, so the page
  links none.
- **Apps in a root.** A root guest embeds apps with `crates/be-sdk-root`, the
  host's half of the app protocol run inside the root. `Embedded::spawn`
  spawns a staged app's entry point through the sandbox and runs it as its
  host: be-sdk-app-host's `Wire` over the instance's channel, which a task on
  the root's executor hears (`be_wasm_ops::guest::spawn`), and a `Run` with
  its session, pacing and holds. The editor and its screens are queued until
  the hello; `Embedded::frame` asks for a frame when the pacing says one is
  due and answers once the app presented, which the root races against
  `host.deadline`; `Embedded::layer` names a screen the app painted as a
  `Surface::Screen`, scaled from its pixels to its points. `Embedding` routes
  each window's input (`Windows`: a `Router` per window over the layers drawn
  last) and sends what lands on a screen to its app as `AppCall::Input`.
  Input for a window the last list leaves out is still routed, against the
  layers it had last, until its presses lift, and only then its router goes;
  one never drawn routes against none. It refuses what it serves no app yet
  (block calls, host services, the network, the shell but `tell`), and
  restarts nothing. be-sdk-root-testing is a root built on it, which
  be-wasm-root-host's tests run with the triangle on a stand-in device.
- **V8's heap.** No guest may declare garbage-collected types (structs,
  arrays) or more than one table, and a table holds at most a million entries
  (`engine::Shape::bounded`): V8 stops delivering a termination once an
  isolate's heap is held at its limit, so a guest that could fill it could not
  be stopped. Each isolate's heap is limited too (`HEAP_LIMIT`), as a
  backstop. A toolchain that starts emitting wasm GC for an app meets this
  first.
- **Traps.** A trap's backtrace is V8's stack (`--stack-trace-limit=100`);
  `BE3_PERF_MAP` turns on `--perf-basic-prof` (guides/performance.md).

On the host, each view of an instance is a retained beui component, `AppView`
(`crates/be-host/src/compositor/app_view.rs`), which mounts the screen, places
it from the `Embed` it fills and forwards input to it; `editors.rs` builds
tabs, takeovers, embedded children and previews from it. What an app sends
reaches them as one revision signal per app. An `Embed`'s `on_place` hears a
placement at once, but being unplaced only if the frame ends with it so: an
`Embed` whose node moves to another parent in one frame (a kept panel dropped
into another dock pane) is never heard unplaced, so its app neither hides nor
presents again for the move (`EmbedState::hold`, `Document::show_content`).

## The layer list

Every app surface the host shows is drawn from a `be_lib_layers::Layer`
(plans/host_as_platform_adapter.md, The layer list): its surface by handle (`Surface::Screen` or
`Surface::Anchored`, with the runtime's slot), a transform from the surface's pixels to the
window's points, the clip in its pixels, the rounding of its own corners and its opacity.
`be-host-runtime/src/layers.rs` makes one from each blit the host's frame paints
(`Blit::screen_layer`, `Blit::anchored_layer`).

Natively each of the host's windows is composited from its list (`beui_draw::compose`, which
`presenter::install` turns on; beui-renderer-wgpu's `composite.rs`). The window's renderer
(present.rs's, and each display's in beui-platform-drm) renders the host's UI into textures of
its own: `Surface::Root(0)`, and above each anchored view it shows, one more for what the UI
paints after that view (`Root(1)` above the first, as the web stacks `#layers` over its canvas). Every drawing that
shows a surface lists it (`Draw::layers`): `AppDrawing` its screens and anchored views,
be-host-wayland's `WindowDraw` and a `TextureDraw` made `hosted` (win32's captured windows) their
program windows as `Surface::Host`. Screens and program windows are still drawn in place, in the
UI's paint order (`Placed::inline`), so they blend over what lies beneath them and fade with the
groups they are in; only an anchored view, which lies over the UI, is left to the renderer,
which draws the window from the list in a pass of its own: `Root(0)`, the first anchored view,
`Root(1)`, the next, and so on, so what the UI paints between two anchored views (a second
view's scrim) lies between them. An anchored view inside a group or filter splits the roots
after it, so it ignores the group's opacity and filter, and what the group paints after it lands
below it; the renderer says so once on stderr. Each anchored view shown costs a root texture the
window's size, on top of `Root(0)`.

The renderer finds every drawing the window shows each frame by walking only the displays
that hold drawings (`Display::drawings`), and keeps what each placed while it stays where it
was (`Drawing::identity` and its `DrawAt`), so only a damaged or moved drawing is prepared. It
visits a drawing that placed an anchored view whether or not it is damaged, so the roots split
where it paints; when the anchored views change, the frame is painted whole, since what the UI
paints after them moves between the roots. Finding the drawings costs a walk of the displays
that hold them and their ancestors each composited frame, on the CPU; keeping the list across
frames and patching it from the tree's changes would remove it. A layer's id names the drawing that shows it
(`layers::id`), so a view's parts below and above its children, or a preview shown twice, never
share one. The pass draws again only where the list's damage against last frame's says
(`WindowLayers::damage`, each root's own damage through its layer, overlapping rects merged),
which is what `Renderer::scissors` reports and the e-paper display reads. Paintings are
unchanged: they come from `Paints`, not from the GPU. On the web `layers::frame` makes the
frame's list (Apps on the page in guides/running/web.md) and the page applies it as CSS. The
host's input goes through beui's forwarding catchers (`AppView`), which route by
`be-lib-input-router`'s rules over beui's hit test; the list's own hit test is not used yet.

## Frames and pacing

An app draws only when the host asks for a frame. When something changes what
it shows, it says a frame is needed and waits, so a burst of input is drawn as
one frame. Its last frame says how long it wants to wait before the next; the
host holds that until due and asks once in its own frame, so an animating
editor draws once per host frame and an editor not on screen is asked for
nothing.

Each step's time is recorded per app in the host's performance report (the
pacing group): the whole step, "GPU calls" (the time inside every gpu import
except those that only record into a pass or bundle, such as draws, which are
left untimed since the clock costs more than they do and count in the step;
`be_wasm_gpu_host::only_records`) and "submit".

### When the host asks

The request leaves as soon as the frame's input is delivered when the host has
nothing to lay out (so the editor draws while the host finishes), and
otherwise after the host's layout, at `settle`. A runtime is asked at most
once per host frame (`Pacing::ask`). A child whose placer draws in this
frame is held until the placer's layout commit lands (`request_frames` in
be-host-runtime's `runtime.rs`), so it is asked once, at its new place;
`pacing::held_behind` breaks a cycle of two apps placing each other. Input the
host routes always brings a request: whatever the app answers with a frame
(input, an action, a change it shows) goes through `Run::send_for_frame`, which
asks for one if it sends anything, and the rest through `Run::queue`; both hold
it until the session runs.

The pacing is be-sdk-app-host's (`be_sdk_app_host::pacing`), so any host of the
protocol paces as be-host does: `Pacing` decides when one app is asked and what
its answers count for, `FrameClock` keeps the deadline and settles against it,
and `held_behind` orders placers before their children. be-host-runtime keeps
what it decides from (which apps are asked, their placers, the bell) and sends
the `Draw` itself.

Input reaches an app on its own stream, apart from the screens it is given, so
input aimed at a screen the host just took back can follow the screens that
dropped it: the app ignores it (`be_sdk::session::ClientSession`), and refuses
input for a screen it was never given.

### Settling

After laying out, the host sends each runtime its views' new sizes and places,
asks for a frame where that changed anything, and waits for every answer at
once before painting (`be_host_runtime::settle`, from beui's
`Document::on_laid_out`). When an answer changes what the host lays out (a
placer resizing a child), beui lays out and runs the hook again
(`Document::run_laid_out_again`, up to `LAYOUTS_PER_FRAME`), so the resized
child is drawn in the frame its placer moved it.

It waits until the frame's deadline (`FrameClock::deadline`): the display's
next present less the time the host's own work after settle has been taking.
While a driven host's clock is stopped (guides/running/drive.md) it waits for
every answer, up to `STOPPED_CLOCK_FRAME_BUDGET`, so a slow machine never
changes which frame shows an answer, and an app that ran out of time is drawn
again only once the clock moves.

### Layout and present commits

Everything an editor lays out against is followed by a `configure` with a
serial. Each frame the app draws comes back as two commits with the serial it
last heard (decisions/app_frames_split_into_layout_and_present.md): a
`LayoutCommit` as soon as it has laid out (view sizes, frame reports, window
requests, child placements, docks), then a `PresentCommit` with the painted
surfaces. The host places and asks the frame's children to draw while the
parent paints. It reads a frame's placements against the state at that
frame's serial (`be-sdk-app-host/src/sent.rs`).

A present commit answers the input sent before its frame was asked for
(`Pacing::answered`, which the keyboard gift's `GiftTo::Answer` waits on)
unless it sets `input-waiting`: be-sdk-beui-platform does when input for a
view whose window its frame opened was taken in only after that frame
(guides/apps/framework.md), and the host then counts the next frame, up to
`INPUT_WAITING_FRAMES` in a row.

Only what draws the frame (texture, layout, placements that move existing
children while none comes or goes) is applied at once; the rest (children coming or going, window
requests) waits for the next `be_host_runtime::poll`, where the browser takes
every answer, so host state changes at the same point on both. It also waits
until the app has sent the `PresentCommit` that follows that layout
(`Laid::presented_by`; the holds are be-sdk-app-host's `holds`, which
be-host-runtime drives: `Holds`, `Shown` and `Presents`), so a child leaves or arrives in the frame whose
painting fills or punches its space: a closed child never shows its hole in
the placer's old painting. The commit's dock messages wait with its children
(`Laid::docks`): which tabs a root shows decides which of the tab bodies it
places are drawn (`docks::resolve`), so the body of a tab switched away from
and the body of the tab switched to change in the same frame. Answers held
after it wait with it, in order. The frame drawn after that layout waits too
(`Runtime::received`, `Holds::waits_for_children`, from the sequence of the last painted present heard
before it, so also when it arrives ahead of its `PresentCommit`): shown at
once, its painting (a stacked dock's bar showing the document opened from the
files) would sit a frame over the children it replaces. It is shown at the
poll that applies them, in the same host frame. Since that painting waits, the
children that only move in the same layout, and the views' sizes (a fit
child's), wait with it rather than change a frame early. A stop or restart
drops a frame held this way. Holding it never lets the app draw into the
texture on screen: each surface has two textures; the app presents only to
answer a `Draw` (`be_sdk::runtime`); the host sends no `Draw` while one is
unanswered, and at most one a pass, after that pass's poll; so the next `Draw`
goes out after the poll that shows the held frame.

A view its app has not reported a frame of yet (a dock tab's body, mounted
again each time its tab is shown) is filled with the backdrop its instance
last reported for another view, else with what its placer's view is filled
with (`Behind` in `app_view.rs`), so it never shows what is behind its placer
while its app draws it.

A view the host stops showing (a dock tab, or a dock bar hidden with its tab)
is left out of the app's next layout, which lets its surface go. The host keeps
that view's last frame (`Shown::keep_left_out` in be-sdk-app-host's `holds`: its placement, textures and
painting) while it still holds the view, and draws it if the view is shown
again, until the app's new frame of it has been drawn; so a text's format bar
never blanks for a frame when a tab switch brings it back before its app
answers. The kept frame goes when that frame is drawn, the instance closes, or
the app restarts, and only the eight views of an app hidden last keep theirs
(`MOST_KEPT`): one hidden longer may show empty for a frame when shown again.
The web keeps none: its surfaces are the app worker's canvases.

An anchored view the app closes leaves the same way (`Runtime::place`, `Shown::place`): the app sends
its screens ahead of the frame's commits, and the host keeps the closed view, showing its last
frame, until the present that follows and, where it composites the app's surfaces, that present's
pixels have arrived. What the view showed may have moved into another of the app's surfaces (a
dragged dock bar settling back into its band), so it is never missing for a frame in between.

A child that has taken its parent's frame does not wait on the parent to learn
where the camera put it: the host moves it with the camera itself and asks
both editors in the same frame (guides/pan_and_zoom.md).

### Events for the host

What an instance asks of be-host one way (to leave its frame, move its view, open a
block, the answer to a clip render, its focus, its children's commits, the child it
gives the keyboard to, ...) is an `InstanceEvent` that be-host-runtime posts, as
the app's call arrives, to the
mailboxes handed to `be_host_runtime::listen` for that instance or for every
instance; nothing in be-host polls per instance for it. A mailbox hears it when its
desk delivers: be-host delivers the frame's mail in the document after
`be_host_runtime::poll`, so an owner acts in the frame the app spoke and may touch
the document (focus, a drag). The compositor acts on what needs no view (actions,
drags, child menu picks, view changes, which wait for the instance's camera until
it closes); `compositor::hear` gives a component its instance's events,
`compositor::own_frame` makes one frame the one that acts on details and leaving,
and `HostCore::instance_heard` takes focus, artifact watches and children's commits.
An instance's last event is `Closed`.

### Late answers and idle editors

An editor that answers late is shown in a later frame without holding the host
up. Until then its last painting is drawn at the size it was painted, from the
view's top-left, cropped where the view is smaller and never stretched; where
the view is larger, the rest shows the view's backdrop (the app's clear colour,
`Backdrop` in `app_view.rs`) within the view's last placement, and beyond it what is behind
the view (its placer's backdrop or the host's surface), until the app reports the new size. A
present says the placement each surface was painted at
(`presented-frame.painted`), which the host keeps per surface, pruned against each layout it publishes
(`presenter::Shared::holds`): every path that draws a painting into its view
(the native texture, the web's canvas and the hole it punches for it, paint
snapshots, damage) places it through `Blit::shown_at` and `presenter::unscaled`
from that size and raster over the view's zoom, never from the size the layout
asked for, since a layout commit can land before the present that brings
textures of its size. Natively the size is the texture's own; in the browser
the canvas keeps its natural size, the backing store its worker last showed,
placed with a matrix from its pixels and clipped in them
(`presenter::canvas_style`), so the browser never scales it into a box of
another size. A block preview is a picture and is fitted to each place it is
shown at. In the browser the answer always lands in a later frame, since the
host cannot block on a worker.

Each frame an app draws is two steps of its guest: one lays out and sends
`laid-out`, the next paints. The calls an app makes without waiting on them
(`layout`, `laid-out`, `frame-needed`, `missing-characters`) are answered with
the next thing the host sends it (`Wire::acks`), since an answer alone would
step it for nothing.

A call that changes nothing drawn costs no frame, and a frame runs only the
instances something touched, so an idle instance costs nothing. Work off the
frame runs in a worker (guides/adding_an_app.md, Workers; the PDF editor
renders pages in one): its answer wakes the host, which steps the app with it,
and the app asks for a frame through `request_repaint` or a
`be_sdk_beui::native::Waker`.

## Headless

`be_host_runtime::Headless` runs the same runtime with no beui: no window, no
presenter and no gpu (`install_headless`, so an app that asks for the device
is told there is none). It steps when something wakes the host, or when the
runtime asked to step again, rather than when a display wants a frame: a step
starts the host frame (`begin_host_frame`, which be-host's beui frame calls
too), then `begin_frame`, `poll`, `flush` and `settle`, as be-host's frame
does. Nothing in it is laid out or painted, so it runs only instances with no
screen, host components (guides/apps/host_services.md, "Host components").
The runtime keeps passing while one runs (`Instances::has_hosts`) without
asking for frames.

`be` runs them this way (`be_tool_cli::hosting::Hosting`, plans/headless_apps.md),
and finds its apps as be-host does (`be_tool_cli::release`), through
`be_host_stack::discovery`: the manifests, the type modules it reads blocks through
(`be_host_stack::types::app_sources`, guides/type_modules.md, "Hosts") and the apps whose
host components it runs. No app's wasm, manifest, type module, list or hash is compiled
into it, so an app changing never relinks `be`. A build that was
not deployed reads the apps staged beside it: `//crates/be-tool-cli:dev` stages every
app's manifest and type module, and the modules and data of the apps with a host component
(`HOST_APPS` in its BUCK) and those `-c be3.apps=NAME,...` names, as be-host's `:dev` takes
them (a program's `modules` entry may be `None`, which stages the manifest alone);
`:terminal-host`, what a development build uploads as a terminal host (guides/terminal.md),
stages every manifest and type module and no module. The tests stage the counter through
`BE_BUNDLE` and install every editor's manifest. `:native`, `be` alone as a deploy ships it,
finds none built here. A deployed `be` ships alone, so it reads its release's apps: the
web bundle a deploy publishes at `data/BUILD/web/` on the mirror (guides/hosting.md,
Deploying), staged as be-host stages them, served to be-host-stack's discovery
through `use_assets` (a reader in place of the bundle beside the executable, as
Android's assets are). Serving it downloads nothing (`be` reads its own `build-info.json`,
never the release's), so a command that needs no block type fetches nothing; the first that
names a type reads `apps.json` and the manifests, and the first that reads a block's content
starts the type modules (`Tracker::types`, every one, since `be sync` asks each type for its
plain form), then a host component's module and libraries only when one runs.

What it trusts is what it shipped with, as be-host trusts the apps it ships
with. The deploy writes the release's `apps` into the `build-info.json` beside
every executable it ships, `be`'s too: the bundle's URL and the SHA-256 of its
`app-hashes.json` (be-host's `app-hashes`, a `bundle_hashes` over its `:apps`,
which the web bundle carries). `be` downloads that list and refuses it unless it
hashes to what its `build-info.json` names, then refuses any file that does not
hash to what the list says, so what the mirror serves later cannot change which
manifests, type modules or apps a deployed `be` reads. Each file is kept in `be`'s
cache (`BE3_CACHE`'s `apps/`) under its hash, downloaded through `HTTPS_PROXY` with the
certificates be-block-client trusts (`tls_config`, which takes `SSL_CERT_FILE`'s); a missing file, a mismatch or a release
with no list fails, naming the file. A terminal host's `be` gets the host's own `apps`
(`Metadata::for_be`). A `be` deployed with `--skip-web` names none, so it reads no block
type and runs no host component, and says so (`be_tool_cli::apps::found` reports what
discovery could not read, once, when a command first needs the apps). A development build's
terminal host gets `:terminal-host`'s files uploaded beside it instead (guides/terminal.md).

## When an app stops

An app that panics stops alone. be-wasm-guest-abi's panic hook hands the
message to the host through `be3_host.host_panic` (a guest's stderr does not
reach the host); every view of the app then shows `CrashNotice` with a Reload
button, and the host shows a toast. A panic in host code working for one app
(inside `Runtime::contain`), a wasm trap, a failed session, and a step not
finished within `STEP_TIMEOUT` stop that app the same way.

Stopping drops the backend, which interrupts the guest: the host terminates
its isolates (`terminate_execution`), so every thread stops at its next loop
or call, including one blocked in `memory.atomic.wait`; in the browser the worker and
its threads' workers are terminated. Every such stop goes through
`Runtime::crashed` then `Runtime::stopped`; the Apps debug window's Crash and
Kill buttons try both paths.

`Frame::stopped` is how a view says it will draw nothing more (closed, or
failed with the panic); the host shows the error in its place with a restart.

A restart (the Reload button, `restart_app`, or a new gpu) stops the app and
starts its module afresh in the same `Runtime` (`Runtime::restart`): its
instances and their views stay, and the new run is sent their content, blocks,
keyboard focus and children's statuses again (`Instances::reopen`). The host
keeps what one run said and was told in a `Run` (be-sdk-app-host) per app, and
`Run::restart` says what goes: the old run's session, queued calls, held
answers, its layout and kept frames, presents and frames in flight, what its
docks declared, and each view's accessibility tree and actions, whose pane stays, empty, until
the new run describes the view. The new run is told the fonts, whether to
describe and the UTC offset again once it runs (`Run::tell`). The frame held
for children and the presenter's textures go with the host's own state.
