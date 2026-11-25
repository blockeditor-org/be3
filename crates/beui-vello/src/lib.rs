//! beui's shapes drawn with Vello. Each shape has one encoding here, which beui-renderer-wgpu
//! writes into a vello_gpu scene and beui-paint-raster into a vello_cpu context, so the GPU and
//! the rasterizer paint the same pixels. Every coordinate is a pixel of the target.

pub use vello_common;
use vello_common::kurbo::{
    Affine, BezPath, Cap, Circle, Join, Line, Point, Rect, RoundedRect, RoundedRectRadii, Shape,
    Stroke,
};
use vello_common::paint::{Image, ImageSource, PaintType, Tint, TintMode};
use vello_common::peniko::{
    BlendMode, Color, Compose, Extend, Fill, Gradient, ImageQuality, ImageSampler, Mix,
};

/// How closely curves are flattened, in pixels.
pub const TOLERANCE: f64 = 0.1;

/// What a shape is drawn onto: the calls vello_gpu's `Scene` and vello_cpu's `RenderContext`
/// share, which each caller forwards from a wrapper of its own.
pub trait Canvas {
    fn set_transform(&mut self, transform: Affine);
    fn set_paint(&mut self, paint: PaintType);
    fn set_paint_transform(&mut self, transform: Affine);
    fn set_tint(&mut self, tint: Option<Tint>);
    fn set_fill_rule(&mut self, rule: Fill);
    fn set_stroke(&mut self, stroke: Stroke);
    fn fill_path(&mut self, path: &BezPath);
    fn stroke_path(&mut self, path: &BezPath);
    fn fill_rect(&mut self, rect: &Rect);
    fn push_clip_rect(&mut self, rect: &Rect);
    fn push_clip_path(&mut self, path: &BezPath);
    fn pop_clip(&mut self);
    fn push_layer(&mut self, blend: Option<BlendMode>, opacity: Option<f32>);
    fn pop_layer(&mut self);
}

/// A straight-alpha sRGB colour, as beui's `Color32` holds it.
pub fn color([red, green, blue, alpha]: [u8; 4]) -> Color {
    Color::from_rgba8(red, green, blue, alpha)
}

/// The rotation about `pivot` by the angle whose cosine and sine are given.
pub fn turn(pivot: [f32; 2], cos: f32, sin: f32) -> Affine {
    if sin == 0.0 && cos == 1.0 {
        return Affine::IDENTITY;
    }
    Affine::rotate_about(
        f64::from(sin).atan2(f64::from(cos)),
        Point::new(f64::from(pivot[0]), f64::from(pivot[1])),
    )
}

pub fn rect([left, top, right, bottom]: [f32; 4]) -> Rect {
    Rect::new(
        f64::from(left),
        f64::from(top),
        f64::from(right),
        f64::from(bottom),
    )
}

fn plain(canvas: &mut impl Canvas, transform: Affine, paint: [u8; 4]) {
    canvas.set_transform(transform);
    canvas.set_paint(color(paint).into());
    canvas.set_tint(None);
    canvas.set_fill_rule(Fill::NonZero);
}

/// The rounded rect `bounds` with each corner's radius (top left, top right, bottom right,
/// bottom left) no larger than half its shorter side.
pub fn rounded(bounds: Rect, radii: [f32; 4]) -> RoundedRect {
    let limit = bounds.width().min(bounds.height()).max(0.0) / 2.0;
    let [top_left, top_right, bottom_right, bottom_left] =
        radii.map(|radius| f64::from(radius).clamp(0.0, limit));
    RoundedRect::from_rect(
        bounds,
        RoundedRectRadii::new(top_left, top_right, bottom_right, bottom_left),
    )
}

