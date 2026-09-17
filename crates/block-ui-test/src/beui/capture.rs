use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use beui::{Color32, FrameOutput, GlyphImage, Quad, Vec2};
use paint_snapshot::{
    Content, Frame, Glyph, Primitive, RoundedRect, Snapshot, Texture, TextureKey,
};

pub(crate) fn capture(
    output: &FrameOutput,
    size: Vec2,
    pixels_per_point: f32,
    background: Color32,
) -> Result<Snapshot, String> {
    let points = |values: [f32; 4]| values.map(|value| value / pixels_per_point);
    let mut textures = BTreeMap::new();
    let mut primitives = Vec::new();
    for quad in beui::quads(output, pixels_per_point) {
        let (clip, content) = match quad {
            Quad::Rect {
                rect,
                clip,
                color,
                corner_radius,
                stroke_width,
            } => (
                clip,
                Content::RoundedRect(RoundedRect {
                    rect: points(rect),
                    corner_radius: corner_radius / pixels_per_point,
                    stroke_width: stroke_width / pixels_per_point,
                    color: color.to_array(),
                }),
            ),
            Quad::Glyph {
                rect,
                clip,
                color,
                glyph,
            } => (
                clip,
                Content::Glyph(Glyph {
                    rect: points(rect),
                    texture: texture(&mut textures, &glyph.image)?,
                    color: color.to_array(),
                }),
            ),
            Quad::Punch { rect, clip, .. } => (clip, Content::Callback(points(rect))),
        };
        primitives.push(Primitive {
            clip: points(clip),
            content,
        });
    }
    Ok(Snapshot::of(
        Frame {
            size: [
                (size.x * pixels_per_point).round().max(1.0) as u32,
                (size.y * pixels_per_point).round().max(1.0) as u32,
            ],
            pixels_per_point,
            background: background.to_array(),
            primitives,
        },
        textures,
    ))
}

fn texture(
    textures: &mut BTreeMap<TextureKey, Texture>,
    image: &GlyphImage,
) -> Result<TextureKey, String> {
    let size = [image.width, image.height];
    let pixels: Vec<[u8; 4]> = image.pixels.iter().map(|coverage| [*coverage; 4]).collect();
    let key = paint_snapshot::fingerprint(size, &pixels);
    if let Entry::Vacant(entry) = textures.entry(key) {
        entry.insert(Texture::encode(size, &pixels)?);
    }
    Ok(key)
}
