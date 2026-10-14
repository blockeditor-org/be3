# beui apps, runners and the inspector

How to run a beui document: as a standalone app, as a block editor plugin, and what the runners,
adapters and renderers underneath do. The last part covers the inspector and its simulations
(touch, mouse, responsive design, back gesture, filters, screen reader). Read
`guides/beui/overview.md` first.

## A standalone app

A standalone app builds its document once and implements `beui::App`:

```rust
use beui::reactive::{
    Frame, List, build, component, create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, DocumentTheme, Text, TextStyle, use_theme};
use beui::{App, Color32, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("Counter", CounterApp::new())
}

#[component]
fn Counter() -> NodeId {
    let (count, set_count) = create_signal(0i64);
    let label = create_memo(move || count.get().to_string());
    let theme = use_theme();

    view! {
        <Frame color={theme.background}>
            <List spacing=8.0>
                <Text style=TextStyle::DISPLAY string={label} />
                <Button
                    label="Decrease"
                    variant=ButtonVariant::Secondary
                    on_click={move || set_count.update(|value| *value -= 1)}
                />
                <Button
                    label="Increase"
                    variant=ButtonVariant::Primary
                    on_click={move || set_count.update(|value| *value += 1)}
                />
            </List>
        </Frame>
    }
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            document: build(|| view! { <Counter /> }),
        }
    }
}

impl App for CounterApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}
```

The demo runs with `./scripts/buck run //crates/beui-demo:native`.

### Run functions

| Function | Does |
| --- | --- |
| `beui::run(title, app)` | loads wgpu and runs in a window |
| `beui::run_with(options, app)` | the same with `RunOptions` (title, app id, starting size) |
| `beui::run_with_renderers` | takes the `beui::WindowRenderer`s to load instead (block-app's opens its device itself, to import Wayland clients' buffers) |
| `beui::run_on` | runs on an adapter of the caller's choosing, such as `beui_adapter_drm::Drm` (drives displays and input devices itself) or `beui::Headless` (draws offscreen, no window or display server) |
| `beui::run_web(element_id, renderers, options, app)` | the browser runner (see "Features") |

### The `App` trait

`update` is the only required method. The rest are optional:

- `clear_color`: the colour behind the document.
- `setup(&Setup)` runs once, after the gpu exists and before the first frame. `Setup` hands over a
  `Waker` and whatever the renderer and runner provide: `setup.get::<beui::GpuSetup>()` is the wgpu
  device, queue and surface format (for an app that paints with the gpu through `draw_gpu`), and on
  the desktop `setup.get::<Arc<beui::winit::window::Window>>()` is the window. `Waker::wake` can be
  called from any thread and asks the runner for another frame: it is how work finishing elsewhere
  is pushed to the ui instead of polled for.
- `close_requested` is asked when the window is closed and can refuse by returning `false` (to ask
  about unsaved work first, then call `Context::close_window`).
- `exiting` runs once on the way out; the runner drops the app right after it, while the GPU device
  and the adapter's displays still exist, and no frame runs after it. Whatever the app keeps in a
  `thread_local!` must be taken out in `exiting`: the main thread's thread-locals are dropped only
  by `exit()` after `main` returns, in no set order, after the GPU and runner are gone, and dropping
  GPU or reactive state there aborts or crashes the process. block-app's `Shell::exiting` takes down
  the Wayland compositor, the plugin runtimes and their worker threads, and the screens this way.
