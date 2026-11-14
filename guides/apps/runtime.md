# How the host runs apps

## Discovery and staging

The build stages every `be-app-*` crate beside be-host: its module built for
`wasm32-wasip1-threads` with the app profile (apps link wgpu with real
backends, pure guests only with the custom one), its manifest as
`<app id>.app.json`, its type module, the shared libraries
(guides/app_libraries.md) and a precompiled `.cwasm` of each, which the host
maps instead of waiting seconds for Cranelift. A `.cwasm` made by another
wasmtime or engine configuration (`configuration()` in be-wasm-host, shared
by the precompiler and the host) is ignored and the `.wasm` compiled instead,
which is how a cross-compiled build runs. `apps.json` lists the manifests.
They sit beside the executable natively, in the APK's assets on Android and
at the bundle root in the browser; native and Android share one loader
(`editors/app/discovery/bundle.rs`).

## Backends

Every platform runs the same host: `be-host-runtime/src/runtime.rs` owns the
runtimes, their instances, input, errors and restarts, and a platform only
implements a `Backend`: `wasm.rs` (wasmtime, everywhere but the browser) or
`web.rs` (a worker). Both are a `be_wasm_guest_host::Channel` under a
`Worker`, which queues frames, steps the guest when it is ready and finds a
hung step. Neither side polls: everything an app says wakes the host, in
order, and the reverse.

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
children) is applied at once; the rest (children coming or going, window
requests) waits for the next `be_host_runtime::poll`, where the browser takes
every answer, so host state changes at the same point on both. It also waits
until the app has sent the `PresentCommit` that follows that layout
(`Laid::presented_by`), so a child leaves or arrives in the frame whose
painting fills or punches its space: a closed child never shows its hole in
the placer's old painting. The commit's dock messages wait with its children
(`Laid::docks`): which tabs a root shows decides which of the tab bodies it
places are drawn (`docks::resolve`), so the body of a tab switched away from
and the body of the tab switched to change in the same frame. Answers held
after it wait with it, in order.

A view its app has not reported a frame of yet (a dock tab's body, mounted
again each time its tab is shown) is filled with the backdrop its instance
last reported for another view, else with what its placer's view is filled
with (`Behind` in `app_view.rs`), so it never shows what is behind its placer
while its app draws it.

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

## When an app stops

An app that panics stops alone. be-wasm-guest-abi's panic hook hands the
message to the host through `be3_host.host_panic` (a guest's stderr does not
reach the host); every view of the app then shows `CrashNotice` with a Reload
button, and the host shows a toast. A panic in host code working for one app
(inside `Runtime::contain`), a wasm trap, a failed session, and a step not
finished within `STEP_TIMEOUT` stop that app the same way.

Stopping drops the backend, which interrupts the guest: apps run with
wasmtime's epoch interruption, so every thread traps at its next loop or call,
including one blocked in `memory.atomic.wait`; in the browser the worker and
its threads' workers are terminated. Every such stop goes through
`Runtime::crashed` then `Runtime::stopped`; the Apps debug window's Crash and
Kill buttons try both paths.

`Frame::stopped` is how a view says it will draw nothing more (closed, or
failed with the panic); the host shows the error in its place with a restart.
