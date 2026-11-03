# Canvas

The infinite canvas is a 2D vector and raster editor. Its camera is the host's
(guides/pan_and_zoom.md); `Fit::Centre` keeps it fitted to the origin, and Fit
all is an `editor.fit` of the first frame, else everything, else the origin.
Its format is not frozen yet (guides/frozen_formats.md). plans/canvas.md has
the work left.

## Where the code lives

- `crates/be-type-infinite-canvas`: the block type, built as a library and as
  the type module the app runs (guides/type_modules.md). `src/canvas.rs` is the
  model and its operations; `src/raster.rs` is the pixel model.
- `crates/be-app-infinite-canvas`: the editor app (`CanvasRoot`).
  - `root/state.rs`: `CanvasState`, the editor's per-viewer state (tool,
    profile, selection, styles, `gesture`, `binds`, `snaps`) and every edit it
    makes. `Tool` and `Gesture` are here.
  - `root/entities.rs`: `Entities`, one set of memos per entity
    (`EntitySignals`: `stored`, the entity as saved; `held`, with bound ends
    resolved; `raster`), handed out by `watch(id)` and dropped when nothing
    watches it, plus a catalog of 256-unit world cells for hit testing and the
    canvas's extent.
  - `root/canvas.rs`: the stage, one node per entity; `root/input.rs`: pointer
    and key handling; `root/overlay.rs`: handles, guides and peers.
  - `root/sketch.rs` and `rough.rs`: what an entity draws; `freehand.rs`:
    strokes; `root/text.rs`: text and labels; `root/export.rs`: copy and
    export; `root/raster.rs` and `raster.rs`: the Raster tools.
  - `root/mode.rs`: the profile switcher; `root/style.rs`: the Prototyping
    style popover; `root/sidebar.rs`: the Vector inspector; `root/toolbar.rs`,
    `root/selection_bar.rs`, `root/menu.rs`, `root/actions.rs` (every action and
    its shortcut), `root/layers.rs`, `root/grid.rs`, `root/touch.rs`.

## The model

`Canvas` (the root) holds `entities`, a `List<Entity>` in paint order, the dot
`grid` (shared by everyone viewing), the last `profile` used and the raster
`palette`. An `Entity` is one shape; `CanvasEntity` is the plain view of one
that the editor and operations pass around (`Entity::view`).

`shape` (`CanvasShape`: Rectangle, Ellipse, Diamond, Path, Freehand, Text,
Image, Block, Editor, Frame, Raster) decides how the general fields read:

- `center`, `size`, `rotation`: the transform. Editors, frames and raster layers
  are never rotated (`normalized_entity`).
- `points` (`CanvasPathPoint`, each with optional bezier handles) are a path's
  or stroke's points in the unit square of the transform. A stroke drawn with
  the pen keeps its points in world units with a `points_frame` around them
  (`set_drawn_points`), so `Extend` can append to it without rewriting what is
  there; `world_points` reads either form.
- `pressures` go with a freehand stroke's points.
- `block` and `scale` hold an embedded block or child editor.
- `text` is `RichText`: a text shape's text, a shape's or line's label, a
  frame's name.
- Every part of the style (`stroke_color`, `fill`, `look`, `roughness`, `seed`,
  ...) is its own field, so concurrent restyles of different fields merge.
- `components` carry `CanvasComponent`s: a database schema and its values.

Edits go through `InfiniteCanvasOperation` and `Canvas::edit_in`: `Add`,
`Update`, `Remove`, `Reorder`, `ExactOrder` and `Extend` (append points and
pressures to a stroke), and for rasters `Paint`, `Fill`, `ReplaceColor`,
`Clear`, `Resize` and `SetPalette`. A profile write is never undone
(`Canvas::undoable`).

### Raster layers

A `CanvasShape::Raster` entity keeps its pixels in `pixels`, a
`Grid<[u8; 4]>` that merges concurrent edits per pixel. Its bounds may start
anywhere, so a paint made during a peer's resize lands where it was painted,
and one the other side cropped away is a conflict. Read pixels with
`Canvas::raster(content, id)` or `Entity::raster`, which share the grid's
bytes (`CanvasRaster`) rather than copy them; they are not in `CanvasEntity`.