- `busy` says whether work the app is waiting on (a server request, a plugin's frame) will still
  change what it shows. Only a driver reads it.

From inside a frame the app asks the window for things through the `Context`: `set_fullscreen`,
`set_ime_area` for an input it draws itself, `set_zoom_factor`, `close_window`, and `with_input` to
show a document only the input meant for it (how the inspector keeps the app's keys while its own
panel has the keyboard; `guides/beui/input.md`, "One input path").

A runner hands the document its events in batches, one per frame. A press that follows typing waits
for the next frame, so text typed before a click always reaches the field it was typed into,
however many events arrive between two frames.

### Features

| `beui` feature | Adds |
| --- | --- |
| none | the document, components, layout, input model and painting output; enough for headless logic tests |
| `render` | the wgpu renderer without a window; embedded hosts (plugins) use this level |
| `window` (default) | the native runner, and `render` |
| `web` | the browser runner and `WebRenderer::Wgpu`, which draws into the element if it is a canvas and into a canvas it adds otherwise; enables `render` |
| `dom` | the browser runner and `WebRenderer::Dom { icons_font }`, which draws with DOM elements instead of wgpu |

On the desktop the native runner is winit's. On Android it is beui's own
(`crates/beui-adapter-android`, with its Java in its `android` directory):

- The app's activity extends `com.be3.beui.BeuiActivity`, whose `BeuiView` is the surface, takes
  touches, keys and the soft keyboard's input connection, and hosts AccessKit. It has to be a view
  that input reaches through the view system: TalkBack's touch exploration arrives as hover events
  on it, which NativeActivity's input queue never delivers.
- The library defines `#[unsafe(no_mangle)] fn android_main(app: beui::AndroidApp)`, which beui
  calls on a thread of its own, and which calls `beui::run_with`.

The soft keyboard takes its room out of the window rather than covering it: on Android its inset
joins the safe area, and on the web the pages' viewport meta asks for
`interactive-widget=resizes-content`. When the rectangle a document is shown in changes size while
the focus takes text, the document scrolls the focused field into what is left, through every
scroll it sits in.

### The web runner

`beui::run_web` loads each `beui::WebRenderer` it is given into the element, shows the first that
loads, and skips (with a console warning) any that fail.

- The wgpu renderer draws with WebGPU, or WebGL where the browser has none.
- The keyboard is read through a hidden text area, so pasting and composition behave like any other
  input on the page.
- Like the desktop runner, it asks the browser for a frame only when an event arrived, something
  asked for a repaint, or a `Waker` was woken.
- The DOM renderer keeps one element per `Display` in the frame's display tree across frames (by
  `Display::key`), holding its shapes and its children's elements placed at their translations, and
  each group in it (a clip, a scroll, a mask, a scale) is an element of its own. A display whose
  `Rc` is unchanged is not looked at, so scrolling moves the rows' elements and rewrites nothing
  inside them. Layout, input and focus are beui's as everywhere else. Text is measured and drawn
  with the browser's fonts (`beui-font-browser`), and the browser fetches the icon font from the URL
  `icons_font` names. A `Drawing`, a `Punch` and a `Filter` draw nothing there.
- `crates/beui-web-demo` is the demo with both renderers, DOM first:
  `./scripts/buck run //crates/beui-web-demo:web` serves it on http://127.0.0.1:8070.

### Drivers and headless windows

`RunOptions::automation` hands the runner an `automation::Inbox` of commands from a driver
(`guides/running/drive.md`). The native run functions fill it from `BEUI_AUTOMATION` and switch to
`beui::Headless` for `BEUI_HEADLESS`; block-app's web build fills it from its `automate` export.

- The runner keeps the accessibility tree (`Context::keep_accessibility`, which a bare `Context`
  and a headless plugin do by default and other runners only for a driver) and the action
  registry's actions.
- It takes the next command at the start of a frame, turns input commands into `Event`s already in
  points, which skip the adapters' intake (`automation/keys.rs` names the keys), and answers once a
  frame asks for no other and the app is not `busy`.
- The driver acts on the window only through the `Platform` it is given, as the app does:
  `clipboard` copies and pastes through it, `resize` asks `Platform::resize` (which only a headless
  window answers), and `state` reads the size, scale and fullscreen the window last reported.
- While a driver is attached the runner hands it the app's file dialogs instead of the platform, so
  `upload` and `dismiss` answer them in any window, and it keeps the focus `focus` and `blur` gave.
- The headless adapter's window is `automation::Simulation`, a `Platform` like any other: it keeps a
  clipboard, grows to its screen for fullscreen, and reports its size, scale and fullscreen through
  its `WindowReports`. beui's own tests drive one the same way, with a renderer that draws nothing.
- `Renderer::capture` reads the presented frame back for a screenshot.

A window nobody can see is `Runner::set_occluded`: its frames still run (layout, paint,
accessibility, the driver), but the runner hands each one's output to `Renderer::note` instead of
`prepare` and presents nothing. `note` keeps only what the frame damaged, so the wgpu `Target`
draws the union of every unseen frame's damage into its retained frame next time it draws; a
renderer without `note` prepares as usual. The headless adapter is always occluded, so it draws
only for a driver's `shot`: that frame prepares, presents and is read back.

