# beui on Vello

Depends on: none.

Replace what beui-renderer-wgpu draws with Vello's GPU renderer, `vello_gpu` (formerly
vello_hybrid: paths prepared on the CPU, drawn with ordinary render passes, no compute shaders).
beui keeps what decides what to draw: the retained display tree, damage by identity, scroll copies,
the glyph atlas and the layer list. Vello takes what draws it: shapes, paths, clips, opacity
layers, blur. Vello goes into the beui app library, exported, so an app can also draw with it
directly. The HTML core is unaffected.

## Why

- Exact anti-aliased fills and strokes of curves, gradients, clip paths, blend modes and COLRv1
  colour glyphs, for every component and editor, not only the canvas. Today paths are tessellated
  into triangles on the CPU (`beui-core/src/path.rs`) and shapes are distance-field quads in one
  shader (`ui.wgsl`).
- Vello's Gaussian blur halves the image until little blur remains and finishes with a kernel of
  at most 13 taps, keeping exact count of the variance each level adds
  (`vello_common/src/filter/gaussian_blur.rs`). So the blur equals the σ asked for at every radius,
  and costs about the same for a large radius as a small one. The dual-Kawase blur
  (decisions/beui_filter_blur_is_dual_kawase.md) snaps when its level count changes, which shows
  as discontinuities while the scale changes.
- Apps that draw their own 2D (charts, annotations) get the renderer beui uses.
- About 2,000–2,500 lines of ours go: the shape half of `ui.wgsl` and the renderer's instance
  encoding, `group.rs`, the tessellator, the dual-Kawase blur and beui-paint-format's `raster.rs`.

## What maps to what

- **Shapes.** Rounded and cornered rects, segments and turns become Vello fills and strokes;
  `Content::Mesh` paths become `BezPath` fills or strokes with their fill rule.
- **Glyphs.** Rectangles drawn with an image paint from beui's atlas, sampled at the lowest
  quality (nearest) on pixel-aligned rects, with a `Tint` of `TintMode::AlphaMask` in the text's
  colour, as glifo draws its own cached glyphs. `AlphaMask` reads alpha, so the coverage atlas
  becomes `Rgba8Unorm` with coverage in alpha (it is `R8Unorm` today, which samples alpha 1).
  Colour glyphs are drawn untinted. Text stays beui's: `vello_gpu`'s `text` feature and glifo are
  off.
- **Clips.** `push_clip_rect` (a fast path for a pixel-aligned rect) and `push_clip_path`, which
  intersect geometry and cost no texture. `push_clip_layer`, which renders into an intermediate
  texture, only where a clip must apply to a group as a whole.
- **Opacity groups** (`Group::Mask`) become `push_opacity_layer`; each is an intermediate texture,
  as `group.rs`'s offscreen layer is now. A group that holds a `draw_gpu` drawing cannot be one
  Vello layer, since the scene is split at the drawing: it keeps an offscreen texture of its own,
  which Vello and the drawing paint into and which is then composed at the group's opacity.
- **Blur** (`Group::Filter`) becomes a filter layer with Vello's Gaussian blur. A filter layer is
  padded by the blur's reach, so it includes shapes just outside the damage.
- **Edge fades** are an isolated layer holding what is faded, then, for each faded edge, a layer
  blended `Compose::DestIn` holding a linear gradient from transparent to opaque across the
  edge's width. vello_gpu 0.4's mask layers are unimplemented (`Scene::push_layer` panics on a
  mask).
- **Punches** are a layer blended `Compose::DestOut` holding the punch's shape, which erases by
  its coverage as today's punch pipeline does. A scene is composited over the target as a whole
  (`TargetInit::SrcOver`), so nothing in it can erase what the target already holds: damaged
  regions are cleared to transparent first (`TargetInit::Clear` with the damaged rects) and the
  window's background is the scene's first fill. Known limit: a punch painted after a `draw_gpu`
  drawing does not erase the drawing, since the scene is split there.
