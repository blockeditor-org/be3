# beui: retained layout and paint, what is left

The first step is done: each node keeps the shapes it painted as a display list
of its own shapes and references to its children (`crates/beui/src/paint.rs`),
and only nodes that changed, were laid out again, were handed a different rect
or painter, or asked for a repaint that fell due paint again. Setters say what
they invalidate (`Arena::get_mut_as` for layout, `paint_mut_as` for paint only,
`touch_mut_as` for neither), damage is where a node's shapes changed, and the
frame carries up to four damaged rects to the renderer, which scissors each one
(`Repaint::Region` holds a `Region`, and `FrameOutput::repaint` builds it). beui's
own tests repaint every frame from scratch and check the retained painting and
its damage against it (`Document::verify_paint`).

What follows is the rest, roughly in order of value.

## Layout

- **Lay out from dirty nodes, not the root.** `update_layout` still walks down
  from the root, stopping at nodes whose placement is reusable. That is
  proportional to the depth times the fan-out of the dirty paths. Add relayout
  boundaries - nodes whose size cannot depend on their children (a `Frame` with
  a fixed width and height, a `Canvas`, overlay content) - stop `mark_stale` at
  them, and lay each dirty boundary out directly from the rect and clip it was
  last placed with. `layout_parent`, `placing` and `placed_children` need
  seeding for a walk that does not start at the root.
- **Rects relative to the parent.** Rects are absolute, so scrolling or moving a
  subtree changes the rect of every visible node under it, which lays each out
  and paints each again. Store an offset per node and let containers (scroll,
  canvas, offset) carry a translation; derive absolute rects lazily for input,
  accessibility and test ids. The display lists then hold local coordinates and
  a scroll re-records one node. `OffsetNode::lengths` also sums every item on
  every pass; keep prefix sums so a scroll costs the visible rows.

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
- **Damage across the plugin boundary.** `block-editor-beui` prepares its
  renderer with `Repaint::Everything` every time, so a plugin pane repaints all
  of itself for a caret blink. The plugin protocol could carry damaged rects
  (plain rectangles keep it framework-independent) and the host could scissor
  its blit to them.

## Other per-frame work found along the way

- `interact_node` visits every placed node on every input event, pointer motion
  included. Hit-test through retained bounds, and deliver keyboard input along
  the focus path.
