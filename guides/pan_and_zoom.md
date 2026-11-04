# Pan and zoom

Pan and zoom is an app capability (`capabilities.pan_and_zoom` in the
manifest, guides/adding_an_app.md). An editor with it draws a world at a
position and scale, but the camera is the host's: the editor is handed it and
asks to move it, and never keeps an offset or zoom of its own
(decisions/pan_and_zoom_camera_belongs_to_the_host.md). So the same editor
works in a tab of its own and as a card on another editor's canvas without
knowing which; a zoom means the same whether it came from the editor's `+`
button or a wheel over the card. `crates/be-app-example-pan-zoom` is the
reference editor.

## What the editor is given and asks for

`editor.canvas()` and `editor.scale()` are signals the framework writes before
every frame (`beui::reactive::CanvasView`, a plain `{origin, scale}`, in the
editor's own coordinates). An editor never told its view fills its space at a
scale of one. It moves the camera by asking: `editor.pan(delta)`,
`editor.zoom(factor)`, `editor.zoom_at(factor, anchor)`, and `editor.reveal(rect)`
to centre a rectangle of its world.

It names the node its content is in once, `editor.content(&node_ref)`; the
framework reports that node's rectangle as insets after every layout. That is
how the host knows what rectangle to pan and zoom and read gestures over, so a
wheel over the editor's sidebar is not a wheel over its content.

## Fitting

What to fit is the editor's; keeping it fitted is the host's.
`editor.keep_fitted(|| Some(fit))`, called once, declares a `Fit`:
`Fit::World` (content laid out inside the world: image, map, games),
`Fit::Area(rect)` (a rectangle of the world: a PDF page), or `Fit::Centre`
(the world's centre at scale one: the infinite canvas's origin). It sends
`ViewChange::KeepFitted` only when that changes, and the host re-fits in the
same frame whenever the content band, the world or the chrome changes, with
nothing round-tripped through the editor (`FitArea::fit` in be-sdk-api, never
above scale one, shared by be-host's camera and be-sdk-test's). It returns a
`Fitting` whose `fit()` is the editor's Fit button. `editor.fit(world, room,
max_scale)` (`crates/be-sdk/src/editor_state/fitting.rs`) is a one-off move
instead, as the canvas's Fit all and the logic grid's recenter use.

The host's camera stops following the fit once moved by anything - a wheel, a
pinch, a pan-drag, or the editor's own `pan`, `zoom` and `fit` - until
`Fitting::fit()` sends `ViewChange::Fit`. `moved` is remembered per block while
be-host runs (not in the view block), so a block reopens where the person left
it, or fitted if they never moved it. It follows only while the editor has a
frame and its chrome drawn; placed on a canvas, `fit()` is a single
`editor.fit` through the camera it is placed in.

## Gestures

Wheel, pinch, and middle- or space-drag over the content are the host's, read
once for the outermost view under the pointer, so a wheel over a block being
edited inside a canvas moves the canvas, not both. beui hands them to the
outermost `Interactive` on the hit path whose `gesture_area` holds the point,
ahead of the app view's input forwarder, so the app never sees them; the
`Camera` claims its content rectangle less the floating chrome (dock bars) the
editor reports. `capabilities.pan` picks which: `TwoFinger` (default), `None`,
or `OneFinger`, which adds a plain drag. A trackpad's two-finger scroll and pinch
are that wheel and pinch on the web too (guides/beui/platforms.md, Input). Only the editor can tell that drag
from one on its own content, so the press goes to the editor, which calls
`editor.pan_drag()` when it pressed on nothing; the host then cancels the
pointer in the editor and pans until release. A finger on a rectangle the
editor reports as `drawing` while canvas touch is off pans nothing
(guides/canvas.md, device input).

A beui view with no host to hand it a camera (a standalone canvas, the demo's
stage) composes `beui::unstyled::PanZoom`, which reads the same gestures and
reports a camera its caller owns ([beui/components.md](beui/components.md), pan
and zoom). An editor on a host never keeps a `PanZoom` of its own.

## Who owns the camera

`BlockFrameView` (`crates/be-host/src/compositor/editors.rs`) shows one block's
editor. The camera is `Viewport` in the `Camera` component
(`crates/be-host/src/compositor/editors/camera.rs`), which `BlockFrameView` and
`DockTabSurface` wrap their view in. Whether a view has one depends on how the
block is placed:

- **A frame of its own** (a tab, a child placed with its own chrome through
  `TabFrame`, or the dock tab holding a joined editor's content): it keeps a
  `Viewport`, remembered per block. The content rectangle is centred on the
  content area plus the pan (`Viewport::shown`), so a resize keeps the middle
  of the world in the middle. The content area is the view less the insets
  the editor last reported (`frame-report.content`), so on a resize the host
  computes the new view itself in the same frame; only a change of the
  editor's own insets lags a frame.
- **Placed by a parent** (a card on a canvas, or a child that took the
  parent's frame): it keeps none. It is told the rectangle the parent placed
  it at and that rectangle's scale over its intrinsic size, and every pan or
  zoom it asks for goes to the nearest frame above that owns a camera
  (`CameraOwner`), never through the parent. That is why zooming while focused
  into a card moves the canvas under it. A placed child without pan and zoom
  is scaled by the host instead: laid out at the assigned size and rendered at
  the rectangle's scale, with input mapped back.

A child that has taken the frame keeps the whole frame as its view, so a pan
changes what it draws, and it should change in the same frame as the canvas
under it. The parent's answer saying where the child sits arrives only after
the parent draws, so the host does not wait: beui marks each `Embed` inside a
panning `Canvas` (`Document::panned_by_canvas`), the app reports those
children as `follows_view`, and the host maps the child's last rectangle from
the view sent at the serial that placed it to the view it is about to send
(`view_rect` on `HostChild`; guides/adding_an_app.md, "6. Frames and pacing").
A card needs none of that: a pan only moves its rectangle, so the host moves
its last texture.

## Drawing through it

An editor hands its `CanvasView` to `<Canvas view={...}>`
(`crates/beui-view/src/components/canvas.rs`, node in
`crates/beui-core/src/base/canvas.rs`). Each `CanvasItem` carries a rectangle
in world coordinates mapped through the view (`origin + point * scale`),
ignoring the `Canvas` node's own laid-out rect, so items track the shared
camera however the layout around the canvas changes. Placing is not
transforming: text in an ordinary item keeps its size, so an item that should
grow with the zoom reads `scale` and sizes itself (the pan-zoom example's
cards), and is laid out and painted again on every frame of a zoom.

`<CanvasItem scales=true>` is transformed instead: laid out in the canvas's
own units inside a scaled space (`Rects::set_scaled_space`,
`Group::Scroll { scale, .. }`), so a pan or zoom changes only that space's
transform, nothing is laid out or painted again, and damage is the canvas
viewport by the group's identity. Its text is laid out at the document's scale
whatever the zoom, so it wraps the same at every zoom. During a zoom the
glyphs are stretched (`Document::set_canvas_raster`, `Zoom::raster`;
`Galley::pixels_per_point` records the density they were rasterised at), and
once the scale has held for 150ms they are rasterised again at the new scale,
repainting without relayout (guides/beui/text.md, Layout scale and raster). Hit testing goes through each space's scale
(`Rects::get`), so pointer events, `TextGeometry`, selection handles, the IME
area and anchored menus all work inside one, in its own units with `pos` in
the window's. Only an `Interactive` takes a press; an item that only draws is
transparent to the pointer, and `pointer=false` lets presses through even its
`Interactive`s (how the infinite canvas edits text only when asked). An
embedded `ChildBlock` sits in an ordinary item.

## The whole frame, and staying transparent

Focused into, a pan-and-zoom editor is handed the whole ancestor frame, not
its card's footprint, so its chrome has room (the example's card list is a
220-point dock pane) while its content stays where the camera puts it. The
host keeps the last painting at the rectangle it was made for until the
editor paints at the new size, both on takeover and when handing the frame
back, rather than stretching it.

That only looks right if the editor paints just its content and chrome. A
frame is transparent wherever the editor does not paint, and an opaque
background around the whole view - fine in a tab - covers the entire canvas
underneath once the editor is a focused child. So fixed chrome gets its own
background where it needs one, and nothing solid wraps the pannable area. A
dock paints its panes' background only while its editor is not placed on a
parent's canvas (`provide_dock_shows_through` in
`crates/be-sdk-beui/src/beui_frame.rs`).
