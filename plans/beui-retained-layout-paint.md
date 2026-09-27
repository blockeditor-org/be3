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
under generational ids.

What follows is the rest, roughly in order of value.

## Layout

- **Rects relative to the parent.** Rects are absolute, so scrolling or moving a
  subtree changes the rect of every visible node under it, which lays each out
  and paints each again. Store an offset per node and let containers (scroll,
  canvas, offset) carry a translation; derive absolute rects lazily for input,
  accessibility and test ids. The display lists then hold local coordinates and
  a scroll re-records one node.

## Paint and rendering

- **Stop flattening.** After a paint pass the display tree is flattened into a
  `Vec<Shape>` (`PaintCache::flatten`), copied into the `Context` every frame
  (`ctx.extend(&self.shapes)`) and compared shape by shape in `end_frame`. Give
  the renderer the display tree instead, keep an instance range per display
  list in the GPU buffer, and re-encode only the lists that changed. That also
  lets `FrameOutput` stop exposing shapes: `FrameOutput::shapes` is still
  public (the `pixel_ray_tracer` overlay tests read it) and `beui::quads` is
  used by `block-ui-test` to rasterise paintings in software.
- **Scroll by copying.** With local coordinates, an opaque scroll viewport can
  copy the retained frame by the scroll delta and repaint only the exposed
  strip.
