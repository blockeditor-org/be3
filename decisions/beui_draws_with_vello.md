# beui draws with Vello

beui-renderer-wgpu draws with Vello's GPU renderer, `vello_gpu` (paths prepared on the CPU, drawn
with ordinary render passes, no compute shaders), and the paintings' rasterizer, beui-paint-raster,
with `vello_cpu`, through one encoding of each shape (`beui-vello`), so a painting and the window
paint the same pixels. beui keeps what decides what to draw: the retained display tree, damage by
identity, scroll copies, the glyph atlas, the layer list and the per-display lists of placed items
(vello_gpu 0.4 has no public recording). Vello is in the beui app library, exported, so an app can
draw with it too. The HTML core is unaffected.

## How beui's drawing maps onto it

- Rects, glyphs (an alpha-mask tint of the RGBA8 atlas), images, lines and paths are Vello fills and
  strokes; paintings hold paths as `Content::Path`. Seamed fills that follow each other alike are
  filled as one nonzero path, so their shared edges cancel.
- Opacity groups are Vello opacity layers. A group holding a `draw_gpu` drawing or a filter keeps an
  offscreen layer of its own (`group.rs`), since the scene is split at each drawing; it is composed
  through non-sRGB views, so it blends as a Vello layer does.
- An edge fade is an isolated layer with a destination-in gradient across each faded edge (mask
  layers are unimplemented in vello_gpu 0.4). A punch is a destination-out layer.
- Damaged regions are cleared to transparent by Vello and the window's background is the scene's
  first fill. A punch after a drawing erases nothing drawn before it in the frame.
- Vello writes sRGB-encoded values, so it draws into each target's non-sRGB view: every target
  lists it in `view_formats`, and be3:gpu's `surface-configuration` carries an app surface's.
  Blending is in sRGB everywhere, as the paintings rasterize it.
- The filter's blur is Vello's (decisions/beui_filter_blur_is_vellos_gaussian.md).
- A lost device stops the renderer before Vello is called (`Lost`); overlays keep wgpu's and
  vello_gpu's uploads from panicking on a device lost mid-frame.

## Why

Exact anti-aliased fills and strokes of curves, gradients, clip paths and blend modes for every
component, not only the canvas; a blur continuous in σ; apps that draw their own 2D get the renderer
beui uses; and our instance shader, tessellator, Kawase blur and software rasterizer went.

## Rejected

- **Vello only for the canvas editor**: the gains are beui's as a whole, and two renderers would draw
  one window two ways.
- **Vello's compute-shader renderer** (`vello`): Linebender is making it experimental, and it needs
  compute shaders weak GPUs and WebGL lack.
- **vello_cpu as the renderer**: every target beui draws on has a GPU (the PineNote's Mali-G52
  through panvk).
- **glifo for text**: beui keeps its own shaping, layout and atlas
  (decisions/fonts_are_read_shaped_and_drawn_in_rust.md); only drawing the atlas goes through Vello.
- **Unexporting Vello from the beui library** so the linker drops what beui does not call: apps may
  call Vello directly.
- **Vello's recordings for the per-display lists**: vello_gpu 0.4 has none public.