## A block editor plugin

A beui editor is a `#[component]` function. Its app type implements `block_editor_beui::BeuiApp`
and is registered with `block_editor_beui::beui_plugin!`. The type holds no state: the framework
builds the view once, keeps the `Document`, and shows it every frame.

```rust
#[component]
pub fn Counter(editor: block_editor_beui::Editor) -> NodeId {
    let counter = editor.block_content::<CounterContent>();
    let count = counter.field(ObjectId::ROOT, CounterModel::COUNT);
    let increment = move || counter.operate(CounterModel::add(1));
    view! { ... }
}

pub struct CounterApp;

impl block_editor_beui::BeuiApp for CounterApp {
    fn view(editor: block_editor_beui::Editor) -> NodeId {
        view! {
            <Counter editor={editor} />
        }
    }

    fn create_block(creation: &block_editor_beui::Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CounterContent::default()))
    }
}

block_editor_beui::beui_plugin!(CounterApp, "../manifest.json");
```

The host supplies input, fonts, clipboard, rendering and the frame rectangle, so a plugin depends
on beui without the window runner:

```toml
beui = { path = "../../beui", default-features = false, features = ["render"] }
```

`crates/editors/counter` is the reference integration. `guides/plugins/adding_an_editor.md` covers
the package, manifest and creation flow step by step, and `guides/plugins/editor_api.md` everything
an `Editor` offers. In brief:

- `Editor` is everything the instance was given: the host, the block, the `Blocks` handle to the
  graph, the view the host shows the content through, and signals for everything the host tells
  it.
- Block content reaches the view through a `ContentProjection`: `editor.block_content::<C>()`
  holds the editor's own block, `project`, `field`, `ids` and `object` derive signals from it, and
  `operate` applies an edit and sends it to the host. The framework pumps every projection once at
  the top of the frame, inside the document's reactive scope, and `read` and `revision` track, so a
  memo or effect reading them reruns when the content changes (`guides/beui/reactive.md`).
- Nothing in an editor polls. What the host sends between frames is delivered at the top of the
  next frame as signals on the `Editor`: `replies()` ticks when the host answers a request (a file
  or block pick, a fetch) and `on_reply(f)` runs `f` each time; `drag()` and `files()` carry a drag
  or file drop over the editor; `audio()`, `history(block)`, `histories()`, `artifacts()`,
  `focused_block()`, `web_view_events()` and `peers::<P>()` follow the host's state;
  `block_types()` tracks the catalog; `placed()` is the content rect.
- Work on another thread takes its `Waker` from `editor.woken()`, whose signal ticks once the work
  has woken the editor; an effect reading that signal collects the result.
- A template the manifest marks `"dialog": true` is made through `creation_view` instead: another
  `#[component]` the framework builds a separate document of and shows in the picker's creation
  dialog. It says what the dialog makes with `creation.on_create(...)` and calls
  `creation.set_ready(true)` once filled in. Host services such as `BlockPicker` work there too,
  collected from `creation.on_reply(...)`.

## Runners, adapters and renderers

Read this when changing a platform adapter, a renderer, or how the window and the app talk.

Each platform is a `beui_core::runner::Adapter`, a trait object the facade picks
(`beui::window_adapter`, `beui::web_adapter`) and runs with a `Launch` (`RunOptions`, `Context`,
the `App`). The adapter owns the event loop and turns the platform's input into `Event`s.
Everything one frame does is `beui_core::runner::Runner::frame`, shared by all adapters: it runs
the app, publishes accessibility, and tells the adapter's `Platform` (another trait object) what
the app asked of the window. A new platform implements `Adapter` and `Platform` and hands its
window or element to the renderer loader the facade gives it. `Platform`'s methods all have
defaults (no clipboard, no file picker), so a platform implements only what it has.

### Window state: app to window

Everything the window shows for the app is one `beui_core::window::WindowState`: the cursor,
pointer lock, back handling, wanting the keyboard, intercepted keys, the paste kinds the focus
takes, the IME area, and `typed`. The `Context` fills it during a frame and `FrameOutput::window`
carries it.