/// A rect filled, or outlined `stroke_width` wide inside its edge, with rounded corners.
pub fn filled_rect(
    canvas: &mut impl Canvas,
    bounds: [f32; 4],
    radii: [f32; 4],
    stroke_width: f32,
    paint: [u8; 4],
    transform: Affine,
) {
    let bounds = rect(bounds);
    plain(canvas, transform, paint);
    let width = f64::from(stroke_width);
    if width <= 0.0 || 2.0 * width >= bounds.width().min(bounds.height()) {
        match radii.iter().all(|radius| *radius <= 0.0) {
            true => canvas.fill_rect(&bounds),
            false => canvas.fill_path(&rounded(bounds, radii).to_path(TOLERANCE)),
        }
        return;
    }
    let inset = bounds.inset(-width / 2.0);
    let radii = radii.map(|radius| (radius - stroke_width / 2.0).max(0.0));
    canvas.set_stroke(Stroke::new(width).with_join(Join::Miter));
    canvas.stroke_path(&rounded(inset, radii).to_path(TOLERANCE));
}

/// A segment `width` wide with round caps.
pub fn segment(canvas: &mut impl Canvas, [x0, y0, x1, y1]: [f32; 4], width: f32, paint: [u8; 4]) {
    plain(canvas, Affine::IDENTITY, paint);
    let (from, to) = (
        Point::new(f64::from(x0), f64::from(y0)),
        Point::new(f64::from(x1), f64::from(y1)),
    );
    let width = f64::from(width);
    if from == to {
        canvas.fill_path(&Circle::new(from, width / 2.0).to_path(TOLERANCE));
        return;
    }
    canvas.set_stroke(Stroke::new(width).with_caps(Cap::Round));
    canvas.stroke_path(&Line::new(from, to).to_path(TOLERANCE));
}

/// A path filled with `rule`.
pub fn filled_path(
    canvas: &mut impl Canvas,
    path: &BezPath,
    rule: Fill,
    paint: [u8; 4],
    transform: Affine,
) {
    plain(canvas, transform, paint);
    canvas.set_fill_rule(rule);
    canvas.fill_path(path);
}

/// A path stroked with `stroke`.
pub fn stroked_path(
    canvas: &mut impl Canvas,
    path: &BezPath,
    stroke: Stroke,
    paint: [u8; 4],
    transform: Affine,
) {
    plain(canvas, transform, paint);
    canvas.set_stroke(stroke);
    canvas.stroke_path(path);
}

/// How a texture is sampled: nearest where each texel lands on one pixel, smooth otherwise.
pub fn quality(smooth: bool) -> ImageQuality {
    match smooth {
        true => ImageQuality::Medium,
        false => ImageQuality::Low,
    }
}

/// The paint that lays `texels` (the size of `source`'s region) over `bounds`.
fn laid(
    canvas: &mut impl Canvas,
    source: ImageSource,
    texels: [f32; 2],
    bounds: Rect,
    quality: ImageQuality,
    alpha: f32,
) {
    canvas.set_paint(PaintType::Image(Image {
        image: source,
        sampler: ImageSampler {
            x_extend: Extend::Pad,
            y_extend: Extend::Pad,
            quality,
            alpha,
        },
    }));
    canvas.set_paint_transform(
        Affine::translate((bounds.x0, bounds.y0))
            * Affine::scale_non_uniform(
                bounds.width() / f64::from(texels[0].max(1.0)),
                bounds.height() / f64::from(texels[1].max(1.0)),
            ),
    );
}

/// A glyph: its texels laid over `bounds`. A coverage glyph is tinted in `paint`, reading the
/// texels' alpha; a colour glyph keeps its own colours, at `paint`'s alpha.
pub fn glyph(
    canvas: &mut impl Canvas,
    bounds: [f32; 4],
    source: ImageSource,
    texels: [f32; 2],
    colored: bool,
    paint: [u8; 4],
    transform: Affine,
) {
    let bounds = rect(bounds);
    canvas.set_transform(transform);
    canvas.set_fill_rule(Fill::NonZero);
    let exact = (bounds.width() - f64::from(texels[0])).abs() < 1.0e-3
        && (bounds.height() - f64::from(texels[1])).abs() < 1.0e-3
        && transform == Affine::IDENTITY;
    let alpha = match colored {
        true => f32::from(paint[3]) / 255.0,
        false => 1.0,
    };
    laid(canvas, source, texels, bounds, quality(!exact), alpha);
    canvas.set_tint((!colored).then(|| Tint {
        color: color(paint),
        mode: TintMode::AlphaMask,
    }));
    canvas.fill_rect(&bounds);
    canvas.set_tint(None);
}

