use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use beui::{Color32, FrameOutput, GlyphImage, Image, Quad, Vec2};
use paint_snapshot::{
    Content, Frame, Glyph, Primitive, RoundedRect, Snapshot, Texture, TextureKey, Triangle, Vertex,
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
    for quad in beui::quads(output, pixels_per_point).list {
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
            Quad::Image {
                rect,
                clip,
                source,
                image,
                tint,
                ..
            } => (
                clip,
                Content::Mesh(mesh(
                    points(rect),
                    source,
                    picture(&mut textures, &image)?,
                    tint.to_array(),
                )),
            ),
            Quad::Line {
                clip,
                segment,
                width,
                color,
                ..
            } => (
                clip,
                Content::Mesh(band(
                    points(segment),
                    width / pixels_per_point,
                    white(&mut textures)?,
                    color.to_array(),
                )),
            ),
            Quad::Punch { rect, clip, .. } | Quad::Drawing { rect, clip, .. } => {
                (clip, Content::Callback(points(rect)))
            }
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

fn mesh(rect: [f32; 4], source: [f32; 4], texture: TextureKey, color: [u8; 4]) -> Vec<Triangle> {
    let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: [x, y],
        uv: [u, v],
        color,
    };
    let [left, top, right, bottom] = rect;
    let [u0, v0, u1, v1] = source;
    let top_left = corner(left, top, u0, v0);
    let top_right = corner(right, top, u1, v0);
    let bottom_right = corner(right, bottom, u1, v1);
    let bottom_left = corner(left, bottom, u0, v1);
    vec![
        Triangle {
            texture,
            corners: [top_left, top_right, bottom_right],
        },
        Triangle {
            texture,
            corners: [top_left, bottom_right, bottom_left],
        },
    ]
}

fn band(segment: [f32; 4], width: f32, texture: TextureKey, color: [u8; 4]) -> Vec<Triangle> {
    let [x0, y0, x1, y1] = segment;
    let length = (x1 - x0).hypot(y1 - y0);
    let (nx, ny) = match length > 0.0 {
        true => (
            -(y1 - y0) / length * width / 2.0,
            (x1 - x0) / length * width / 2.0,
        ),
        false => (width / 2.0, 0.0),
    };
    let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: [x, y],
        uv: [u, v],
        color,
    };
    let start_left = corner(x0 + nx, y0 + ny, 0.0, 0.0);
    let end_left = corner(x1 + nx, y1 + ny, 1.0, 0.0);
    let end_right = corner(x1 - nx, y1 - ny, 1.0, 1.0);
    let start_right = corner(x0 - nx, y0 - ny, 0.0, 1.0);
    vec![
        Triangle {
            texture,
            corners: [start_left, end_left, end_right],
        },
        Triangle {
            texture,
            corners: [start_left, end_right, start_right],
        },
    ]
}

fn white(textures: &mut BTreeMap<TextureKey, Texture>) -> Result<TextureKey, String> {
    let pixels = [[255, 255, 255, 255]];
    let key = paint_snapshot::fingerprint([1, 1], &pixels);
    if let Entry::Vacant(entry) = textures.entry(key) {
        entry.insert(Texture::encode([1, 1], &pixels)?);
    }
    Ok(key)
}

fn picture(
    textures: &mut BTreeMap<TextureKey, Texture>,
    image: &Image,
) -> Result<TextureKey, String> {
    let size = [image.width(), image.height()];
    let pixels: Vec<[u8; 4]> = image
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect();
    let key = paint_snapshot::fingerprint(size, &pixels);
    if let Entry::Vacant(entry) = textures.entry(key) {
        entry.insert(Texture::encode(size, &pixels)?);
    }
    Ok(key)
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