- The runner hands it to `Platform::show_window` with the state it showed before, only when they
  differ, so an adapter compares just the fields it uses. A new field is one more field there and
  in the adapters that act on it. `Runner::window` is the state last shown.
- The IME area is in the window's own units (winit's logical pixels, CSS pixels, a plugin region's
  points: physical pixels over the platform's scale, so points times the zoom).
- `typed` counts the frames that delivered IME or text input. Typing moves the platform's own copy
  of the field (the web's hidden textarea, Android's editable) even when the app turns the text
  away or a composition ends with the field unchanged, so the change in `typed` is when the web and
  Android adapters bring that copy back to the app's text, comparing against what they last sent.
- While `Context::touch_emulation` is on the cursor is `CursorIcon::Touch` whatever the app asked:
  a finger-sized circle defined once in `beui_core::touch_cursor` as an image at any scale (winit's
  custom cursor, the DRM sprite), an SVG (the web's CSS `url(...)` cursor) and a painter call (the
  mouse simulation's cursor). A plugin region passes it to its host as the protocol's
  `CursorIcon::Touch`; a platform with no custom cursors falls back to `CursorIcon::Touch.name()`,
  a crosshair.
- What happens once rather than holds (a copy, a paste, a file pick, `set_fullscreen`, closing) has
  a call or field of its own.

### Window reports: window to app

What the window did goes through `Runner::reports()`, a `WindowReports` handle an adapter may clone
and keep (the web adapter hands one to its `fullscreenchange` listener):

- `resized` with its size in physical pixels;
- `scaled` with its pixels per point (winit's scale factor, the browser's device pixel ratio,
  Android's density, a plugin region's scale factor, the DRM displays' scale);
- `fullscreen`.

The runner applies them at the start of the next frame, resizing the renderers and setting
`Context::set_pixels_per_point` before the app lays out, so no adapter resizes a renderer or passes
a scale itself. A report made during a frame (a simulated window answering) makes that frame ask for
the next.

Fullscreen goes both ways: `Context::set_fullscreen` asks the window to enter or leave it, the
runner passing on each request that differs from what the window last reported, and the window
reports what it actually did, whether asked or not, which the app reads as `Context::fullscreen`
and a component as `use_fullscreen()`. The DRM adapter is always fullscreen; Android and plugins
say nothing.

### Input intake

An adapter only translates. It hands `Runner::push` a `runner::Input`:

- an `Event` with positions and deltas in physical pixels;
- a `Keystroke` (the key, its text and scancode, with no modifiers);
- a `ScrollDelta` in pixels, lines (`SCROLL_LINE` points) or pages (`SCROLL_PAGE`);
- a `Pinch` (a Ctrl-wheel, as a `ScrollDelta`), which becomes `Event::Zoom` at `PINCH_SPEED` per
  point.

`Input::logical` wraps any of them whose pixels are the window's logical ones (physical pixels over
the scale it last reported: the browser's CSS pixels), so the runner converts them with the scale
the frame lays out at and no adapter reads its scale at event time. Text is passed as the platform
gives it; the runner drops control characters. An adapter may send `Event::Modifiers` on every
event; the runner passes it on only when it changes.

At the start of each frame the runner turns its input into points. It stamps keys, pointer buttons
and intercepted keys with the modifiers held, and drops a held modifier's repeats and the text of a
shortcut. It turns the primary mouse button into a finger while `Context::touch_emulation` is on,
holds `PointerGone` back while a button is held, and lets go of the modifiers, buttons and emulated
touch when focus is lost.

### Clipboard and paste

The clipboard is the platform's. `Context::copy_text` writes it, `Context::watch_copies` hears
every copy (how be-wayland hands the app's copies to Wayland programs), and a paste is the only way
to read it.

- The focus says what it takes: an `Interactive`'s `pastes` lists the `PasteKind`s (text, image) in
  preference order, and one with only `on_text` takes text.
- `request_paste` (a Paste menu) and Ctrl+V, which the document turns into a paste when nothing on
  the key's way took it or the focus forwards it, ask `Platform::paste` once a frame for the first
  of those kinds the clipboard holds. The answer arrives at the focus as `Event::Paste`
  (`Pasted::Text`, or `Pasted::Image` as a PNG `PickedFile`), handed to `on_paste`, or a text paste
  to `on_text` without one.
- A platform that delivers Ctrl+V's paste itself (`Platform::delivers_paste_shortcut`: the browser,
  from its paste event's data and the window's `paste_kinds`, and a plugin's host) is not asked
  again for the shortcut.
- The winit adapter reads images too; Android and DRM read text only.

### File picks, accessibility and frame timing

- `Platform::pick_file` gets a `FilePickAnswer`, which can be sent from any thread and wakes the
  runner; the pick reaches the app at the next frame (`guides/beui/components.md`, "Picking a
  file").
- Accessibility trees are published only between `Runner::accessibility_requested` and
  `accessibility_deactivated`, and `Context::accessibility_requested` says whether they are, so a
  host asks what it embeds to describe itself only while something is listening.
- `Frame::wait` is how long the adapter may sleep before the next frame (`None` until something
  wakes it), and `Frame::next` the same once it knows whether the frame it presented asks to be
  presented again.

### Closing and panics

Closing is the runner's. When the app calls `close_window` (or a driver's close is accepted) the
runner exits after that frame, and `frame` returns `None` with `Runner::exited` set, which every
adapter answers by stopping its loop. Android also finishes its activity, the web adapter closes the
tab where the browser lets a script, and a plugin region's `Frame` says `stopped` from then on
(`Stopped::Closed` or `Stopped::Failed`), which the host shows in the region's place.

An adapter whose window is closed from outside calls `Runner::os_close`, which asks the app's
`close_requested` and exits if it agrees. The web adapter asks on `beforeunload`, and a refusal is
the browser's leave-the-page prompt.

A panic out of the app's `update` stops the runner rather than unwinding through the event loop:
the app hears `exiting`, `frame` returns `None` from then on, `Runner::failure` says why, and the
adapter ends its loop and returns that as an error (`Runner::finish`), so whatever owns the screen
is let go of in order. An app that can carry on after a panic in one part catches it there itself.

### Renderers

Every adapter draws through `beui_core::renderer::Renderer` trait objects and names no renderer
crate: wgpu's `WindowSurface` (native windows), `CanvasSurface` (a browser canvas) and
`DomRenderer` implement it, and the facade hands the runner the ones to load.

- A renderer is told where to draw with `attach` (a raw-window-handle `WindowHandle`) or is made
  with its element, and draws a frame in two steps, `prepare` and then `present`.
- `beui_core::renderer::Renderers` holds what a runner loaded, each with the `FontBackend` its text
  is measured with if it needs its own, and shows one.
- `Context::choose_renderer` (offered by the inspector's Perf tab when more than one is loaded)
  hides and detaches the shown one, attaches or resizes the chosen one, and swaps the context's
  fonts with `Context::replace_fonts`, which lays all text out again.
- `Renderer::screens` reports the monitors (`guides/beui/components.md`, "Screens");
  `Renderer::shift` copies a scrolled region (`guides/beui/writing_components.md`, "Painting and
  damage").

### DRM

`beui-adapter-drm` drives Linux displays and input devices through DRM/KMS and libinput: an
`Adapter`, `Platform` and `Renderer` whose screens each keep a retained frame and are drawn when
their display flips. The app's frames are paced to the flips too, by `pacing.rs`: at most one per
flip, none while nothing changes. It hands the flip a fence (or, without atomic fences, waits for it
in the event loop) instead of blocking on the GPU. Callers run it with `beui::run_on`. See
`guides/linux_desktop.md` for the desktop built on it.

## The inspector

Ctrl+Shift+I in a standalone beui window opens the node, component, accessibility and performance
inspector (Perf tab: `guides/performance.md`).

| Key | Does |
| --- | --- |
| Ctrl+Shift+I | open or close the inspector |
| Ctrl+Shift+C | enable node picking |
| Ctrl+Shift+F | move keyboard focus into the panel and back out |
| Escape (in the panel) | return focus to the document |
| Ctrl+Shift+M | responsive design mode |

- The Comp tab lists the `#[component]`s that built the tree rather than its base nodes: every
  component records its name against the node it returns, so components returning the same node
  nest there, outermost first.
- Tree rows select and expand together: clicking a row, or Enter or Space on it, selects the node
  and opens or closes its children; the arrow keys walk the tree.

The Sim tab holds the simulations below. They run inside `Document::show`, so they work the same in
a standalone window and in a beui block editor plugin. It also switches the document's theme.

### Touch and mouse simulation

"Emulate touch with mouse" turns mouse presses into touch events.

"Simulate mouse with touch" turns the whole shown rectangle into a trackpad and paints a cursor the
document reacts to, in the shape of the frame's `CursorIcon`.

- One finger moves the cursor; two fingers scroll smoothly.
- A tap presses the primary button where it lands and holds it until the double-tap timer runs out,
  making it a click. A press within that time keeps the button down: moving drags from where the tap
  landed, and lifting in place locks the button, so the drag carries on across touches until a
  single tap releases it. A stroke that moved the cursor is never a tap.
- The strip along the bottom holds the left, middle and right buttons and a keyboard toggle. A
  button stays held while its finger is down, another finger can work the trackpad meanwhile, and
  swiping up or down on the middle button scrolls a wheel tick at a time.
- The keyboard toggle opens an on-screen keyboard that sends `Event::Key`s with the text they type.
  Its Shift, Ctrl and Alt keys latch until the next key and also apply to clicks, so Ctrl+Shift+I
  on it reopens the inspector.

The strip and keyboard take their room out of the window rather than covering it:
`Document::show` asks the simulation how tall it is, trims that off the bottom, and lays the
document and inspector panel out in what is left. Nothing is drawn over live content, so the bars
are opaque.

### Responsive design mode

The Sim tab's "Responsive design mode", or Ctrl+Shift+M, lays the document out in a screen of a
chosen width and height in points, like a browser's responsive design mode.

- A toolbar above the app picks a device preset, types the width and height, rotates the screen and
  sets its zoom: Fit shrinks it to the room beside the inspector but never grows it, and a
  percentage draws it that many real points per simulated point.
- The screen sits centred at the top of a backdrop, with handles on its right edge, bottom edge and
  corner that resize it. A drag holds the scale it started at, so the handle stays under the
  pointer, and the screen refits on release.
- A screen drawn larger than its room follows the pointer: the point under the pointer is always the
  point at the same fraction of the simulated screen, so moving across the room looks around the
  whole screen.
- The toolbar is a document of its own spanning the toolbar and the room below, so its selects can
  open over the app, and the app gets no pointer while one is open.
- The state lives in the `Context`, so the screen stays simulated after the inspector closes.

`Document::show` does it with `Context::scaled` and `Context::clipped`, laying the document out at
a rectangle chosen so a real point is always the document point times the scale; panning moves
where the document is laid out instead of adding an offset. Every mapping stays a pure scale, which
an app that reads input or places surfaces outside `Document::show` needs: `Context::screen_scale`
and `Context::screen_input` give it the scale and the frame's input in document points, as
block-app's host reads them for its plugin surfaces.

### Back gesture

The Sim tab's Back gesture slider plays Android's back gesture into the document, so a `BackSlide`
can be tried without a phone: dragging starts the gesture and follows it, reaching the end goes
back, and letting go before then cancels. It calls `Document::back` directly, as the Android
runner's events do, so it reaches the document even while the inspector holds the keyboard.

### Filters

The Sim tab's Filters section puts a blur, a contrast reduction and colour vision simulations over
the shown rectangle. They are independent of the screen reader simulation and of each other, and
need no cooperation from the document: they are a post-processing pass in `Renderer`, so whatever a
document paints (a plugin's beui pane included) is filtered the same way.

`Context::apply_filter` turns them on. It takes a `Filter` (the region in points, a blur radius, a
contrast multiplier and a `ColorVision`) and wraps everything painted so far, since the innermost
`Context::clipped` or `Context::scaled` it is called in, in a filter group (`Group::Filter`): what
was painted before the call goes through the filter, and what is painted after lands on top
untouched. A second call filters the first one's result again rather than replacing it.

- The wgpu renderer draws each filter group into a scene texture of its own, starting transparent,
  filters it and lays it over what lies beneath; a filter at the very bottom of the frame holds the
  background too, so it filters the whole picture.
- The inspector calls it once a frame, after the document, panel, overlays and the screen reader's
  focus outline, and before the reader's readout, so the outline blurs with the page while the words
  stay readable. The region is the shown rectangle, so the panel holding the sliders is never
  filtered.
- The blur is a dual Kawase chain: the frame is halved a level at a time, then tented back up, with
  the number of levels taken from the radius. A radius of 120 points costs no more than 8. The
  radius reads like CSS `blur()`: light from an edge reaches about three times it. Sampling is
  clamped to the filter's region at every level, so nothing outside bleeds in.
- Contrast and the colour vision matrices run in the pass that composites the blur. The matrices
  are the Viénot, Brettel and Mollon linear-RGB approximations, applied in linear light; the
  contrast reduction pulls toward mid grey in gamma space, as CSS `contrast()` does.

A filter keeps the frame's damage rectangle, so a filtered frame costs no more to repaint:

- Contrast and colour vision are per-pixel and need nothing beyond the changed region. A blur
  spreads light, so `Prepared::widen` grows the damage by the chain's reach (the sum of what every
  pass can move a sample), clipped to the filter's region, once per filter.
- Scene textures and blur chains are retained like the frame, and every pass is scissored to the
  widened rectangle: what lies outside was left correct by the previous frame, because the reach
  bounds what a change can touch at every level. Each damaged rectangle goes through the whole chain
  as a pass of its own.
- A filter that changed (a new radius, switched on or off) damages only the region it covers: the
  `Context` reports the regions of filters that differ from the previous frame, and the renderer
  adds those it prepared differently.
- A scroll is copied unless what it moves overlaps a filter's region.

### Screen reader simulation

"Simulate a screen reader" at the bottom of the Sim tab hands the document over to what a screen
reader would say.

- A readout takes a fixed strip off the bottom of the shown rectangle (above the mouse simulation's
  bar when both are on), holding the last thing said and how far through the document the reader
  is. It is a strip the document does not get, not a sheet over it, so its height stays fixed (a bar
  that grew with the text would relayout the page on every utterance) and a long phrase is clipped
  rather than wrapped.
- The highlight marking the reader's position paints over the document under the filters, so
  turning up blur or contrast reduction leaves the page's shape with nothing to read; the readout is
  outside the filtered region and stays legible.

It reads nothing of the beui tree. It walks the AccessKit tree the document publishes every frame,
in reading order, and changes the document only by sending `ActionRequest`s back through
`Context::accessibility_action`, the path a platform screen reader uses.

- An item is anything with a name of its own, anything that answers Click, Focus, Increment or
  `SetValue`, and any scroll area. A control's own text is folded into its name rather than read
  separately, as a platform computes one.
- It speaks the name, the role, the value when it differs from the name, then the state (checked,
  expanded, selected, a slider's percentage, "dimmed" for a disabled control). A control with
  nothing to name it is announced as its bare role: "button" alone is the bug.

While it is on, the document answers no pointer input directly, so the only way to reach anything
is the simulation. The keyboard is split as a platform reader splits it: the reader's commands all
hold Alt, and every other key goes to what the document has focused. Landing on a control focuses
it, so walking to a text field and typing types into it, and the arrows, Enter and Space do what
the focused control does. The reader follows focus too: when the document moves focus itself (Tab,
Shift+Tab, a dialog opening) the reader moves to the control that took it and reads it. Keyboard
and touch work at the same time.

| Keyboard | Touch | Does |
| --- | --- | --- |
| Alt+Left/Right (or Up/Down) | flick left or right | move an item at a time |
| Alt+Shift+arrows | | move between controls |
| Alt+Home, Alt+End | | jump to the ends |
| Alt+Enter, Alt+Space | double tap | activate |
| Alt+Minus, Alt+Plus | flick up or down | adjust a value |
| Alt+Page Up, Alt+Page Down | drag two fingers | scroll |
| Alt+R | two-finger tap | repeat the current item |
| | drag a finger | read what is under it |

The document never sees those keys. Walking past either end says so and reads the item again.
Turning "Emulate touch with mouse" on as well makes the gestures reachable from a mouse. The same
commands sit in the panel as buttons, listed in the Sim tab, so the simulation can be driven without
either device. Ctrl+Shift+F still parks keyboard focus in the panel and Escape returns it; the panel
does not otherwise hold focus while the simulation is on, so clicking a command button leaves the
keyboard commands working.
