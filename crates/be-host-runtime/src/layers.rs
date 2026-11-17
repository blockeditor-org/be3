//! The host's layer list (plans/host_as_platform_adapter.md, The layer list), with the host's
//! own UI as its only producer: every app surface the host's frame shows becomes a `Layer`.
//! Natively a drawing lists its layers (`Draw::layers`) and the window's renderer composites
//! the window from them (beui_draw::compose): the host's UI is `Surface::Root(0)`, and one more
//! root above each anchored view (`Root(1)` above the first), textures of their own. A screen is still drawn in place in
//! the UI's paint order (`texture_quad` places it), blending over what lies beneath it; only an
//! anchored view is composited between the roots. A renderer that does not composite (Space's
//! panels) has the drawing draw anchored views in place too. On the web the frame's list is
//! applied as CSS to the canvases the page places (`web::canvases`, `web::overlay`), with the
//! host's canvas as `Root(0)` between `#apps` and `#layers`, showing a hole where it shows a
//! screen.

use be_lib_layers::{Affine, Layer, LayerId, LayerInput, Surface};
use beui::{Rect, native::DrawAt, pos2};

use super::presenter::{Painted, Quad};

/// A layer showing `pixels` of a surface holding `size` pixels, laid over `quad` (in the
/// window's points): `pixels` fills `quad`, and the clip keeps the layer to it.
pub(crate) fn placed(
    id: LayerId,
    surface: Surface,
    quad: Quad,
    pixels: Rect,
    size: (u32, u32),
) -> Option<Layer> {
    if !pixels.is_positive() || size.0 == 0 || size.1 == 0 || !quad.rect.is_positive() {
        return None;
    }
    let [first, second, _, fourth] = quad.corners();
    let across = (second - first) / pixels.width();
    let down = (fourth - first) / pixels.height();
    if across.length() <= f32::EPSILON || down.length() <= f32::EPSILON {
        return None;
    }
    let origin = first - across * pixels.min.x - down * pixels.min.y;
    Some(Layer {
        id,
        surface,
        size,
        transform: Affine::from_axes(across, down, origin),
        clip: pixels,
        corner_radius: quad.corner_radius.max(0.0),
        opacity: quad.opacity.clamp(0.0, 1.0),
        shown: true,
        input: LayerInput::default(),
    })
}

/// The layer of a screen's blit that shows `source` (a fraction of what the surface holds) over
/// `quad`, as `Blit::shown_at` placed it for a surface holding `painted`.
pub(crate) fn screen(
    id: LayerId,
    surface: Surface,
    (quad, source): (Quad, Rect),
    painted: Painted,
) -> Option<Layer> {
    let (width, height) = (painted.size.0 as f32, painted.size.1 as f32);
    let pixels = Rect::from_min_max(
        pos2(source.min.x * width, source.min.y * height),
        pos2(source.max.x * width, source.max.y * height),
    );
    placed(id, surface, quad, pixels, painted.size)
}

/// The texture quad that draws `layer` natively, in a pass at `at`: what it shows of its
/// surface's texture, and where, turned, faded and rounded as the layer says.
pub(crate) fn texture_quad(layer: &Layer, at: &DrawAt) -> beui::native::texture_quad::Quad {
    beui::native::texture_quad::Quad {
        clip: at.clip,
        srgb_encoded: true,
        ..beui::native::compose::quad(layer, at.pixels_per_point, [0.0; 2])
    }
}

/// Counts the pieces each drawing shows of each view, so a layer's id names which of them it is
/// (`id`), whatever blits come or go before it.
#[derive(Default)]
pub(crate) struct Pieces(be_lib_stable_hash::HashMap<(u32, u64, bool, u64), usize>);

impl Pieces {
    /// The index of the next piece `drawing` shows of the view `of` (a screen, or an anchored
    /// surface) of the runtime in `slot`.
    pub(crate) fn next(&mut self, slot: u32, of: u64, anchored: bool, drawing: u64) -> usize {
        let count = self.0.entry((slot, of, anchored, drawing)).or_default();
        *count += 1;
        *count - 1
    }
}

/// A layer's id, stable while the same drawing shows the same piece of the same view: its
/// runtime's slot, its screen or anchored surface, the drawing (a view's parts below and above
/// its children, or a preview shown in two places, are drawings of their own) and which of its
/// pieces it is.
pub(crate) fn id(slot: u32, of: u64, anchored: bool, drawing: u64, piece: usize) -> LayerId {
    LayerId(std::hash::BuildHasher::hash_one(
        &be_lib_stable_hash::State::default(),
        (slot, of, anchored, drawing, piece),
    ))
}

