# Canvas

The infinite canvas is a 2D vector and raster editor. Its camera is the host's
(guides/pan_and_zoom.md). Like every format, its format is
not frozen (guides/frozen_formats.md). plans/deferred/canvas.md has the work left.

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
A copy is `be3/canvas-items`: the entities in the canvas's JSON, naming the
canvas they came from, whose content a paste reads for the pixels of raster
layers (`CanvasState::rasters_from`), so the clip stays small. The JSON holds no
block ids: the canvas and every block the entities name (the block shown, each
component's schema, and the blocks and databases component values name, in the
order `CanvasEntity::blocks_mut` walks them) travel in the item's refs, so the
host can translate them, and a paste fills them back in that order, refusing a
clip whose count does not match. Beside it go
`be3/blocks` for the blocks the entities show, a PNG of the selection the PNG
exporter makes only when a paste asks for it (`ExporterOf::once`), and the text
of text items.
A paste of entities that show blocks waits for `paste-blocks`, and leaves out
those the host refuses.

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

A gesture that makes something writes it as it goes, so peers see it: a shape
drawn out, a stroke through `Extend`, a raster stroke, a duplicating move's
copies, new text. `CanvasState::open_writes` brackets it as one undo step. A
gesture that chooses one result (`Gesture::chooses`: moving, resizing, rotating,
dragging a line's end or bend, the eraser's dimming) holds its writes in a
tentative gesture (`open_hold`), as do the sidebar's value controls while one is
dragged or typed into (`drag_value` follows their `ValueDrag`, `type_value`): every view of the block on
this device previews them, and peers see nothing of them but a carried move
(below) until the release writes them as one undo step. `commit_writes` commits both; Escape, a second finger or
another tool calls it off with `cancel_writes`, leaving nothing written or to
undo. The eraser drops its held dimming on release and writes the removal alone.
When the replica ends the hold (an undo mid-drag), `gesture_ended` lets go of
the drag.

Moving the selection is a drag board carry (`root/carry.rs`;
guides/beui/components.md, Drag and drop): a press on the selection begins
`Gesture::Move`, and the surface's `drag` lifts it once the press passes the
threshold, or Move in the palette lifts it from the keyboard (Space stays the
host's, a held Space panning: `lifts_on_space=false`) and the arrows step it. The
arrows also nudge a selection not carried, by the same world unit (ten with
Shift), each nudge an edit of its own; a carry is the move a screen reader hears
place by place, that Escape puts back, written once when dropped. While the
board carries the selection, the canvas's own keys and commands wait, so nothing
deletes or reselects what is carried. The
canvas is a `free(..).unbounded()` site whose place is the world point under the
pointer, applied to the move through the hold; the board's drop commits it, and
a put-down (Escape, a cancelled press) drops it. While the board carries a move
the release leaves it to the board, and `gesture_ended` puts the carry down. The
surface is an edge scroller: a carry held in its band pans the camera, and the
site chooses again as the camera moves. A move held in the hold reaches peers as
presence instead: `CanvasCursor::carried` gives the offset of the selection
it carries (and whether it snaps), and a peer's `Entities` draws what of that
selection is unlocked there (`carry_for_peers`, the `drawn` memo the stage
reads; edits read `stored`), with lines bound to it following.

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

`root/export.rs` is the canvas's two exporters (guides/apps/editor.md,
"Exporting"): `png` makes an image block and `svg` a plain file block, from
`ImageExport` settings (scale, background, and the items: what was selected,
a selected frame, clipped, or none for the whole canvas). Each reads the
canvas through a `CanvasReader`, the entities and the graph's labels without
the document a `CanvasState` needs, and draws in the colours of the theme the
host has the app show (`be_sdk::host_preferences`), so an export looks as the
canvas does; it reads no view's theme, since an artifact is made with no
document. There is no direct Copy as PNG or SVG:
the Export dialog's clipboard destination does that. It reads the same sketches as the canvas. SVG writes paths, text as `<text>` with a
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
