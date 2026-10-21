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
- **Raster**: one consistent pixel size, which lets display take shortcuts.
  The pixel-art editor (`crates/editors/pixel-art`) merges into it: a raster
  layer is an entity in the canvas. A game level made of pixel art is a canvas
  holding such layers, or child editors.
- **Interactivity**: a 2D level editor. Entities carry components (they can
  already, through `CanvasComponent`), and a live preview runs modular wasm
  behaviour.

The profile belongs to the viewer, not the document. Switching it never changes
a peer's profile. The document stores the last profile used as a default, so
closing and reopening it comes back the same; it is a viewer default, not
shared live state.

## Where it stands

It has select, artboard, line, rectangle, text and pressure-aware pen tools;
resize, rotate, group, lock, layers, align and distribute; peers' cursors and
selections; child blocks edited in place; and a phone dock and selection bar.
What holds it back:

- beui draws only rectangles, straight lines, text and images
  (`beui-core/src/painter.rs`). There are no paths, so no ellipses, curves,
  clean joins or filled freehand: a pen stroke is line segments with a dot at
  each joint (`paint.rs::stroke`).
- The camera is applied while painting (`Camera::at`), so every pan or zoom
  frame repaints every entity. All entities sit in one
  `ReadSignal<Vec<CanvasEntity>>`, so each edit, and each frame of a drag,
  clones the list and every shape re-checks itself.
- `CanvasEntityKind` is a closed variant per tool. Lines have two ends and bind
  to nothing, text cannot sit in a shape, `style` is saved as one value (two
  peers changing colour and width at once lose one change), and freehand
  points live inside `kind`.
- The chrome is built for precise work: an accordion inspector with X/Y/W/H,
  and artboards among the main tools.
- On web, opening a canvas needs a server sign-in first.

The canvas is not a frozen format yet (guides/frozen_formats.md), so its model
can change freely until step 7.

## 1. Foundations in beui

- **A path shape.** Fill and stroke, curves, joins, caps and dashes, in
  `beui-renderer-wgpu` and `beui-renderer-dom`, with a painting in beui-demo.
- **Camera transforms in the renderer.** The `Canvas` node places and scales
  its items by a transform, so a pan or zoom changes only that transform, and
  an entity repaints only when it changes.
- **Per-entity signals.** A drag touches only the entities it moves.
- **Benchmarks** (guides/performance.md): pan and zoom frame time on a canvas
  of 10,000 shapes, and the latency from pen down to ink on screen.

## 2. Model v2

- Shapes: rectangle, ellipse, diamond, lines and arrows with any number of
  points, freehand, text, image, block embed, and frame (today's artboard).
- Arrow ends bind to shapes and follow them. A shape or an arrow can carry a
  text label.
- Style saved field by field, so concurrent changes merge.
- A hand-drawn look (like rough.js) with a fixed seed per shape, and a clean
  look.
- Freehand stored as points and pressures and drawn as a smoothed outline that
  follows pressure (like perfect-freehand), filled as a path. A mouse or finger
  simulates pressure.
- Geometry general enough for the vector profile's bezier paths, so that
  profile adds tools, not a second model.

## 3. The prototyping profile

- A small style popover by the selection: a few colour swatches, three stroke
  widths, fill none / hatched / solid, stroke style, roughness, opacity, and
  text sizes S / M / L / XL as design tokens. The current inspector moves to
  the vector profile.
- Hand-drawn by default. New shapes take the last style used.
- One-letter and number shortcuts for every tool, and a tool lock.
- Double-clicking empty canvas types text; Enter on a selected shape edits its
  label.
- An eraser.
- Snapping to other shapes' edges and centres, with guides and equal-spacing
  hints. Today only angles snap.
- Lasso selection, double-click into a group, drag a line's midpoints to bend
  it.
- Copy and export a selection or frame as PNG and SVG.
- A library of reusable shapes. **(ask)** whether a library is a block.

## 4. Input on every device

- Pen: palm rejection while a pen is down; in the pen tool, the pen draws and
  a finger selects or pans; the eraser end and barrel button.
- Web: read coalesced and predicted pointer events (`getCoalescedEvents`,
  `getPredictedEvents`) in `beui-adapter-web`, for smoother strokes and less
  latency.
- Touch: long press opens the context menu; check what one finger does in
  every tool.
- Trackpad: two-finger scroll pans and pinch zooms, on web too.
- Peers see a stroke while it is drawn, through presence, not only on release.

## 5. As quick to open as a URL

- Visiting the web app with no account opens a blank canvas at once, kept in a
  local-only workspace in the browser (IndexedDB; see `plans/public_release.md`,
  Web offline). Local-only workspaces are permanent, not a testing aid.
- Signing in later can move the canvas into a remote workspace and share it by
  link.
- Load only the canvas plugin to start, and cache the app so it opens offline
  and installs to a home screen.
- A cold-start target on a mid-range phone, measured in CI. **(ask)** the
  target.

## 6. The other profiles

Vector, then raster (with the pixel-art merge), then interactivity. Each gets
its own plan when it starts.

## 7. Freeze the format

Once steps 2 and 3 have settled, add the canvas to guides/frozen_formats.md
with its tests.
