//! A painting rasterized with vello_cpu, through the same encoding of each shape
//! (`beui_vello`) beui-renderer-wgpu draws with vello_gpu, so the two paint the same pixels.

use std::collections::BTreeMap;
use std::sync::Arc;

use beui_paint_format::{
    Content, Fade, Frame, Glyph, Pixels, Primitive, Round, RoundedRect, TextureKey, Triangle,
    Turn,
};
use beui_vello::vello_common::kurbo::{Affine, BezPath, Rect, Shape, Stroke};
use beui_vello::vello_common::paint::{ImageSource, PaintType, Tint};
use beui_vello::vello_common::peniko::{BlendMode, Fill, ImageAlphaType};
use beui_vello::vello_common::pixmap::{PixelMetadata, Pixmap};
use beui_vello::{Canvas, TOLERANCE};
use vello_cpu::{RasterizerSettings, RenderContext, RenderMode, Resources};

mod mesh;
mod snapshot;
pub use snapshot::render;

/// `frame` painted over `background` (straight sRGB), its textures given by `textures`.
pub fn rasterize<'a>(
    frame: &Frame,
    background: [u8; 4],
    textures: impl Fn(TextureKey) -> Option<&'a Pixels>,
) -> Result<Pixels, String> {
    let [width, height] = frame.size.map(|side| side.clamp(1, u32::from(u16::MAX)));
    let (width, height) = (width as u16, height as u16);
    let mut context = RenderContext::new(width, height);
    let mut painter = Painter {
        textures: &textures,
        pixmaps: BTreeMap::new(),
        chains: BTreeMap::new(),
        scale: frame.pixels_per_point,
    };
    painter.paint(&mut Cpu(&mut context), &frame.primitives)?;
    context.flush();
    let mut target = Pixmap::new(width, height);
    context.render_with(
        &mut target,
        &mut Resources::new(),
        RasterizerSettings {
            render_mode: RenderMode::OptimizeQuality,
            target_init: beui_vello::vello_common::TargetInit::Clear(beui_vello::color(
                background,
            )),
            ..RasterizerSettings::default()
        },
    );
    let rgba = target.take_rgba8(ImageAlphaType::Alpha);
    Ok(Pixels {
        size: [u32::from(width), u32::from(height)],
        rgba: rgba
            .chunks_exact(4)
            .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
            .collect(),
    })
}

/// vello_cpu's context as the canvas beui's shapes are drawn on.
struct Cpu<'a>(&'a mut RenderContext);

impl Canvas for Cpu<'_> {
    fn set_transform(&mut self, transform: Affine) {
        self.0.set_transform(transform);
    }
    fn set_paint(&mut self, paint: PaintType) {
        self.0.set_paint(paint);
    }
    fn set_paint_transform(&mut self, transform: Affine) {
        self.0.set_paint_transform(transform);
    }
    fn set_tint(&mut self, tint: Option<Tint>) {
        self.0.set_tint(tint);
    }
    fn set_fill_rule(&mut self, rule: Fill) {
        self.0.set_fill_rule(rule);
    }
    fn set_stroke(&mut self, stroke: Stroke) {
        self.0.set_stroke(stroke);
    }
    fn fill_path(&mut self, path: &BezPath) {
        self.0.fill_path(path);
    }
    fn stroke_path(&mut self, path: &BezPath) {
        self.0.stroke_path(path);
    }
    fn fill_rect(&mut self, rect: &Rect) {
        self.0.fill_rect(rect);
    }
    fn push_clip_rect(&mut self, rect: &Rect) {
        self.0.push_clip_rect(rect);
    }
    fn push_clip_path(&mut self, path: &BezPath) {
        self.0.push_clip_path(path);
    }
    fn pop_clip(&mut self) {
        self.0.pop_clip();
    }
    fn push_layer(&mut self, blend: Option<BlendMode>, opacity: Option<f32>) {
        self.0.push_layer(None, blend, opacity, None, None);
    }
    fn pop_layer(&mut self) {
        self.0.pop_layer();
    }
}

struct Painter<'a, 't> {
    textures: &'t dyn Fn(TextureKey) -> Option<&'a Pixels>,
    /// Each texture as a premultiplied pixmap, made as it is used.
    pixmaps: BTreeMap<TextureKey, Arc<Pixmap>>,
    /// Each texture's mip chain below it, for the meshes (`mesh`).
    chains: BTreeMap<TextureKey, Vec<Pixels>>,
    scale: f32,
}

impl Painter<'_, '_> {
    fn texture(&self, key: TextureKey) -> Result<&Pixels, String> {
        (self.textures)(key).ok_or_else(|| "the painting is missing a texture it uses".to_owned())
    }

    /// `key`'s texture as an image, and its size.
    fn image(&mut self, key: TextureKey) -> Result<(ImageSource, [f32; 2]), String> {
        if !self.pixmaps.contains_key(&key) {
            let made = Arc::new(pixmap(self.texture(key)?));
            self.pixmaps.insert(key, made);
        }
        let pixmap = self.pixmaps[&key].clone();
        let size = [f32::from(pixmap.width()), f32::from(pixmap.height())];
        Ok((ImageSource::Pixmap(pixmap), size))
    }

