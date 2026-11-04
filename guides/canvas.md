# Canvas

The infinite canvas is a 2D vector and raster editor. Its camera is the host's
(guides/pan_and_zoom.md). Its format is not frozen yet
(guides/frozen_formats.md). plans/canvas.md has the work left.

## Where the code lives

- `crates/be-type-infinite-canvas`: the block type (guides/type_modules.md).
  `src/canvas.rs` is the model and its operations, `src/raster.rs` the pixel
  model.
- `crates/be-app-infinite-canvas`: the editor (`CanvasRoot`).
  - `root/state.rs`: `CanvasState`, the per-viewer state (tool, profile,
    selection, styles, the current `Gesture`, binding and snapping) and every
    edit the editor makes.
  - `root/entities.rs`: `Entities`, one set of memos per entity, handed out by
    `watch(id)` and dropped when nothing watches it, plus a grid of 256-unit
    world cells for hit testing and the canvas's extent.
  - `root/canvas.rs` the stage, one node per entity; `root/input.rs` pointer
    and keys; `root/overlay.rs` handles, guides and peers.
  - `root/sketch.rs` and `rough.rs` what an entity draws; `freehand.rs`
    strokes; `root/text.rs` text and labels; `root/export.rs` copy and export;
    `root/raster.rs` and `raster.rs` the Raster tools.
  - Chrome: `root/mode.rs` (profiles), `root/style.rs`, `root/sidebar.rs`,
    `root/toolbar.rs`, `root/selection_bar.rs`, `root/layers.rs`,
    `root/menu.rs` (frame and context menu from the same submenus),
    `root/actions.rs` (every action and its shortcut), `root/touch.rs`.

## The model

`Canvas` holds `entities`, a `List<Entity>` in paint order, the dot grid, the
last profile used and the raster palette. An `Entity` is one shape; its
`shape` decides how the general fields read (`CanvasEntity`, from
`Entity::view`, is the plain form the editor passes around). Every part of the
style is its own field, so concurrent restyles of different fields merge.

- Path and stroke points are in the unit square of the transform, except a pen
  stroke, which keeps world-unit points with a `points_frame` so `Extend` can
  append without rewriting them (`world_points` reads either).
- Editors, frames and raster layers are never rotated (`normalized_entity`).
- Edits go through `InfiniteCanvasOperation` and `Canvas::edit_in`. A profile
  write is never undone (`Canvas::undoable`).

### Raster layers

A raster entity keeps its pixels in a `Grid<[u8; 4]>`, which merges concurrent
edits per pixel. Its bounds may start anywhere, so a paint made during a
peer's resize lands where it was painted, and one the other side cropped away
is a conflict. Pixels are not in `CanvasEntity`; read them with
`Canvas::raster` or `Entity::raster`, which share the grid's bytes.

At natural size one pixel is one world unit. `Update` moves and scales a layer
like any shape (scaling its pixels); only `Resize` changes the pixel count,
keeping world units per pixel and moving the transform so the pixels stay put.
The clipboard carries a layer as a base64 PNG.

## Profiles

`CanvasProfile` is Prototyping (default), Vector or Raster. The profile is the
viewer's (`CanvasState::profile`); `Canvas::profile` only remembers the last
one, read once on open (decisions/canvas_profile_belongs_to_the_viewer.md).

- Prototyping: a style popover in the selection bar, hand-drawn new shapes, no
  frame tool, no snapping (decisions/canvas_prototyping_has_no_snapping.md).
- Vector: the inspector, clean shapes.
- Raster: the pixel tools and the raster panel.

`Tool::belongs_to` picks each profile's tools; one letter picks a different
tool per profile (R is a rectangle or a pixel rectangle). New shapes take the
viewer's last style, kept per profile. Whole-pixel snapping is per viewer and
profile, on by default only in Raster.

In Raster a drawing tool paints into the topmost raster layer under the
pointer, or starts a new layer centred on the press. Each layer draws as one
texture, rebuilt only when its `raster` memo changes, with a pixel grid once
a pixel is `GRID_MIN_CELL` points across.

## Line binding

A path's `start_binding` and `end_binding` name a shape and an anchor in its
unit square. `CanvasEntity::follow` resolves each bound end whenever the
canvas is read, stopping `BINDING_GAP` short of the outline. In the editor a
line's `held` memo reads the memos of the shapes it follows, so an end moves in
the same frame as its shape. Ctrl (Cmd on a Mac) keeps an end free.

## Drawing

Every shape has a clean and a hand-drawn look. `rough.rs` is a port of
rough.js seeded by the entity's `seed`, fixed when its gesture starts.
`root/sketch.rs` turns an entity into a `Sketch`, cached until its
`SketchKey` changes.

A freehand stroke is a perfect-freehand style outline (`freehand::stroke`)
following its pressures. Every length is a multiple of the stroke width in
world units, so a stroke scales with the zoom. The stroke being drawn is a
`LiveStroke`, cut into 48-point pieces so a new sample repaints only the
tail. A mouse or finger gets pressures from `freehand::Ink`, from speed in
screen points per millisecond of sample timestamps, so they do not depend on
the sample rate, frame rate or zoom.

Text draws as read-only rich text areas the renderer scales; the one being
edited syncs live through `LiveText`. Turned text is drawn upright only while
edited (`text::text_turn`).

## Gestures

Every gesture writes the document as it goes (adding a shape, a stroke through
`Extend`, moving, resizing, the eraser's dimming, a typed inspector value), so
peers see it. `CanvasState::open_writes` and `commit_writes` bracket it as one
undo step; Escape, a second finger or another tool calls it off with
`cancel_writes`, leaving nothing to undo.

## Input

- Pen: in the pen tool the pen draws and, once a pen has been seen, a finger
  selects (`tool_for`). The eraser end erases; the barrel button opens the
  context menu.
- Touch: the dock's Ignore touch button is the this-device display setting
  `canvas_touch` (decisions/touch_off_toggle_instead_of_palm_rejection.md). A
  long press opens the context menu.
- Web: the pen tool keeps every coalesced sample and draws the predicted tail
  without storing it (`beui-platform-web`).
- Wheels, trackpads and two-finger pans move the host's camera; the canvas
  handles none of them.

## Export

`root/export.rs` reads the same sketches, for the selection, a selected frame
(clipped) or the whole canvas. SVG writes paths, text as `<text>` with a
`<tspan>` per run, and raster layers as pixelated PNG images. PNG paints into
an offscreen beui context and rasterizes on the CPU
(`beui_paint_format::rasterize`), since an app cannot read pixels back from
the GPU; raster layers are upscaled nearest-neighbour first because that
rasterizer samples bilinearly.

## Testing and benchmarks

- `crates/be-type-infinite-canvas/src/tests/`: merges and operations.
- `crates/be-app-infinite-canvas/src/tests/`: the editor through be-sdk-test;
  `src/tests.rs` has the helpers that open an editor in each profile, drive it
  and a peer, and build one of every shape. Paintings show raster layers
  blurred, since `beui_paint_format`'s rasterizer has no nearest sampling.
- Tests that a pan, zoom, drag or pen move repaints no other entity
  (`panning_and_zooming_paints_no_entity_again`,
  `dragging_one_entity_wakes_only_that_entity`) guard the per-entity memos.
- be-host's canvas tests (dock bars, menus, a canvas inside a canvas) are in
  `crates/be-host-tests`.
- `//crates/be-app-infinite-canvas:example-canvas-bench` times open, pan,
  zoom, drag and ink on 100 and 10,000 shapes; the suite's
  `app-canvas-and-text` times a pen line in be-host (guides/performance.md).