At its natural size one pixel is one world unit. A blank `Add` derives the
pixel count from the transform's size (`raster_dimensions`) and puts the corner
on whole units; an `Add` carrying `pixels` (duplicate, Alt-drag, paste) keeps
them and the transform's size. `Update` moves and scales a layer like any
shape, which scales its pixels; only `Resize` changes the pixel count, keeping
world units per pixel and moving the transform by its anchor so the pixels stay
put. `DEFAULT_RASTER_SIZE` is 32, `MAX_RASTER_SIZE` 2048. The clipboard
carries each copied layer as a base64 PNG (`encode_raster`).

## Profiles

`CanvasProfile` is Prototyping (default), Vector or Raster. The profile is the
viewer's (`CanvasState::profile`); `Canvas::profile` only remembers the last
one, read once when the editor opens
(decisions/canvas_profile_belongs_to_the_viewer.md). A viewer switches it from
the mode dock (`root/mode.rs`, folded into a menu in narrow chrome), the menu,
the context menu or the palette.

- Prototyping: the style popover in the selection bar, hand-drawn new shapes
  (`profile_style`), no frame tool.
- Vector: the accordion inspector, clean shapes.
- Raster: the pixel tools (`PixelTool`: pencil, eraser, fill, eyedropper,
  replace colour, line, rectangle, ellipse; brush size and shape, filled
  shapes, mirror symmetry) and the raster panel (colour, palette, layer size,
  Resize… and Clear… dialogs, opacity, pixel grid, snapping, layers).

`Tool::belongs_to` picks each profile's dock tools; a tool that does not belong
is disabled, so one letter picks a different tool per profile (R is a
rectangle in Prototyping and Vector, a pixel rectangle in Raster). New shapes
take the viewer's last style, kept per profile.

Snapping covers angles and, per viewer and per profile, whole pixels
(`CanvasState::snaps`, on by default only in Raster): moving, nudging,
resizing and drawing any shape but freehand put its corners on whole world
units (`moved_to_whole_units`, `sized_to_whole_units`). Prototyping has no other
snapping (decisions/canvas_prototyping_has_no_snapping.md).