    fn paint(&mut self, canvas: &mut Cpu<'_>, primitives: &[Primitive]) -> Result<(), String> {
        // Primitives under one fade are faded together, as the renderer fades the space that
        // holds them.
        let mut start = 0;
        while start < primitives.len() {
            let fade = primitives[start].fade;
            let end = primitives[start..]
                .iter()
                .position(|primitive| primitive.fade != fade)
                .map_or(primitives.len(), |run| start + run);
            let faded = fade_of(fade, self.scale);
            if faded.is_some() {
                beui_vello::begin_fade(canvas);
            }
            for primitive in &primitives[start..end] {
                self.primitive(canvas, primitive)?;
            }
            if let Some(faded) = faded {
                let bounds = [0.0, 0.0, f32::from(u16::MAX), f32::from(u16::MAX)];
                beui_vello::end_fade(canvas, faded, bounds);
            }
            start = end;
        }
        Ok(())
    }

    fn primitive(&mut self, canvas: &mut Cpu<'_>, primitive: &Primitive) -> Result<(), String> {
        let scale = self.scale;
        let clip = primitive.clip.map(|value| (value * scale).round());
        if clip[0] >= clip[2] || clip[1] >= clip[3] {
            return Ok(());
        }
        canvas.set_transform(Affine::IDENTITY);
        canvas.push_clip_rect(&beui_vello::rect(clip));
        for round in &primitive.rounds {
            push_round(canvas, round.scaled(scale));
        }
        let painted = self.content(canvas, &primitive.content, clip);
        for _ in &primitive.rounds {
            canvas.pop_clip();
        }
        canvas.pop_clip();
        painted
    }

    fn content(
        &mut self,
        canvas: &mut Cpu<'_>,
        content: &Content,
        clip: [f32; 4],
    ) -> Result<(), String> {
        let scale = self.scale;
        match content {
            Content::RoundedRect(shape) => rounded_rect(canvas, shape, [shape.corner_radius; 4], scale),
            Content::CorneredRect(shape, corners) => rounded_rect(canvas, shape, *corners, scale),
            Content::Glyph(glyph) => self.glyph(canvas, glyph)?,
            Content::Mesh(triangles) => self.mesh(canvas, triangles, clip)?,
            Content::Callback(rect) => beui_vello::filled_rect(
                canvas,
                rect.map(|value| (value * scale).round()),
                [0.0; 4],
                1.0,
                [255, 0, 255, 255],
                Affine::IDENTITY,
            ),
            Content::Group(group) => {
                canvas.push_layer(None, Some(group.opacity.clamp(0.0, 1.0)));
                let painted = self.paint(canvas, &group.primitives);
                canvas.pop_layer();
                painted?;
            }
        }
        Ok(())
    }

    fn glyph(&mut self, canvas: &mut Cpu<'_>, glyph: &Glyph) -> Result<(), String> {
        let bounds = glyph.rect.map(|value| (value * self.scale).round());
        let (source, texels) = self.image(glyph.texture)?;
        beui_vello::glyph(
            canvas,
            bounds,
            source,
            texels,
            false,
            glyph.color,
            turned(glyph.turn.scaled(self.scale)),
        );
        Ok(())
    }

    /// A mesh, rasterized as the GPU draws triangles (`mesh`), drawn as an image of the
    /// pixels it covers.
    fn mesh(
        &mut self,
        canvas: &mut Cpu<'_>,
        triangles: &[Triangle],
        clip: [f32; 4],
    ) -> Result<(), String> {
        let Some((pixmap, [left, top])) =
            mesh::mesh(triangles, clip, self.scale, self.textures, &mut self.chains)?
        else {
            return Ok(());
        };
        let size = [f32::from(pixmap.width()), f32::from(pixmap.height())];
        beui_vello::image(
            canvas,
            [left, top, left + size[0], top + size[1]],
            ImageSource::Pixmap(Arc::new(pixmap)),
            size,
            [255; 4],
            0.0,
            false,
            Affine::IDENTITY,
        );
        Ok(())
    }
}

fn rounded_rect(canvas: &mut Cpu<'_>, shape: &RoundedRect, corners: [f32; 4], scale: f32) {
    beui_vello::filled_rect(
        canvas,
        shape.rect.map(|value| value * scale),
        corners.map(|radius| radius * scale),
        shape.stroke_width * scale,
        shape.color,
        turned(shape.turn.scaled(scale)),
    );
}

fn push_round(canvas: &mut Cpu<'_>, round: Round) {
    canvas.set_transform(turned(round.turn));
    canvas.push_clip_path(
        &beui_vello::rounded(beui_vello::rect(round.rect), [round.radius; 4]).to_path(TOLERANCE),
    );
    canvas.set_transform(Affine::IDENTITY);
}

fn fade_of(fade: Fade, scale: f32) -> Option<beui_vello::Fade> {
    let fade = fade.scaled(scale);
    let faded = beui_vello::Fade {
        rect: fade.rect,
        widths: fade.widths,
    };
    faded.fades().then_some(faded)
}

fn turned(turn: Turn) -> Affine {
    let (sin, cos) = turn.angle.sin_cos();
    beui_vello::turn(turn.pivot, cos, sin)
}

/// A texture's straight-alpha texels as a premultiplied pixmap.
fn pixmap(pixels: &Pixels) -> Pixmap {
    let [width, height] = pixels.size.map(|side| side.clamp(1, u32::from(u16::MAX)) as u16);
    let mut data: Vec<u8> = pixels.rgba.iter().flatten().copied().collect();
    data.resize(usize::from(width) * usize::from(height) * 4, 0);
    Pixmap::from_parts(
        data,
        width,
        height,
        PixelMetadata {
            may_have_transparency: true,
            alpha_type: ImageAlphaType::Alpha,
        },
    )
}

#[cfg(test)]
mod tests;