- **Damage.** Each frame's scene holds only the display items that cross the damage, clipped to
  it, rendered onto the retained target, cleared only within the damage. Scroll copies stay
  texture copies. vello_gpu 0.4 has no public recording API (`vello_common/src/record.rs` is its
  scheduler's), so the per-display lists beui retains today stay beui's: each holds a display's
  placed primitives, encoded once, and is written into the frame's scene when the display is
  damaged.
- **Colour.** Vello takes and writes sRGB-encoded values and never converts them, so it draws
  into the non-sRGB view of its target (`format.remove_srgb_suffix()`): every texture beui draws
  on lists that format in its `view_formats` (surface configurations, offscreen and Space
  targets, composite roots, filter scenes, and the surfaces be-wasm-gpu-host makes for apps).
  Blending is then in sRGB on every target, as the paintings rasterize it; on an sRGB target it
  was linear before.
- **`draw_gpu` drawings** run in paint order as now: the scene is split at each drawing, Vello
  renders the part before it, the drawing's `paint` runs in its own pass on the same target, then
  Vello renders the rest.
- **The Space layer** keeps one renderer per texture
  (decisions/beui_space_keeps_one_renderer_per_texture.md): one Vello renderer each.

Vello's features are the beui library's (`crates/be-sdk-library-beui/Cargo.toml`). External
textures apply only where the device has `wgpu::Features::EXTERNAL_TEXTURE`; apps get them once
`be3:gpu` carries them (`create_external_texture` in crates/be-wasm-gpu-guest is unimplemented),
which this plan leaves for later.

## Steps

1. **The renderer draws with Vello**, as mapped above, together with the RGBA8 atlas, and
   beui-paint-format's `raster.rs` is rebuilt on `vello_cpu` in a crate of its own, so the tests
   that compare the GPU with the rasterizer compare Vello with Vello. Paintings do not change,
   since they record beui's display list, not pixels. Delete `ui.wgsl`'s shape
   code, the instance encoding, and `group.rs` but the offscreen layer of a group that holds a
   drawing. The rasterizer stays out of the beui library: vello_cpu reaches only the tools, the
   tests and the apps that export pictures. Put the suite's `renderer` benchmark, compared on
   a machine with a GPU against the build before (guides/performance.md), in the pull request.
2. **Blur.** Draw `Group::Filter` with Vello's blur, delete `filter.rs` and `blur.wgsl`, and
   replace decisions/beui_filter_blur_is_dual_kawase.md with a decision for Vello's blur
   (continuous in σ, logarithmic in cost). Put the `renderer` benchmark's blur rows, compared as
   in the step before, in the pull request.
3. **Paths in paintings.** `Content::Mesh` becomes a path with its fill rule or stroke style,
   `Frame::tidy` and the rasterizer learn it. Bump the format's
   `VERSION` (guides/testing_a_gui.md, 4) and let verify rewrite the paintings. Delete the
   tessellator in `beui-core/src/path.rs`.
4. **Documents.** Update guides/beui/core.md (Retained paint and damage), guides/beui.md's crate
   map, guides/beui/components.md (Drawing with the gpu), guides/testing_a_gui.md and
   guides/canvas.md. Record the choice in decisions/ with
   what was rejected below, and delete this plan.

## Rejected

- **Vello only for the canvas editor**: the gains (curves, gradients, clips, the blur) are beui's
  as a whole, and two renderers would draw one window two ways.
- **Vello's compute-shader renderer** (`vello`): Linebender is making it experimental, and it needs
  compute shaders weak GPUs and WebGL lack.
- **vello_cpu as the renderer**: every target beui draws on has a GPU (the PineNote's Mali-G52
  through panvk).
- **glifo for text**: beui keeps its own shaping, layout and atlas
  (decisions/fonts_are_read_shaped_and_drawn_in_rust.md); only drawing the atlas goes through Vello.
- **Unexporting Vello from the beui library** so the linker drops what beui does not call: apps
  may call Vello directly.