In Raster, a drawing tool paints into the topmost raster layer under the
pointer or starts a `DEFAULT_RASTER_SIZE` layer centred on the press. A stroke
is `Gesture::Raster`; its `Stroke` (held in the state's cells) maps the pointer
to a pixel through the layer's transform and sends `Paint`s live, and a shape
tool puts back the pixels it no longer covers as it is dragged. Each layer
draws as one texture, made again only when its `raster` memo changes, with the
pixel grid over it once a pixel is 12 screen points on its shorter side
(`GRID_MIN_CELL`).

## Line binding

A path's `start_binding` and `end_binding` (`CanvasBinding`) name a bindable
shape and an anchor in its unit square. `CanvasEntity::follow` resolves each
bound end whenever the canvas is read: it stops `BINDING_GAP` short of the
outline on the way from the neighbouring point to the anchor. In the editor the
`held` memo of a line resolves its ends from the memos of the shapes it
follows, so an end moves in the same frame as its shape. A selected or
being-drawn line shows each anchor with a dashed segment. Binding is the
per-viewer `CanvasState::binds` (on by default); Ctrl (Cmd on a Mac) keeps an
end free while drawing or dragging it.

## Drawing

Every shape has a clean and a hand-drawn look (`CanvasLook`). `rough.rs` is a
port of rough.js seeded by the entity's `seed`, fixed when its gesture starts.
`root/sketch.rs` turns an entity into a `Sketch` (strokes, solid and hatched
fills, freehand outlines), kept per entity until its `SketchKey` (shape,
transform, points, pressures, style) changes.

A freehand stroke is filled as a perfect-freehand style outline
(`freehand::stroke`) that follows its pressures, through a centripetal
Catmull-Rom spline of its streamlined points, drawn as quadratic curves. Every
length in it is a multiple of the stroke width, which is in world units, so a
stroke scales with the zoom. The stroke being drawn is a `LiveStroke`, cut
into pieces of 48 points so each new sample repaints only the tail. A mouse or
finger gets pressures from `freehand::Ink`, which reads speed in screen points
per millisecond from sample timestamps, so they do not depend on the sample or
frame rate or the zoom; they are stored like a pen's. The pen tool keeps a
sample only once it is a screen pixel from the last one.

Text and labels draw as read-only rich text areas the renderer scales; the one
being edited syncs live with peers through `LiveText` (`root/text.rs`), and
peers see each other's carets and selections. The canvas draws turned text
upright only while it is edited (`text::text_turn`).

## Gestures

Every gesture writes the document as it goes (a shape as it is dragged out, a
stroke through `Extend`, a move, resize, rotation, bend, endpoint drag, the
eraser's dimming, a typed inspector value), so peers see it through sync.
`CanvasState::open_writes` and `commit_writes` wrap a gesture's writes in
`ContentProjection::begin_gesture` and `commit_gesture`, making it one undo
step. Escape, a second finger or another tool calls it off with
`cancel_writes`, which reverts it and leaves nothing to undo.

## Input

- Pen: in the pen tool the pen draws and, once a pen has been seen, a finger
  selects (`tool_for`). The eraser end erases; the barrel button opens
  the context menu.
- Touch: Ignore touch (`root/touch.rs`, the last button in the dock) is the
  this-device display setting `canvas_touch`, which keeps fingers off the canvas
  entirely (decisions/touch_off_toggle_instead_of_palm_rejection.md). A long
  press opens the context menu in every tool.
- Web: `beui-platform-web` reads `getCoalescedEvents` and `getPredictedEvents`;
  the pen tool keeps every coalesced sample and draws the predicted tail
  without storing it.
- Trackpad scrolling and pinching, wheels and two-finger pans move the host's
  camera; the canvas handles none of them itself.

## Export

`root/export.rs` reads the same sketches. Copy as PNG / SVG and Export as
PNG / SVG… take the selection, a selected frame (clipped to it) or the whole
canvas. SVG writes each sketch's paths, text as `<text>` with a `<tspan>` per
formatted run, images embedded, and raster layers as PNG `<image>`s with
`image-rendering="pixelated"`. PNG paints into an offscreen beui context and
rasterizes it on the CPU (`beui_paint_format::rasterize`), since an app cannot
read pixels back from the GPU; raster layers are upscaled nearest first
(`nearest_upscaled`) because that rasterizer samples bilinearly. Both lay text
out as the canvas does (`TextAreaState::lay_out_prose`) and turn it with
`text_turn`.

## Testing and benchmarks

- `crates/be-type-infinite-canvas/src/tests/`: the model's merges and
  operations (peers restyling, binding, raster paint, fill, resize, clear and
  undo).
- `crates/be-app-infinite-canvas/src/tests/`: the editor through be-sdk-test.
  `src/tests.rs` has the helpers: `editor`, `phone`, `vector_editor`,
  `raster_editor` open one; `apply`, `entities`, `undo`, `sync` and
  `forward_presence` drive it and a peer; `every_shape` builds one of each
  shape. Its paintings are in `snapshots/`, among them
  `every_shape_is_drawn_in_the_clean_look`,
  `every_shape_is_drawn_in_the_hand_drawn_look`,
  `freehand_is_drawn_as_an_outline_that_follows_pressure` and
  `a_raster_layer_is_drawn_pixel_sharp_with_its_grid`. Paintings show raster
  layers blurred: `beui_paint_format`'s rasterizer has no nearest sampling.
- Tests that a pan, zoom or drag repaints nothing it should not
  (`panning_and_zooming_paints_no_entity_again`,
  `dragging_one_entity_wakes_only_that_entity`,
  `a_pen_move_paints_and_visits_no_entity_again`) guard the per-entity
  signals.
- be-host's own canvas tests (tool bar docking and popping out, menus, a canvas
  inside a canvas) are in `crates/be-host-tests`.
- `//crates/be-app-infinite-canvas:example-canvas-bench` (`examples/canvas-bench.rs`)
  times open, pan, zoom, drag and ink on 100 and 10,000 shapes, and the
  suite's `app-canvas-and-text` times a pen line in be-host (guides/performance.md).