/// The root's own surface: the host's UI, over the whole window.
#[cfg(any(target_arch = "wasm32", test))]
fn root(viewport: beui::Vec2) -> Layer {
    Layer {
        id: LayerId(0),
        surface: Surface::Root(0),
        size: (
            viewport.x.ceil().max(1.0) as u32,
            viewport.y.ceil().max(1.0) as u32,
        ),
        transform: Affine::IDENTITY,
        clip: Rect::from_min_max(beui::Pos2::ZERO, pos2(viewport.x, viewport.y)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    }
}

/// The frame's layers for the web, from the blits its frame painted, in paint order: the
/// screens under the host's canvas, the host's canvas, then the anchored views and the screens
/// that lie over an anchored view painted before them (`presenter::lifted`), which the page
/// shows above it. The page has no textures, so a screen is placed at the size its worker last
/// painted it, and an anchored view's piece is kept to the view's bounds, which its canvas clips
/// to.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn frame(
    blits: &[(u64, super::presenter::Blit)],
    viewport: beui::Vec2,
) -> be_lib_layers::WindowLayers {
    let lifted = super::presenter::lifted(
        blits
            .iter()
            .map(|(_, blit)| (blit.quad.rect, blit.layer.is_some())),
    );
    let mut pieces = Pieces::default();
    let mut below = Vec::new();
    let mut above = Vec::new();
    for ((drawing, blit), lifted) in blits.iter().zip(lifted) {
        let drawing = *drawing;
        match blit.layer {
            Some(anchored) => {
                let of = u64::from(anchored.surface);
                let piece = pieces.next(blit.surface, of, true, drawing);
                let id = id(blit.surface, of, true, drawing, piece);
                above.extend(anchored_piece(id, blit, anchored));
            }
            None => {
                let shared = blit.shared.borrow();
                let Some(placement) = shared.placement(blit.screen).copied() else {
                    continue;
                };
                drop(shared);
                let index = pieces.next(blit.surface, blit.screen.0, false, drawing);
                let Some(layer) = blit.screen_layer(&placement, None, drawing, index) else {
                    continue;
                };
                match lifted {
                    true => above.push(layer),
                    false => below.push(layer),
                }
            }
        }
    }
    below.push(root(viewport));
    below.extend(above);
    be_lib_layers::WindowLayers {
        window: None,
        layers: below,
    }
}

/// An anchored view's piece as the web shows it: the part of its surface's `pixels` that falls
/// within the view's bounds.
#[cfg(any(target_arch = "wasm32", test))]
fn anchored_piece(
    id: LayerId,
    blit: &super::presenter::Blit,
    anchored: super::presenter::AnchoredSource,
) -> Option<Layer> {
    let target = blit.quad.rect;
    let shown = target.intersect(anchored.bounds);
    if !shown.is_positive() || !target.is_positive() || !anchored.pixels.is_positive() {
        return None;
    }
    let per = beui::vec2(
        anchored.pixels.width() / target.width(),
        anchored.pixels.height() / target.height(),
    );
    let at = |point: beui::Pos2| {
        pos2(
            anchored.pixels.min.x + (point.x - target.min.x) * per.x,
            anchored.pixels.min.y + (point.y - target.min.y) * per.y,
        )
    };
    let pixels = Rect::from_min_max(at(shown.min), at(shown.max));
    // The page knows only the pixels it is told to show, so `size` is a lower bound of what the
    // surface holds: enough for `rounded`, since an anchored layer has no rounding.
    let size = (
        anchored.pixels.max.x.ceil().max(1.0) as u32,
        anchored.pixels.max.y.ceil().max(1.0) as u32,
    );
    placed(
        id,
        Surface::Anchored {
            instance: blit.surface,
            surface: anchored.surface,
        },
        Quad::upright(shown),
        pixels,
        size,
    )
}

/// The CSS that places a screen's canvas for `layer`, at `scale` CSS pixels per point, `order`
/// its place among the canvases it stacks with. It never sizes the canvas: the element keeps its
/// natural size, which is the backing store its worker last showed, and the matrix maps one of
/// its pixels as the layer's transform does, with the layer's clip in its own pixels. So the
/// browser never scales a canvas into a box of another size, even when the worker shows a
/// texture of a new size before the host hears of it: until then that texture still shows at its
/// own size, from the same corner.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn canvas_style(layer: &Layer, scale: f32, order: usize) -> Option<String> {
    let transform = layer.transform;
    let (per_x, per_y) = (transform.across() * scale, transform.down() * scale);
    let origin = transform.origin();
    if per_x.length() <= f32::EPSILON || per_y.length() <= f32::EPSILON {
        return None;
    }
    let radius = layer.corner_radius * scale / per_x.length();
    let [top_left, top_right, bottom_right, bottom_left] = layer
        .rounded()
        .map(|rounded| if rounded { radius } else { 0.0 });
    let clip = layer.clip;
    Some(format!(
        "transform:matrix({},{},{},{},{},{});\
         clip-path:xywh({}px {}px {}px {}px round {}px {}px {}px {}px);opacity:{};z-index:{order}",
        per_x.x,
        per_x.y,
        per_y.x,
        per_y.y,
        origin.x * scale,
        origin.y * scale,
        clip.min.x,
        clip.min.y,
        clip.width(),
        clip.height(),
        top_left,
        top_right,
        bottom_right,
        bottom_left,
        layer.opacity,
    ))
}

#[cfg(test)]
mod tests;
