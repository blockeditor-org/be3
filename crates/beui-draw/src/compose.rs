//! Composited windows (plans/host_as_platform_adapter.md, The layer list): a program that turns
//! it on (`composite_windows`) has each window's UI rendered into textures of its own, the root's
//! surfaces, and the window drawn from its layer list in a pass of its own, with the surfaces
//! its drawings show (`Draw::layers`) as layers between them. The renderer (beui-renderer-wgpu)
//! does both.

use std::cell::Cell;

use be_lib_layers::Layer;

use crate::texture_quad::Bound;

/// A surface a drawing shows as a layer of its window, with the texture that holds it.
#[derive(Clone, Debug)]
pub struct Placed {
    /// In the points of the drawing's target (its pixels over `DrawAt::pixels_per_point`).
    pub layer: Layer,
    pub texture: Bound,
    /// Whether the texture has no alpha, so it shows as opaque.
    pub opaque: bool,
    /// Whether the texture holds sRGB-encoded values in a linear format.
    pub srgb_encoded: bool,
}

thread_local! {
    static COMPOSITING: Cell<bool> = const { Cell::new(false) };
}

/// Composites every window this thread renders from now on.
pub fn composite_windows() {
    COMPOSITING.set(true);
}

/// Whether windows this thread renders are composited.
pub fn compositing() -> bool {
    COMPOSITING.get()
}

/// The texture quad that draws `layer` (in the window's points) into a target at
/// `pixels_per_point`, whose top left is the window's `origin` (in the target's pixels): what it
/// shows of its surface, where, turned, faded and rounded as the layer says. Its clip is the
/// whole target.
pub fn quad(layer: &Layer, pixels_per_point: f32, origin: [f32; 2]) -> crate::texture_quad::Quad {
    let transform = layer.transform;
    let (across, down) = (
        transform.across() * pixels_per_point,
        transform.down() * pixels_per_point,
    );
    // Upright and unflipped only: a turn of exactly half a circle has negative axes and goes
    // through the other arm as a rotation, so the scale is always positive and the rect never
    // has a negative size.
    let (scale, rotation) = match across.y == 0.0 && down.x == 0.0 && across.x > 0.0 && down.y > 0.0
    {
        true => ((across.x, down.y), 0.0),
        false => ((across.length(), down.length()), across.y.atan2(across.x)),
    };
    let clip = layer.clip;
    let center = transform.apply(clip.center());
    let center = [
        center.x * pixels_per_point - origin[0],
        center.y * pixels_per_point - origin[1],
    ];
    let half = [
        clip.width() * scale.0 * 0.5,
        clip.height() * scale.1 * 0.5,
    ];
    let (width, height) = (layer.size.0 as f32, layer.size.1 as f32);
    let radius = layer.corner_radius.max(0.0) * pixels_per_point;
    crate::texture_quad::Quad {
        rect: [
            center[0] - half[0],
            center[1] - half[1],
            center[0] + half[0],
            center[1] + half[1],
        ],
        uv: [
            clip.min.x / width,
            clip.min.y / height,
            clip.max.x / width,
            clip.max.y / height,
        ],
        rotation,
        opacity: layer.opacity.clamp(0.0, 1.0),
        corner_radii: layer
            .rounded()
            .map(|rounded| if rounded { radius } else { 0.0 }),
        ..crate::texture_quad::Quad::default()
    }
}

/// The layer that shows what `quad` draws of a texture of `size` pixels, in a target at
/// `pixels_per_point`: the inverse of `quad`, for a drawing that places a surface it would
/// otherwise draw as a texture quad. An upright quad's clip is kept to its surface's pixels; a
/// turned one is shown whole.
pub fn layer_of(
    quad: &crate::texture_quad::Quad,
    size: (u32, u32),
    pixels_per_point: f32,
    id: be_lib_layers::LayerId,
    surface: be_lib_layers::Surface,
) -> Option<Layer> {
    use be_lib_layers::{Affine, LayerInput};
    use beui_types::{Rect, pos2, vec2};

    let [left, top, right, bottom] = quad.rect;
    let [u0, v0, u1, v1] = quad.uv;
    let (width, height) = (size.0 as f32, size.1 as f32);
    let shown = (u1 - u0) * width;
    let shown_down = (v1 - v0) * height;
    if right <= left || bottom <= top || shown <= 0.0 || shown_down <= 0.0 {
        return None;
    }
    let per = pixels_per_point.max(f32::EPSILON).recip();
    let (sin, cos) = quad.rotation.sin_cos();
    let turn = |x: f32, y: f32| vec2(x * cos - y * sin, x * sin + y * cos);
    let across = turn((right - left) / shown, 0.0);
    let down = turn(0.0, (bottom - top) / shown_down);
    let center = pos2((left + right) * 0.5, (top + bottom) * 0.5);
    // The texture's pixel at the quad's top left, then back to the texture's origin.
    let corner = center + turn(-(right - left) * 0.5, -(bottom - top) * 0.5);
    let origin = corner - across * (u0 * width) - down * (v0 * height);
    let transform = Affine::from_axes(across * per, down * per, pos2(origin.x * per, origin.y * per));
    let mut clip = Rect::from_min_max(pos2(u0 * width, v0 * height), pos2(u1 * width, v1 * height));
    if quad.rotation == 0.0 {
        let [x0, y0, x1, y1] = quad.clip;
        let to_pixels = |x: f32, y: f32| {
            pos2(
                u0 * width + (x - left) / (right - left) * shown,
                v0 * height + (y - top) / (bottom - top) * shown_down,
            )
        };
        let kept = Rect::from_min_max(to_pixels(x0.max(-1.0e9), y0.max(-1.0e9)), to_pixels(x1.min(1.0e9), y1.min(1.0e9)));
        clip = clip.intersect(kept);
    }
    clip.is_positive().then(|| Layer {
        id,
        surface,
        size,
        transform,
        clip,
        corner_radius: 0.0,
        opacity: quad.opacity.clamp(0.0, 1.0),
        shown: true,
        input: LayerInput::default(),
    })
}