/// An image: `source`'s region of `texels` laid over `bounds`, its corners rounded by `radius`,
/// each texel multiplied by `tint`.
#[allow(clippy::too_many_arguments)]
pub fn image(
    canvas: &mut impl Canvas,
    bounds: [f32; 4],
    source: ImageSource,
    texels: [f32; 2],
    tint: [u8; 4],
    radius: f32,
    smooth: bool,
    transform: Affine,
) {
    let bounds = rect(bounds);
    canvas.set_transform(transform);
    canvas.set_fill_rule(Fill::NonZero);
    laid(canvas, source, texels, bounds, quality(smooth), 1.0);
    canvas.set_tint((tint != [255; 4]).then(|| Tint {
        color: color(tint),
        mode: TintMode::Multiply,
    }));
    match radius > 0.0 {
        true => canvas.fill_path(&rounded(bounds, [radius; 4]).to_path(TOLERANCE)),
        false => canvas.fill_rect(&bounds),
    }
    canvas.set_tint(None);
}

/// Erases what lies under the rounded rect, by its coverage, at `strength`: a layer blended
/// destination-out.
pub fn punch(
    canvas: &mut impl Canvas,
    bounds: [f32; 4],
    radius: f32,
    strength: f32,
    transform: Affine,
) {
    canvas.push_layer(
        Some(BlendMode::new(Mix::Normal, Compose::DestOut)),
        (strength < 1.0).then_some(strength),
    );
    plain(canvas, transform, [0, 0, 0, 255]);
    match radius > 0.0 {
        true => canvas.fill_path(&rounded(rect(bounds), [radius; 4]).to_path(TOLERANCE)),
        false => canvas.fill_rect(&rect(bounds)),
    }
    canvas.pop_layer();
}

/// Where a fade ramps what it holds from transparent to opaque: across `widths` (left, top,
/// right, bottom) inward from each edge of `rect`. A zero width does not fade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    pub rect: [f32; 4],
    pub widths: [f32; 4],
}

impl Fade {
    pub fn fades(&self) -> bool {
        self.widths.iter().any(|width| *width > 0.0)
    }
}

/// Begins what `end_fade` fades: an isolated layer.
pub fn begin_fade(canvas: &mut impl Canvas) {
    canvas.push_layer(None, None);
}

/// Ends the layer `begin_fade` began, fading it by `fade` within `bounds`: for each faded edge,
/// a layer blended destination-in holding the edge's ramp.
pub fn end_fade(canvas: &mut impl Canvas, fade: Fade, bounds: [f32; 4]) {
    let [left, top, right, bottom] = fade.rect.map(f64::from);
    let [near_x, near_y, far_x, far_y] = fade.widths.map(f64::from);
    let ramps = [
        (near_x, (left, 0.0), (left + near_x, 0.0)),
        (near_y, (0.0, top), (0.0, top + near_y)),
        (far_x, (right, 0.0), (right - far_x, 0.0)),
        (far_y, (0.0, bottom), (0.0, bottom - far_y)),
    ];
    let clear = Color::from_rgba8(0, 0, 0, 0);
    let opaque = Color::from_rgba8(0, 0, 0, 255);
    for (width, from, to) in ramps {
        if width <= 0.0 {
            continue;
        }
        canvas.push_layer(Some(BlendMode::new(Mix::Normal, Compose::DestIn)), None);
        canvas.set_transform(Affine::IDENTITY);
        canvas.set_tint(None);
        canvas.set_paint(PaintType::Gradient(
            Gradient::new_linear(from, to).with_stops([clear, opaque]),
        ));
        canvas.fill_rect(&rect(bounds));
        canvas.pop_layer();
    }
    canvas.pop_layer();
}

/// Which level of a texture's mip chain (each level half the one before, level 0 the texture)
/// draws `texels` onto `pixels`: the largest whose texels are still no fewer than the pixels.
pub fn mip_level(texels: [f32; 2], pixels: [f32; 2]) -> u32 {
    let spread = |axis: usize| texels[axis] / pixels[axis].abs().max(1.0e-3);
    let spread = spread(0).max(spread(1));
    match spread > 1.0 && spread.is_finite() {
        true => spread.log2().floor() as u32,
        false => 0,
    }
}
