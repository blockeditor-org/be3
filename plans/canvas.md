# Canvas

A plan to make the infinite canvas (`crates/editors/infinite-canvas`, model in
`crates/be-block/src/canvas.rs`) a general 2D vector and raster image editor,
starting with a prototyping profile as quick to reach for as Excalidraw. Each
step says what "done" looks like. Items marked **(ask)** need a decision from
the user during the work.

## Profiles

A profile is a way of using the editor: its tools, its chrome and its defaults.
Every profile edits the same document format, so a sketch made in one can be
finished in another.

- **Prototyping**: like Excalidraw. Quick, imprecise, hand-drawn by default.
- **Vector**: precise vector editing: bezier paths, the numeric inspector the
  canvas has today, boolean operations.
- **Raster**: pixel layers, one world unit a pixel at their natural size,
  scaled like images, and snapping to whole pixels.
  It replaces the pixel-art editor, which was removed for it and whose tools
  move into the canvas: a raster layer is an entity in the canvas. A game level made of pixel art is a canvas
  holding such layers, or child editors.
- **Interactivity**: a 2D level editor. Entities carry components (they can
  already, through `CanvasComponent`), and a live preview runs modular wasm
  behaviour.

The profile belongs to the viewer, not the document. Switching it never changes
a peer's profile. The document stores the last profile used as a default, so
closing and reopening it comes back the same; it is a viewer default, not
shared live state.

## Where it stands

