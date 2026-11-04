# The app framework

How be-sdk and be-sdk-beui turn the app protocol into an editor's views, for
work on the framework rather than on one app. The protocol itself is described
in `crates/be-sdk-api/PROTOCOL.md`.

## The crates

- `crates/be-sdk`: the core, with no UI framework in it. It speaks the app
  protocol, keeps each instance's `EditorHost`, gives every screen the host
  asks for and every window an instance opens a surface of its own, and owns
  the wgpu device. Its state (content, graph, editor, artifact and creation
  state) is plain values with change callbacks
  (guides/blocks/apps.md, State and change callbacks), so any UI
  framework can adapt it.
- `crates/be-sdk-beui`: what every beui editor goes through. `BeuiApp<A>`
  implements be-sdk's `Instance` for a `BeuiRoot`, turns the callbacks into
  signals, and adds the frame's chrome, menu and children.
- `crates/be-sdk-beui-platform`: beui's platform for an app module (fonts,
  renderer, the runner).
- `crates/be-sdk-beui-platform-api`: what the two share. be-sdk-beui reaches
  the platform only through the symbol `__be3_beui_app_runtime`, so editing
  the platform rebuilds no editor library. Whatever runs an editor links it
  (`extern crate be_sdk_beui_platform as _;` in the beui library and
  be-sdk-test).

A framework other than beui implements `be_sdk::driver::Regions` over its
runner and registers a `Driver`, which keeps surfaces, damage, asks and host
services for it. An editor with no framework at all implements `App`,
`Instance` and `ViewHandler` against be-sdk and uses `be_sdk::app!`;
`crates/be-app-example-triangle` is the example and its tests drive the
handler directly.

## Views

The host shows an instance in one or more views (`be_sdk_api::View`: a
`ViewId` unique in the instance, and a `ViewPlacement` naming who places it):
its main view, a preview or artifact settings, a dock tab body, bar or dialog,
a further display of a desktop host, a window it opened, or an anchored view.
Only `ViewId::MAIN`, `PREVIEW` and `ARTIFACT_SETTINGS` have fixed meanings;
otherwise the placement says what a view is.

All instances of one module share a `be_sdk::Module` and one `ViewHandler`,
which the first instance to open a view registers. The core sends every view's
calls to it as `ShownView`s: input, `frame(&[ShownView])` (one call frames
every shown view, so a view just asked for is drawn in that call), and
`paint` into a `PaintTarget`. A surface's age says how many presents ago it
was last painted; at age 0 the core cleared it and the handler paints
everything, otherwise only what changed. `paint` returns the rectangles it
drew so the host redraws only those.

A `Frame` carries what changed, when to draw again, the reports the host is
told, `asks` about windows, and `WindowRequests` (cursor, pointer lock, IME
area, back, keyboard, intercepted keys, paste kinds, every-sample), which the
core reports only when they change.

## beui's side

`AppRuntime::launch` starts beui's runner for the module's one beui `App` and
registers a `Driver` over it as the module's handler (guides/blocks/apps.md,
The region driver), then returns; the host's
calls drive frames. The framework runs one beui `Context` and one `Document`
for the whole module, opens a beui `Window` per view at the view's size (so
it lays out at that size in the frame that shows it), and builds
`<A::View editor />` into the frame window once. Each window is painted by
beui's wgpu renderer straight into its view's surface, all sharing one set of
pipelines, atlases and images. When the app's layout stops listing a screen,
both sides drop that view's surface and textures at once, so an app showing
nothing holds no per-view GPU memory.

The app surface is not an sRGB format, so beui encodes gamma there and linear
where it draws its own window.

`Views` (`setup.get::<Views>()`) gives the platform each view's size, scale,
placement and `FrameSpec`, and takes its `Report`: content, painted and
floating rectangles, the overlays' dimming scrims and press claims. The app
never paints scrims (`Context::hosts_scrims`): the host draws them over the
view's children, so a command palette dims program windows instead of hiding
them. be-sdk-beui's `HeadlessApp` paints them itself.

A view knows nothing of monitors; an app that lays out per monitor (the
desktop's bars) reads `DisplaysWatch`. A panic anywhere in the app stops the
whole module.

## Windows

The platform shows no main window. It opens only the windows it placed,
unless a view says the host shows windows opened from it as its own
(`ShownView::windows`, set for the desktop shell). Then the module hosts every
window: a dock window or a window opened `AtPointer` is asked of the host
with an `Open` naming the view it came from, along with its moves, raises,
resizes and closes, in document order on the instance's first view's frame.

A window is drawn in the frame that opens it: the core gives it a screen
numbered by the app (`ScreenId::opened`), frames it with
`ViewHandler::frame_opened` from the same beui frame, and sends it in the
layout before the commit that opens it; be-host adopts the screen
(`Instances::adopt_window`) so the window shows in the frame that applies the
commit. Any other window is an overlay inside its view.

## Anchored views

A view may anchor views of its own, which the host draws above every view
(PROTOCOL.md, Anchored views): menus, popovers, tooltips, small dialogs and
edge sheets that escape the editor's frame. The core does the protocol's part
for any framework (`screens.rs`, `Screens::frame`): it gives each anchored
view a screen of its own, frames it with `frame_opened`, and reports, paints,
lays out and presents it in the same commits as its parent. It drops anchored
views whose parent reports no frame or anchors none.

In beui an overlay that escapes its window is a layer, and a window's layer
space is its view's anchor space (guides/beui/overlays.md, Layers that escape
their window). `crates/be-sdk-beui-platform/src/escaped.rs` turns each
escaped layer into one anchored view; input on it goes to beui as
`Incoming::layer`, and a press outside dismisses it through
`ViewHandler::dismiss_anchored`, which becomes `Context::dismiss_layer`. A
child placed inside an escaped layer is placed in that layer's anchored view.
The native host draws anchored views from the commit with no round trip
(`crates/be-host/src/compositor/anchored.rs`) and hit-tests them first; the
web host draws them in each worker's overlay canvas.

## Input

Pointer input arrives as `TimedInputs`, each sample stamped with when it
happened on the app's frame clock (`be_sdk::instant_at`). The host sends one
`InputBatch` per screen at a time and merges the next into one still queued,
so input never backs up. By default it keeps only the latest move of each
pointer between other events; a drawing tool sets `every_sample` (beui's
`every_sample` on an `Interactive`) to get every sample. A press held over
another of the module's views carries that view as `over`.

## Fonts

An app opens no font file: the host sends its fonts (`fonts-watch`) as soon as
the app connects, and be-sdk-beui-platform keeps one `FontLibrary` for every
context of the app (`be_sdk_beui::context()` for painting outside the views,
as the canvas's export does). A character no font covers is reported back and
a native host answers with a system font (fontconfig on Linux); the web host
has none to offer. Tests use only beui's bundled fonts
(`AppRuntime::use_bundled_fonts`), so paintings match on every machine.

## Where the pixels live

An app never learns where its pixels live. It is handed an opaque texture:
under wasmtime a texture in be-host's own device (nothing exported or
imported), in the browser one on the app worker's device, drawn into a canvas
the page keeps behind the host's canvas, showing through a transparent hole.
Its wgpu calls reach the device over the `be3:gpu` world
(guides/app_libraries.md, The gpu).
