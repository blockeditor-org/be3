# Pan and zoom

The camera a pan-and-zoom editor is handed by its host: what the editor gets, how it draws
through it, who owns it, and why its frame must stay transparent. Read this when writing an
editor whose content has a world of its own (a canvas, a picture, a map), or when changing how
the host pans and zooms editors.

## Why the host owns the camera

Pan and zoom is a plugin capability, `capabilities.pan_and_zoom` in the manifest
(`guides/plugins/adding_an_editor.md`), not something an editor builds for itself. An editor with
it declares that its content has a camera of its own (a world drawn at a position and a scale)
and hands the camera to whatever hosts it instead of keeping one.

The same editor code then works in a tab of its own and inside another editor's canvas without
knowing which. "Zoom in" means the same to the pixel art editor whether the person clicked its
toolbar button or turned the wheel over it while it was a card on an infinite canvas: in both
cases the camera it draws through was handed to it.

`pan_zoom` (`crates/editors/pan-zoom`) is the reference implementation. Its block stores nothing,
and its whole view is built the same way wherever the host puts it.

## Writing a pan-and-zoom editor

1. Set `capabilities.pan_and_zoom` in the manifest (and `max_zoom` if the default of 32 is not
   enough).
2. Name the node the content is in once, with `editor.content(&node_ref)`
   ([Reporting where the content is](#reporting-where-the-content-is)).
3. Hand `editor.canvas()` to a `<Canvas view={...}>` and place the content as `CanvasItem`
   children in world coordinates ([Drawing through it](#drawing-through-it)).
4. Wire zoom and fit controls to the requests below; keep no offset or zoom of your own.
5. Leave wheel, pinch and middle- or space-drag to the host.
6. Keep the content area transparent ([Staying transparent](#staying-transparent)).

## What the editor is given

| Call | What it is |
|---|---|
| `editor.canvas()` | `ReadSignal<Option<CanvasView>>`: where the content goes, a plain `{origin, scale}` (`beui::reactive::CanvasView`), in the editor's own coordinates |
| `editor.scale()` | the scale the content is shown at |
| `editor.pan(delta)` | ask the host to pan |
| `editor.zoom(factor)`, `editor.zoom_at(factor, anchor)` | ask the host to zoom |
| `editor.fit()` | ask the host to fit the content |
| `editor.reveal(rect)` | pan a rectangle of the editor's own world into the middle of the content (a jump-to-this list) |

- Both signals are written before every frame.
- The editor draws its content there, at the resolution it is shown at, and keeps no zoom or
  offset of its own.
- An instance never told where its view is fills the content it is given at a scale of one. That
  is what happens on the first frame, and to an editor with no camera to speak of.
- The `-`, `+` and Fit buttons of a toolbar call `zoom`, `zoom` and `fit`. The pixel art editor puts
  a zoom readout, zoom buttons and a fit button in its own toolbar.
- The host answers against whatever it shows the instance in, so these controls work the same in
  a tab and inside an infinite canvas, where zooming zooms the canvas and the block's content grows
  with it.
- A zoom readout is true either way as long as it reads from the view it was given: the rectangle
  for a picture of known size (the pixel art editor divides it by the image), the scale for a world
  of its own (the canvas draws a world unit that many pixels wide).

A plugin without beui gets the same from its `EditorHost`: `host.view()` (the rectangle, in the
coordinates it draws in), `host.view_scale()`, `host.pan_view(delta)`,
`host.zoom_view(factor, anchor)` and `host.fit_view()`.

### Gestures

- Wheel, pinch, and middle- or space-drag over the content belong to the host. It reads them once,
  for the outermost view under the pointer, so a wheel over a block edited inside a canvas moves
  the canvas rather than both.
- An editor leaves them alone, reading them only to know not to start a gesture of its own: the
  pixel art editor holds off its tools while the view is being dragged.
- A gesture only the editor understands (the PDF editor panning on a plain left drag) it turns into
  a request of its own, the same way a button does.

## Drawing through it

Hand the `CanvasView` straight to a `<Canvas view={...}>` node
(`crates/beui-view/src/components/canvas.rs`, over the node in
`crates/beui-core/src/base/canvas.rs`).

- A `Canvas` does not lay its items out through the normal measure/layout pass. Each `CanvasItem`
  child carries a rectangle in the editor's world coordinates, and the canvas maps it through the
  view (`origin + point * scale`) to where it paints, ignoring the `Canvas` node's own laid-out
  rect.
- That keeps an item's screen position on the shared camera exactly, even though the surrounding
  layout (a sidebar taking width out of the frame) does not know where the canvas sits on some
  ancestor's canvas.
- The canvas clips and culls to the rectangle it was laid out at.
- A canvas never told where its view is fills the space it was given at a scale of one. One given
  a width and a height reports that as its size, which lets a canvas of absolutely placed items sit
  inside a `Scroll`.
- Placing an item is not transforming it: text inside keeps the size it asked for. An editor whose
  content should grow with the zoom reads the scale and sizes its text by it, the way `pan_zoom`'s
  cards scale their title and padding off `scale`.

## Reporting where the content is

The host cannot see where a beui editor put its sidebar. The editor names the node its content is
in once, with `editor.content(&node_ref)`, and the framework reads that node's rectangle after
every layout. That is the rectangle the host pans, zooms and reads wheel gestures against, so a
wheel over the sidebar is not read as a wheel over the content. The host works out where in that
band the content goes, against the band the editor reported last frame.

## Who owns the camera

The host side is `BlockFrameView` in `crates/block-app/src/compositor/editors.rs`, the component
that shows one block's editor in a frame. Whether it owns a camera depends on how the block was
placed.

### A frame of its own

A tab, or a child a parent places with its own chrome (through `TabFrame`). Nothing above has a
camera for it, so it keeps one:

- a `Viewport` signal with a zoom, a pan offset and an auto-fit flag that keeps refitting the
  content until the person's own gesture cancels it;
- remembered per block, so it survives the component being rebuilt;
- wheel, pinch and pan-drag over the region, and the editor's own `ViewChange`s, settle into it;
- the resulting content rectangle and scale go to the editor as its `EditorView`, with the
  region's placement.

### Placed by a parent

A card on a canvas, or a child that has taken the parent's frame over. The parent's camera already
decides where the child lands, so the child keeps none:

- it is told the rectangle the parent placed it at, at the scale that rectangle has over the
  child's intrinsic size;
- its gestures and `ViewChange`s are reported to the parent as child view changes
  (`ChildBlock`'s `on_view_change`) instead of being applied.

### How a canvas shares its camera

An infinite canvas, every frame, loops over the children it placed, takes each one's queued
`ViewChange`s (`Pan`, `Zoom`, `ResumeAutoFit`) and applies them straight to its own `Viewport`, the
same struct that answers its own scroll and pinch.

- `Fit` is the exception: fitting a child means fitting that child's rect into the canvas's world,
  which only the canvas can compute, so it is remembered as "fit this entity" and resolved the
  next frame.
- This loop is what makes the camera shared rather than handed down once: a zoom the focused child
  requests becomes a `ViewChange` the canvas's viewport absorbs, so panning or zooming while
  focused into a card moves the canvas under it too.

### Without a host

A beui view that needs the same gestures with no host to hand it a camera (a standalone canvas, or
the demo's stage) composes `beui::unstyled::PanZoom`, which reads wheel, pinch and middle-drag over
its own rectangle and reports a camera its caller owns. It is the gesture half of the same idea,
not a second camera: an editor placed on a host still takes the camera it is given rather than
keeping a `PanZoom` of its own.

## Taking the frame over

A pan-and-zoom editor that is focused is handed the whole ancestor frame to draw in, not a
rectangle clipped to where it sits on the parent canvas. Its real chrome needs somewhere to live
(`pan_zoom`'s sidebar is a fixed 220px, which cannot fit a card's footprint), and its content
still belongs where the shared camera says, which can be smaller than, bigger than, or partly
overlapping that footprint once the person pans and zooms.

Taking the frame over resizes the editor's screen from a card's footprint to the whole frame in
one step, and a plugin draws a frame behind the host, so the painting the host holds was made for
the old size. Stretching it would flash the whole editor scaled up, so the host keeps the last
painting at the rectangle it was made for until the editor has painted at the new size, and the
same in reverse when the frame is handed back. Nothing about this reaches the editor. It applies
only to a takeover, not an ordinary resize, where a stretched frame is a pixel or two and following
the window matters more.

## Staying transparent

A frame is transparent wherever the editor does not paint, which lets the layer beneath show
through around a block edited inside a canvas. That only looks right if the editor paints just its
own content and chrome. A `Canvas` keeps to this by construction: its `paint` draws only its
items.

- Do not wrap the whole view (sidebar, rail and stage) in one opaque background, as a non-canvas
  editor reasonably might for its always-solid tab. It looks right as a standalone tab, but once
  the editor is focused as a child the stage spans the whole frame and the background paints over
  the entire canvas underneath.
- Give fixed chrome (a sidebar, a rail) its own opaque background where it needs to stay legible
  over whatever is behind it.
- Never wrap the pannable content area in anything that paints its full bounds solid.