It has select, frame, line, arrow, rectangle, ellipse, diamond, text and
pressure-aware pen tools; resize, rotate, group, lock, layers, align and
distribute; peers' cursors and selections; child blocks edited in place; and a
phone dock and selection bar. A viewer picks a profile from the mode dock (a
dock bar of radio buttons at the top-left of the stage, `app/mode.rs`; on a
narrow frame it folds into a menu button at the start of the tool dock, since
it would not fit beside the zoom pill), the frame's menu, the context menu or
the palette: Prototyping (the default: a style popover in the selection bar,
hand-drawn shapes, no frame tool), Vector (the accordion inspector) or Raster
(pixel tools and the raster panel). The profile lives in the editor's
`CanvasState`; `Canvas::profile` only keeps the last one used, and
`Root::undoable` keeps that write out of undo. New shapes take the viewer's
last style, kept per profile. A tool belongs to profiles (`Tool::belongs_to`,
which picks the dock's tools); a tool that does not belong is disabled, so one
letter picks a different tool per profile: R, O, L, E and P draw pixel
rectangles, ellipses, lines, erase pixels and draw with the pencil in Raster,
and shapes, lines, the eraser and the pen elsewhere.

The Raster profile (`app/raster.rs`, with the rasterization pixel-art had in
`src/raster.rs`) has pencil (B or P), pixel eraser (E), fill (G), eyedropper
(I), replace colour (C), and pixel line, rectangle and ellipse (L, R, O), with
a brush size (`[` and `]`), a square or round brush, filled shapes (X), mirror
symmetry (H left to right, Shift+H top to bottom; pixel-art's V selects here)
and Shift for straight lines and squares. Its sidebar holds the brush, the
colour and the canvas-wide palette, the selected layer's size, Resize… (an
anchored card with width, height and anchor) and Clear… (an alert), the
layer's opacity, the pixel grid and snapping toggles, and the layers. A
drawing tool paints into the topmost raster layer under the pointer, or starts
a `DEFAULT_RASTER_SIZE` layer centred on the press; a stroke is a gesture
(`Gesture::Raster`, its pixels in `CanvasCells::stroke`) whose `Paint`s go out
live and undo as one step, and a shape tool puts back the pixels it no longer
covers as it is dragged. Each layer is drawn as one texture sampled nearest,
made again only when that layer's grid changes (`EntitySignals::raster`), with
its outline and, once a layer pixel is 12 screen points on its shorter side,
the pixel grid over it. A pixel is one world unit at a layer's natural size,
so one screen pixel at 100% on a 1x display; a layer is created that size,
its corner on whole units. Its handles scale it like an image, keeping its
proportions unless Shift is held, and the pixels draw larger or smaller,
sampled nearest; drawing maps the pointer to a pixel through the layer's
transform. Only Resize… changes the pixel count. Snap to whole pixels
(`CanvasState::snaps`, the Tools menu, the palette and the Raster panel) is
a per-viewer choice kept per profile, on by default in Raster only: while it
is on, moving, nudging, resizing and drawing any shape (not freehand
strokes) put its corners on whole world units. Export embeds a layer as
a PNG `<image>` with `image-rendering="pixelated"` in SVG and upscales it
nearest before the CPU rasterizer in PNG, which samples bilinearly. Paintings
show raster layers blurred for the same reason: `paint_format` has no nearest
sampling.

The model (`be-block/src/canvas.rs`, step 2) is one `Entity` per shape. Its
`shape` names what it is (`CanvasShape`: rectangle, ellipse, diamond, path,
freehand, text, image, block, editor, frame, raster) and decides only how the general
fields are read: `points` is a path or stroke in the unit square of the
entity's transform, each point with optional bezier handles; `pressures` go
with a freehand stroke's points; `block` and `scale` hold an embed; `text` is
rich text (`be_model::RichText`): a text shape's text, a shape's or line's
label, and a frame's name, which stays plain. Text and labels are drawn as
read-only rich text areas the renderer scales, and the one being edited syncs
live with peers through the shared `LiveText`. Every part of the style is its
own field, so concurrent restyles merge.

A raster layer (`CanvasShape::Raster`, model in
`crates/editors/infinite-canvas/types/src/raster.rs`) keeps its pixels in the
entity's `pixels` field, a `be_model::Grid<[u8; 4]>` that merges concurrent
edits per pixel; its bounds may start anywhere, so a paint made during a
resize lands where it was painted, and one cropped away by the other side
counts as a conflict. The pixels are not in `CanvasEntity`: read them with
`Canvas::raster(content, id)` or `Entity::raster`, which share the grid's bytes
(`be_model::Cells` holds them copy-on-write) rather than copy them. The canvas
root holds the raster `palette`. A layer is never rotated, and its transform
may be any size, which scales its pixels: a blank `Add` derives the pixel
count from the transform's size, one pixel a world unit, and puts the corner on
whole units, while an `Add` carrying `pixels` (a duplicate, an Alt-drag or a
paste) keeps them and the transform's size; the clipboard's text carries each
copied layer's pixels as a base64 PNG; `Update` moves and scales it like any shape; and `Resize` keeps the
layer's world units per pixel on each axis and moves the transform by its
anchor so the pixels stay where they were. `Paint`, `Fill`,
`ReplaceColor`, `Clear`, `Resize` and `SetPalette` are pixel-art's operations,
ported, and undo like every other edit.

A line's or arrow's end drawn or dragged onto a shape binds to it, as in
Excalidraw. A path's `start_binding` and `end_binding` name the shape and an
anchor in its unit square: where the pointer put the end, pulled just inside
the outline when it was dropped beside the shape. The end is resolved
against that shape whenever the canvas is read (`CanvasEntity::follow`): it
stops a gap (`BINDING_GAP`) short of the outline on the way from its
neighbouring point to the anchor, so it follows the shape whoever moved it.
In the editor one memo per entity resolves its ends from the held memos of
the shapes it follows, so an end moves in the same frame as its shape. A
selected line or arrow, and one being drawn, shows each bound end's anchor
with a dashed segment from the end to a dot on it. Binding is on by default
and is a per-viewer option of the line and arrow tools (the link button in
the selection bar while one is picked, and "Bind lines and arrows to shapes"
in the Tools menu and the palette); holding Ctrl (Cmd on a Mac) while drawing
or dragging an end keeps that end free. Turning it off leaves lines already
bound as they are.

Shapes, lines and arrows have a clean look and a hand-drawn one: `rough.rs`
ports rough.js, seeded by the shape's stored `seed`, and `app/sketch.rs` turns
an entity into the paths it draws in either look (strokes, solid and hatched
fills, freehand outlines), kept per entity until its geometry or style
changes. Export (`app/export.rs`) reads the same sketches: Copy as PNG / SVG
and Export as PNG / SVG… take the selection, a selected frame (clipped to it),
or the whole canvas. SVG writes each sketch's paths, text and labels as SVG text with a
`<tspan>` per formatted run, and images embedded; PNG paints the entities into an offscreen beui context and
rasterizes that painting on the CPU (`paint_format::rasterize`), since a
plugin cannot read pixels back from the GPU. Both lay text out as the
canvas's text area does (`TextAreaState::lay_out_prose`), and turn a text
shape, and the label of a box shape, by its rotation as the canvas does
(`text::text_turn`; the canvas draws it upright only while it is edited,
since beui's `Turn` does not turn pointer input). A freehand stroke is filled as a perfect-freehand outline
(`freehand.rs`) that follows its pressures; a mouse or finger stroke gets
them when it is drawn, from `freehand::Ink`, which reads the pointer's speed in
screen points per millisecond from each sample's timestamp, so they do not
depend on the sample rate, the frame rate or the zoom, and are stored like a
pen's. The stroke being drawn uses the same outline. Its streamlined
points are joined by a centripetal Catmull-Rom spline and the outline is drawn
as quadratic curves through the midpoints of its points, so sparse samples
still draw a smooth curve. Every length in it is a multiple of the stroke
width, which is in world units like every shape's, so a stroke scales with the
zoom and one drawn at 400% is the same stroke drawn at 100% with a pen four
times as wide; the pen tool keeps a sample only once it is a screen pixel from
the last, so streamline lags by the same screen distance at any zoom.

Every gesture writes the document as it goes - a shape as it is dragged out,
a stroke point by point, a move, resize, rotation, bend, endpoint drag, the
eraser's dimming and a value typed into the inspector - so peers see it
through sync, with no drafts in presence. A gesture is one undo step
(`ContentProjection::begin_gesture` and `commit_gesture` around its writes,
`CanvasState::open_writes` / `commit_writes`), and Escape, a second finger or
another tool calls it off with `cancel_writes`, which reverts it in the
client and leaves nothing to undo. A shape's id and hand-drawn seed are fixed
when the gesture starts. Peers also see each other's carets and selections in a
text or label, whether or not they edit it themselves.

What holds it back:

- On web, opening a canvas needs a server sign-in first.

The canvas is not a frozen format yet (guides/frozen_formats.md), so its model
can change freely until step 8.

## 1. Foundations in beui

- **A path shape.** Fill and stroke, curves, joins, caps and dashes, in
  `beui-renderer-wgpu`, with a painting in beui-demo.
- **Camera transforms in the renderer.** The `Canvas` node places and scales
  its items by a transform, so a pan or zoom changes only that transform, and
  an entity repaints only when it changes.
- **Per-entity signals.** A drag touches only the entities it moves.
- **Benchmarks** (guides/performance.md): pan and zoom frame time on a canvas
  of 10,000 shapes, and the latency from pen down to ink on screen.

## 2. Model v2

- Shapes: rectangle, ellipse, diamond, lines and arrows with any number of
  points, freehand, text, image, block embed, and frame (today's artboard).
- Line and arrow ends bind to shapes and follow them, stopping a gap short of
  the outline, and a selected one shows its anchors with dashed segments.
  Binding is a per-viewer option of the line and arrow tools, and Ctrl keeps
  an end free while drawing. A shape or an arrow can carry a text label.
- Style saved field by field, so concurrent changes merge.
- A hand-drawn look (like rough.js) with a fixed seed per shape, and a clean
  look.
- Freehand stored as points and pressures and drawn as a smoothed outline that
  follows pressure (like perfect-freehand), filled as a path. A mouse or finger
  simulates pressure.
- Geometry general enough for the vector profile's bezier paths, so that
  profile adds tools, not a second model.
- Text is rich text. A be-model type stores rich text, so block-app can hold
  the rich text beui's editor already edits, and text shapes and labels use
  it. One document provider serves both the text editor and the canvas, so
  the canvas does not reimplement it (as the first attempt, PR #519, had to).

## 3. The prototyping profile

- A small style popover by the selection: a few colour swatches, three stroke
  widths, fill none / hatched / solid, stroke style, roughness, opacity, and
  text sizes S / M / L / XL as design tokens. The current inspector moves to
  the vector profile.
- Hand-drawn by default. New shapes take the last style used.
- One-letter and number shortcuts for every tool, and a tool lock.
- Double-clicking empty canvas types text; Enter on a selected shape edits its
  label. Text and labels are edited live as rich text in place, through the
  shared document provider, and peers see the edits as they are typed.
- An eraser.
- A dot grid behind the canvas: a setting of the canvas, saved with it and
  shared by everyone viewing it, that can be turned off or spaced
  differently.
- No snapping: in a quick sketch nobody should have to worry about lines
  being perfectly straight or boxes perfectly aligned. This departs from
  Excalidraw. Snapping comes in step 6.
- Lasso selection, double-click into a group, drag a line's midpoints to bend
  it.
- Copy and export a selection or frame as PNG and SVG.

## 4. Input on every device

- Pen: in the pen tool, the pen draws and a finger selects or pans; the eraser
  end and barrel button.
- Touch: a toggle that turns touch input off entirely, so a resting hand never
  draws, selects or zooms. No palm rejection: heuristics misfire, which is why
  people draw in gloves even on devices known for good palm rejection.
- Web: read coalesced and predicted pointer events (`getCoalescedEvents`,
  `getPredictedEvents`) in `beui-platform-web`, for smoother strokes and less
  latency.
- Touch: long press opens the context menu; check what one finger does in
  every tool.
- Trackpad: two-finger scroll pans and pinch zooms, on web too.
- Peers see a stroke while it is drawn, through presence, not only on release.

## 5. As quick to open as a URL

- Visiting the web app with no account opens a blank canvas at once, kept in a
  local-only workspace in the browser (its origin private file system; see
  `plans/public_release.md`, Web offline). Local-only workspaces are permanent, not a testing aid.
- Signing in later can move the canvas into a remote workspace and share it by
  link.
- Load only the canvas plugin to start, and cache the app so it opens offline
  and installs to a home screen. Done: the service worker that opens it
  offline (`crates/block-app/web/sw.js`).
- A cold-start target on a mid-range phone, measured in CI. **(ask)** the
  target.

## 6. Precision and reuse

- Snapping to other shapes' edges and centres, with guides and equal-spacing
  hints. Off in the prototyping profile unless a viewer turns it on; on in the
  more precise profiles. Today only angles snap.
- A library of reusable shapes. First build what makes it easy: per-workspace
  settings, and a good way for an editor to provide data. **(ask)** whether a
  library is a block.

## 7. The other profiles

Vector, then raster (with the pixel-art merge), then interactivity. Each gets
its own plan when it starts.

## 8. Freeze the format

Once steps 2 and 3 have settled, add the canvas to guides/frozen_formats.md
with its tests.
