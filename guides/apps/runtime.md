# How the host runs apps

## Discovery and staging

The build stages every `be-app-*` crate beside be-host: its module built for
`wasm32-wasip1-threads` with the app profile (apps link wgpu with real
backends, pure guests only with the custom one), its manifest as
`<app id>.app.json`, its type module and the shared libraries
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
it loads (Liftoff, then TurboFan on hot code), with no code cache on disk;
each library is compiled once per process and shared between isolates as a
`v8::CompiledWasmModule`. Each guest thread has an isolate of its own (a
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
reaches them as one revision signal per app.

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
once per host frame (`Pacing::asked_in`). A child whose placer draws in this
frame is held until the placer's layout commit lands (`Host::request_frames`),
so it is asked once, at its new place; `Host::held_behind` breaks a cycle of
two apps placing each other. Input the host routes always brings a request.

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

It waits until the frame's deadline (`Host::frame_deadline`): the display's
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
frame's serial (`be-host-runtime/src/sent.rs`).

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
(`Laid::presented_by`), so a child leaves or arrives in the frame whose
painting fills or punches its space: a closed child never shows its hole in
the placer's old painting. The commit's dock messages wait with its children
(`Laid::docks`): which tabs a root shows decides which of the tab bodies it
places are drawn (`docks::resolve`), so the body of a tab switched away from
and the body of the tab switched to change in the same frame. Answers held
after it wait with it, in order. The frame drawn after that layout waits too
(`Runtime::received`, from the sequence of the last painted present heard
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
that view's last frame (`Kept` in `runtime.rs`: its placement, textures and
painting) while it still holds the view, and draws it if the view is shown
again, until the app's new frame of it has been drawn; so a text's format bar
never blanks for a frame when a tab switch brings it back before its app
answers. The kept frame goes when that frame is drawn, the instance closes, or
the app restarts, and only the eight views of an app hidden last keep theirs
(`MOST_KEPT`): one hidden longer may show empty for a frame when shown again.
The web keeps none: its surfaces are the app worker's canvases.

An anchored view the app closes leaves the same way (`Runtime::place`, `Closing`): the app sends
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
view's top-left, cropped and never stretched (`presenter.rs`, `unstretched`).
In the browser the answer always lands in a later frame, since the host
cannot block on a worker.

Each frame an app draws is two steps of its guest: one lays out and sends
`laid-out`, the next paints. The calls an app makes without waiting on them
(`layout`, `laid-out`, `frame-needed`, `missing-characters`) are answered with
the next thing the host sends it (`Wire::acks`), since an answer alone would
step it for nothing.

A call that changes nothing drawn costs no frame, and a frame runs only the
instances something touched, so an idle instance costs nothing. Work off the
frame (the PDF editor renders pages on its own thread) asks for a frame
through `request_repaint` or a `be_sdk_beui::native::Waker`.

## Headless

`be_host_runtime::Headless` runs the same runtime with no beui: no window, no
presenter and no gpu (`install_headless`, so an app that asks for the device
is told there is none). It steps when something wakes the host, or when the
runtime asked to step again, rather than when a display wants a frame: a step
starts the host frame (`begin_host_frame`, which be-host's beui frame calls
too), then `begin_frame`, `poll`, `flush` and `settle`, as be-host's frame
does. Nothing in it is laid out or painted, so it runs only instances with no
screen, host components (guides/apps/host_services.md, "Host components").
`be` runs them this way (plans/headless_apps.md).

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
