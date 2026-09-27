# beui: retained layout and paint, what is left

The first step is done: each node keeps the shapes it painted as a display list
of its own shapes and references to its children (`crates/beui/src/paint.rs`),
and only nodes that changed, were laid out again, were handed a different rect
or painter, or asked for a repaint that fell due paint again. Setters say what
they invalidate (`Arena::get_mut_as` for layout, `paint_mut_as` for paint only,
`touch_mut_as` for neither), damage is where a node's shapes changed, and the
frame carries up to four damaged rects to the renderer, which scissors each one
(`Repaint::Region` holds a `Region`, and `FrameOutput::repaint` builds it), drawings
included. beui's own tests and `block-ui-test` repaint every frame from scratch
and check the retained painting and its damage against it (`beui::verify_paint`).
Layout writes the one rect map in place, stops invalidating at relayout
boundaries and lays those out directly, and removed nodes' slots are reused
under generational ids. A scroll keeps its items' lengths and prefix sums, told
which items went stale by the arena, so a scroll lays out only the visible
rows. A plugin frame reports the surface rectangles it drew into, and the host
damages only those through `Drawing::redrawn`. A scroll is a coordinate space of
its own (`Document::enter_space`): its rows keep their rects and display lists
in its content's coordinates, `Rects` resolves screen rects lazily through the
spaces' translations, and a scroll lays out and repaints only itself and the
rows it exposes. Nodes that read `painter.origin()` or `painter.clip_rect()`
are laid out and painted again when their place in the space changes.

What follows is the rest, roughly in order of value.

## Layout

- **Rects relative to the parent everywhere.** Only a scroll starts a
  coordinate space. A subtree that moves inside one - a row pushed down by the
  row above it growing, a dock panel resized beside it - still has every
  node's rect changed, so each is laid out and painted again. Store every
  node's rect relative to its parent's and let any container translate its
  children, not just a scroll. A pan-and-zoom `Canvas` is the next candidate
  for a space of its own: panning it lays out every visible item again.

## Paint and rendering

- **Stop flattening.** After a paint pass the display tree is flattened into a
  `Vec<Shape>` (`PaintCache::flatten`), copied into the `Context` every frame
  (`ctx.extend(&self.shapes)`) and compared shape by shape in `end_frame`. Give
  the renderer the display tree instead, keep an instance range per display
  list in the GPU buffer, and re-encode only the lists that changed. That also
  lets `FrameOutput` stop exposing shapes: `FrameOutput::shapes` is still
  public (the `pixel_ray_tracer` overlay tests read it) and `beui::quads` is
  used by `block-ui-test` to rasterise paintings in software.
- **Scroll by copying.** A scroll's rows now keep their shapes as it moves,
  but the whole viewport is still damaged and re-rasterised. An opaque scroll
  viewport could copy the retained frame by the scroll delta and repaint only
  the exposed strip.
