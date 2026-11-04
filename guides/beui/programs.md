# beui programs

How a beui document becomes something that runs: a standalone program, the
layers stacked between it and the platform, the inspector, and a beui editor
inside an app. What a platform does underneath (the runner, input, clipboard,
renderers, Android, the web, DRM displays) is in guides/beui/platforms.md.

## A standalone program

A standalone program depends on beui-host as well as beui. It builds its
document once and implements `beui::Program`, whose only required method shows
the document each frame:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui_host::native::run(CounterProgram {
        document: build(|| view! { <Root title="Counter"><Counter /></Root> }),
    })
}

struct CounterProgram { document: Document }

impl Program for CounterProgram {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
```

beui-demo is the full example (`./scripts/buck run //crates/beui-demo:native`),
and crates/beui-web-demo the same in a browser (`:web`, served on
http://127.0.0.1:8070).

### Root and the window's appearance

`build` takes a closure returning a `Root`, which is not a node: a document has
exactly one at the top, holding one child, and a node in its place does not
compile. Its props (`clear_color`, `title`, `system_bars`) are reactive and
become the window's `Appearance`: the runner clears to the colour wherever
nothing paints, so a program needs no frame filling the window; winit and the
web show the title (the web also colours the page body); Android colours its
bars from `system_bars`. `styled::Root` takes a `ColorRole` (`Background` by
default) and picks `system_bars` from the theme.

Holes are cut from the clear colour, not left by it: an `Embed` of a child
editor punches its rectangle. Documents drawn over another (the inspector, the
software keyboard) are made with `Document::new`, `enter` and `set_root`, have
no `Root`, and leave the window's appearance alone.

### Features

beui-host's features pick how much is linked: none gives the document,
components, layout, input and paint output (enough for headless logic tests);
`render` adds the wgpu renderer without a window (embedded hosts); `window`
(the default) adds the native runner (winit on the desktop, beui's own on
Android); `web` adds the browser runner, `beui_host::native::run_web`.

### RunOptions and the rest of Program

`run` uses default `RunOptions`; `run_with` takes them: the program id and
starting size, `device_extensions` for wgpu's Vulkan device (be-host imports
Wayland clients' buffers with them), and `platforms` the program offers beside
the native one (such as `beui_platform_drm::Drm`) for `BEUI_PLATFORM` to pick
by name (`beui_runner::environment::choose`). `BEUI_RENDERER` and
`BEUI_LAYERS` pick the renderer and the layer stack the same way; the stacking
is `choose_platform` and `stack` in crates/beui-host/src/runner.rs.

`Program`'s other methods are optional. The ones with traps:

- `setup(&Setup)` runs once after the gpu exists. `setup.get::<GpuSetup>()` is
  the wgpu device, queue and format for a program that paints with the gpu
  itself (`draw_gpu`). `Setup`'s `Waker` can be woken from any thread and is how
  work finishing elsewhere asks for a frame; beui never polls.
- `close_requested` may refuse a close (to ask about unsaved work, then call
  `Context::close_window`).
- `exiting` runs once, while the GPU device and displays still exist; the
  program is dropped right after. Anything kept in a `thread_local!` has to be
  taken out here: the main thread's thread-locals are dropped after `main`
  returns, after the GPU is gone, and dropping GPU or reactive state there
  aborts the process. be-host's `Shell::exiting` takes down the compositor,
  app runtimes and displays this way.
- `lifecycle` hears `Background` (occluded, Android `onPause`, a hidden tab),
  `Foreground`, `Focused` and `NetworkChanged`. The process may be killed
  without another frame after `Background`, so that is where unsaved work is
  put somewhere durable, without blocking.
- `busy`, `drawing` and `environment` are read only by a driver (below).

From inside a frame the program asks the window for things through `Context`:
`set_fullscreen`, `set_ime_area` for an input it draws itself,
`set_zoom_factor`, `close_window`, and `with_input`, which shows a document
only the input meant for it (how the inspector keeps the program's keys while
its own panel has the keyboard; guides/beui_keyboard.md, One input path).

The runner hands the document its events in one batch per frame. A press that
follows typing waits for the next frame, so text typed before a click reaches
the field it was typed into.

## Layers

A `beui_runner::Layer` is middleware between a platform and the program:
drive, space (guides/beui/space.md), xr and be-host's own. `Launch::layers` is
the stack, the layer nearest the platform first. Input goes up the stack from
the platform to the program; the program's output and its calls on the
platform go down. The hooks, in frame order (each documented on the trait):

- `pace` blocks for a paced display (a headset) and gives the frame's time;
  `now` may change the time (`Context::stop_clock` and `advance_clock` stop and
  step the clock everyone then reads).
- `input` sees everything that arrived for the frame, bottom first, and may
  rewrite, drop or add entries, name each pointer event's window, and push
  `WindowReport`s. `keyboard_attached` then overrides whether a keyboard is
  attached.
- `output`, top first, gets a `Shown`: what the layers above left of the
  program's windows and requests. What the bottom layer leaves is what the
  platform shows (`Runner::shown_window`).
- `framed` may ask for the next frame sooner; `wants_sight`, `wants_capture`
  and `captured` get pixels back from the renderer.
- `up` and `down` map points between the windows below and above the layer;
  `LayerFrame::below` and `above` map through the rest of the stack to the
  platform and the program.
- The services (`copy`, `paste`, `pick_file`, `show_window`, `resize`,
  `publish_accessibility`, ...) are handed the `PlatformServices` below and
  forward to it by default, so a layer may answer a request itself or change it.

`LayerFrame` also carries the `LayerView` (platform window size, scale, time),
the `Context`, the `Program` (for `busy`, `drawing` and the rest) and `close()`.

A layer that opens the program's windows sets `WindowHosting::Any` in `attach`.
The main window's size is then the one the layer reports, the program gets no
safe area, and a wgpu layer draws each window into a texture and composites
them through a `Drawing` (`beui::native::drawing`). beui-platform-drm under
such a layer shows the main window on its first display.

## Drivers

A driver is the layer `beui_layer_drive::Drive`; guides/running/drive.md covers
its commands, taps and sessions. The native run functions stack one where
`BEUI_LAYERS` names `drive` and serve it on `BEUI_DRIVE_SOCKET`;
`environment::drive_taps` works out a tap under each other layer
(`drive,space,drive`). be-host's web build stacks one its `automate` export
feeds. What a driver relies on from beui:

- `Context::keep_accessibility`, on by default for a bare `Context` and a
  headless app, and turned on by other runners only for a driver.
- Its input commands become `Incoming`s already in points, skipping the
  platform's intake (`beui_core::input::typing` names the keys); a tap under
  another layer sends physical pixels, as a platform does.
- It steps a stopped clock in `now` and answers in `framed` once a frame asks
  for no other and `Program::busy` is false.
- It acts on the window only through the `PlatformServices` below its lowest
  tap: `resize` works only on the headless window.

A shot of an occluded window waits for `Program::drawing` to say nothing is
left to draw (guides/beui/platforms.md, Unseen windows).

## The inspector and the simulation tools

Ctrl+Shift+I in a standalone window opens the inspector (crates/beui-inspector).
Its document is a `Docking` (guides/beui/docking.md) whose tabs are the tools
(Beui, Comp, A11y, Perf, Sim) and the inspected program, an `Embed` the
program's document is laid out in, so tools can be docked beside, under or
floating, and on a narrow window the dock stacks
([decision](../../decisions/beui_inspector_tools_are_dock_tabs.md)). The tree
tools pin a Pick toggle and every tool pins Close to the dock's bar.

- **Drawing order.** `InspectorTools` shows the inspector first, so the
  Program tab's rectangle is this frame's, sets its layers aside
  (`Context::set_aside`), shows the program in that rectangle, then puts the
  inspector's layers back on top (`put_back`). The Program tab is therefore
  `see_through`, and the pointer is kept from the program while over one of
  the inspector's overlays.
- **Whose inspector.** The chords (Ctrl+Shift+I, C, M, F) belong to the
  innermost document with the keyboard; while focus is on a node that forwards
  keys (be-host's app region) they go to that app. A document with no main
  window (`Context::shows_main` false: an app, the DRM session) gets an
  inspector with no Program tab, drawn over the right edge of the window that
  has the keyboard.
- **Comp** lists the `#[component]`s that built the tree: each records its name
  against the node it returns, so components returning the same node nest.
- **Keys.** Ctrl+Shift+C picks a node; Ctrl+Shift+F moves the keyboard into the
  panel and back, Escape returns it.

The Sim tab's tools work inside `Document::show`, so they behave the same in a
standalone window and in an app:

- **Emulate touch with mouse** turns mouse presses into touches
  (`Context::touch_emulation`). **Simulate mouse with touch** turns the shown
  rectangle into a trackpad with a button strip, trimmed off the bottom of the
  layout rectangle.
- **Responsive design mode** (Ctrl+Shift+M) lays the document out in a
  simulated screen of a chosen size and zoom, with `Context::scaled` and
  `clipped`. Its state lives in the `Context`, so it stays after the inspector
  closes. Every mapping is a pure scale, which a program that places surfaces
  outside `Document::show` relies on through `Context::screen_scale` (be-host
  does, for app surfaces).
- **Back gesture** plays Android's back gesture through `Document::back`, as
  the Android runner does, so a `BackSlide` can be tried without a phone.
- **Filters** (blur, contrast, colour vision) are `Context::apply_filter`: a
  `Group::Filter` around everything painted since the innermost `clipped` or
  `scaled`, drawn by the wgpu renderer (crates/beui-renderer-wgpu/src/filter.rs)
  into a scene texture and composited, so anything the document paints, an
  app's pane included, is filtered. A blur grows the damage by its reach
  (`Prepared::widen`); a filter that changes damages only its region. The
  inspector applies it after the screen reader's outline and before its
  readout, so the outline blurs and the words stay readable.
- **Simulate a screen reader** walks the AccessKit tree the document publishes
  and acts only through `Context::accessibility_action`, as a platform reader
  does; a control announced as a bare role has no name. While it is on the
  document gets no pointer input directly. Its commands hold Alt (walk, jump,
  activate, adjust, scroll, repeat), touch has the usual flicks and double tap,
  and the same commands are buttons in the panel. The readout is a fixed strip
  at the bottom ([decision](../../decisions/beui_screen_reader_readout_is_a_fixed_strip.md)).

## beui in an app

A beui editor implements `be_sdk_beui::BeuiRoot`, whose `View` (and the other
views the host asks for) are trait components, and registers with
`be_sdk_beui::beui_app!(Root, "../manifest.json")`. The type holds no state:
the framework builds the view once and keeps it. crates/be-app-example-counter
is the reference; guides/adding_an_app.md covers the manifest, creation, the
host connection and asking the host.

An app module is one beui program, with one `Document` and one `Context`:
every region the host shows of every instance (a frame, a preview, artifact
settings) is one of its windows, opened by be-sdk-beui from
be-sdk-beui-platform-api's `Views` handle, so an editor only writes
`View(editor)`. The app depends on beui alone; the host supplies input, fonts,
clipboard, rendering and the frame rectangle.

`View` returns the editor's own `Root`, whose `clear_color` the surface is
cleared to; the protocol carries no title. A preview, drawn over something of
the host's, is rooted in a transparent `Root`; an app dialog in `SurfaceRaised`.

`Editor` carries everything the instance was given:

- Block content arrives through a `ContentProjection`
  (`editor.block_content::<C>()`, then `field`, `project`, `operate`). The
  framework pumps every projection once at the top of the frame, and reads
  track, so memos rerun when content changes (guides/reactive.md, Blocks).
- A request the host answers is an `Asked` future; `editor.then(asked, f)` runs
  `f` with the answer unless the asking scope has gone.
- What the host sends unasked (drags, file drops, audio, history, focus,
  placement, peers) arrives at the top of the next frame as signals on
  `Editor`.
- Work on another thread takes its `Waker` from `editor.woken()`, whose signal
  ticks once the work has woken the editor.

A template the manifest marks `"dialog": true` shows `BeuiRoot::Creation` in
the picker's creation dialog, and `BeuiRoot::create_block` makes the block.

An app's `Window` (a dock's floating window) is an overlay inside its region's
window, except where the host shows windows as its own (the screen request's
`windows`; guides/adding_an_app.md): then each becomes a host-placed view.
be-host keeps those in `compositor/desktop.rs` (`Floated`): the desktop shell
draws them on the displays, and under space each is a `Panel` holding an
`AppView` (`PanelWindows` in `compositor/app_view.rs`). be-sdk-beui places the
children laid out in a window on that window's region.
