use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::hash::{Hash as _, Hasher as _};

/// Named apart from `display::Group`, the groups a display records.
pub use beui_paint_format::Group as PaintGroup;
pub use beui_paint_format::{
    Content, Fade, Frame, Glyph, Painting, Pixels, Placement, Primitive, RoundedRect,
    ScreenPaintings, TextureKey, Triangle, Turn, Vertex, preview_key, previewed,
};
use beui_paint_format::{Cap, Join, PathCommand, PathShape, PathStroke, PathStyle};

use crate::context::{FrameOutput, WindowOutput};
use crate::display::Flat;
use crate::draw::Quad;
use crate::font::GlyphImage;
use beui_types::Color32;
use beui_types::Image;
use beui_types::{PathElement, Rect, Vec2, pos2};

pub trait Paints {
    fn painting(&self, rect: Rect) -> Option<Painting>;

    fn overflows(&self, _rect: Rect) -> Vec<String> {
        Vec::new()
    }
}

pub fn overflows(output: &FrameOutput, pixels_per_point: f32) -> Vec<String> {
    let mut found = output.overflows.clone();
    let quads = output
        .windows
        .iter()
        .flat_map(|window| crate::draw::quads(window, pixels_per_point).list);
    for quad in quads {
        if let Quad::Drawing { rect, drawing, .. } = &quad
            && let Some(paints) = drawing.paints()
        {
            found.extend(paints.overflows(Rect::from_min_max(
                pos2(rect[0] / pixels_per_point, rect[1] / pixels_per_point),
                pos2(rect[2] / pixels_per_point, rect[3] / pixels_per_point),
            )));
        }
    }
    found.sort();
    found.dedup();
    found
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Punches {
    Shown,
    Erase,
}

pub fn painting(
    output: &WindowOutput,
    size: Vec2,
    pixels_per_point: f32,
    background: Color32,
    punches: Punches,
) -> Result<Painting, String> {
    let items = crate::display::flatten_faded(&output.layers);
    let mut converting = Converting {
        size,
        pixels_per_point,
        punches,
        textures: BTreeMap::new(),
        holes: Vec::new(),
    };
    let mut primitives = Vec::new();
    let mut background = background;
    if punches == Punches::Erase && background.alpha() > 0 && punched(&items) {
        let whole = [0.0, 0.0, size.x, size.y];
        primitives.push(Primitive {
            clip: whole,
            fade: Fade::NONE,
            rounds: Vec::new(),
            content: Content::RoundedRect(RoundedRect {
                rect: whole,
                corner_radius: 0.0,
                stroke_width: 0.0,
                color: background.to_array(),
                turn: Turn::NONE,
            }),
        });
        background = Color32::TRANSPARENT;
    }
    converting.items(items, &mut primitives)?;
    Ok(Painting {
        frame: Frame {
            size: [
                (size.x * pixels_per_point).round().max(1.0) as u32,
                (size.y * pixels_per_point).round().max(1.0) as u32,
            ],
            pixels_per_point,
            background: background.to_array(),
            primitives,
        },
        textures: converting.textures,
    })
}

fn punched(items: &[Flat]) -> bool {
    items.iter().any(|item| match item {
        Flat::Shape(shape, _) => matches!(shape, crate::painter::Shape::Punch { .. }),
        Flat::Group(_, inner) => punched(inner),
    })
}

/// The primitives painted before a punch, cut around its hole.
fn cut(primitives: Vec<Primitive>, hole: [f32; 4]) -> Vec<Primitive> {
    primitives
        .into_iter()
        .flat_map(|primitive: Primitive| {
            let content = match primitive.content {
                Content::Group(group) => Content::Group(PaintGroup {
                    primitives: cut(group.primitives, hole),
                    ..group
                }),
                content => content,
            };
            outside(primitive.clip, hole)
                .into_iter()
                .map(|clip| Primitive {
                    clip,
                    fade: primitive.fade,
                    rounds: primitive.rounds.clone(),
                    content: content.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

struct Converting {
    size: Vec2,
    pixels_per_point: f32,
    punches: Punches,
    textures: BTreeMap<TextureKey, Pixels>,
    /// The holes punched so far, so a punch inside a group cuts what was painted
    /// beneath the group too, as it erases the window there.
    holes: Vec<[f32; 4]>,
}

impl Converting {
    #[allow(clippy::unnecessary_to_owned)] // the copy leaves self.holes free for the loop to punch more
    fn items(&mut self, items: Vec<Flat>, primitives: &mut Vec<Primitive>) -> Result<(), String> {
        let mut quads = Vec::new();
        let mut run = Vec::new();
        for item in items {
            match item {
                Flat::Shape(shape, fade) => run.push((shape, fade)),
                Flat::Group(opacity, inner) => {
                    self.shapes(std::mem::take(&mut run), &mut quads, primitives)?;
                    let mut grouped = Vec::new();
                    let punched = self.holes.len();
                    self.items(inner, &mut grouped)?;
                    for hole in self.holes[punched..].to_vec() {
                        *primitives = cut(std::mem::take(primitives), hole);
                    }
                    primitives.extend(PaintGroup::primitive(opacity, grouped));
                }
            }
        }
        self.shapes(run, &mut quads, primitives)
    }

    fn shapes(
        &mut self,
        shapes: Vec<(crate::painter::Shape, crate::fade::Fade)>,
        quads: &mut Vec<Quad>,
        primitives: &mut Vec<Primitive>,
    ) -> Result<(), String> {
        let (size, pixels_per_point, punches) = (self.size, self.pixels_per_point, self.punches);
        let textures = &mut self.textures;
        let holes = &mut self.holes;
        let points = |values: [f32; 4]| values.map(|value| value / pixels_per_point);
        let turn_of = |turn: crate::draw::Turn| turned(turn, pixels_per_point);
        quads.clear();
        for (shape, fade) in shapes {
            let fade = saved(fade);
            if let crate::painter::Shape::Path {
                path,
                scale,
                offset,
                color,
                rotation,
                clip,
            } = &shape
            {
                let clip = [clip.min.x, clip.min.y, clip.max.x, clip.max.y]
                    .map(|value| (value * pixels_per_point).round() / pixels_per_point);
                let outside = clip[2].min(size.x) <= clip[0].max(0.0)
                    || clip[3].min(size.y) <= clip[1].max(0.0);
                if !outside && !path.path().is_empty() {
                    primitives.push(Primitive {
                        clip,
                        fade,
                        rounds: Vec::new(),
                        content: Content::Path(placed_path(path, *scale, *offset, *rotation, *color)),
                    });
                }
                continue;
            }
            crate::draw::push_quads(&shape, pixels_per_point, None, quads);
            for quad in quads.drain(..) {
            if let Quad::Drawing {
                rect,
                clip,
                drawing,
            } = &quad
                && let Some(painted) = drawing.paints().and_then(|paints| {
                    paints.painting(Rect::from_min_max(
                        pos2(rect[0] / pixels_per_point, rect[1] / pixels_per_point),
                        pos2(rect[2] / pixels_per_point, rect[3] / pixels_per_point),
                    ))
                })
            {
                let clip = points(*clip);
                for (key, pixels) in painted.textures {
                    textures.entry(key).or_insert(pixels);
                }
                for mut primitive in painted.frame.primitives {
                    primitive.clip = [
                        primitive.clip[0].max(clip[0]),
                        primitive.clip[1].max(clip[1]),
                        primitive.clip[2].min(clip[2]),
                        primitive.clip[3].min(clip[3]),
                    ];
                    primitive.fade = saved(loaded(fade).within(loaded(primitive.fade)));
                    if primitive.clip[2] > primitive.clip[0]
                        && primitive.clip[3] > primitive.clip[1]
                    {
                        primitives.push(primitive);
                    }
                }
                continue;
            }
            if let Quad::Punch { rect, clip, .. } = &quad
                && punches == Punches::Erase
            {
                let hole = intersect(points(*rect), points(*clip));
                *primitives = cut(std::mem::take(primitives), hole);
                holes.push(hole);
                continue;
            }
            let (clip, content) = match quad {
                Quad::Rect {
                    rect,
                    clip,
                    color,
                    corner_radius,
                    stroke_width,
                    turn,
                } => {
                    let corners = corner_radius.map(|radius| radius / pixels_per_point);
                    let shape = RoundedRect {
                        rect: points(rect),
                        corner_radius: corners[0],
                        stroke_width: stroke_width / pixels_per_point,
                        color: color.to_array(),
                        turn: turn_of(turn),
                    };
                    match corners.iter().all(|radius| *radius == corners[0]) {
                        true => (clip, Content::RoundedRect(shape)),
                        false => (clip, Content::CorneredRect(shape, corners)),
                    }
                }
                Quad::Glyph {
                    rect,
                    clip,
                    color,
                    glyph,
                    turn,
                } if glyph.image().is_some_and(|image| image.color) => (
                    clip,
                    Content::Mesh(mesh(
                        points(rect),
                        [0.0, 0.0, 1.0, 1.0],
                        texture(textures, glyph.image())?,
                        [255, 255, 255, color.to_array()[3]],
                        turn_of(turn),
                    )),
                ),
                Quad::Glyph {
                    rect,
                    clip,
                    color,
                    glyph,
                    turn,
                } => (
                    clip,
                    Content::Glyph(Glyph {
                        rect: points(rect),
                        texture: texture(textures, glyph.image())?,
                        color: color.to_array(),
                        turn: turn_of(turn),
                    }),
                ),
                Quad::Image {
                    rect,
                    clip,
                    source,
                    image,
                    tint,
                    turn,
                    ..
                } => (
                    clip,
                    Content::Mesh(mesh(
                        points(rect),
                        source,
                        picture(textures, &image)?,
                        tint.to_array(),
                        turn_of(turn),
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
                        white(textures)?,
                        color.to_array(),
                    )),
                ),
                Quad::Punch { rect, clip, .. } | Quad::Drawing { rect, clip, .. } => {
                    (clip, Content::Callback(points(rect)))
                }
            };
            let clip = points(clip);
            let outside =
                clip[2].min(size.x) <= clip[0].max(0.0) || clip[3].min(size.y) <= clip[1].max(0.0);
            if outside {
                continue;
            }
            primitives.push(Primitive {
                clip,
                fade,
                rounds: Vec::new(),
                content,
            });
            }
        }
        Ok(())
    }
}

/// `path` placed as `Shape::Path` places it, in points: scaled, moved by `offset` and turned.
fn placed_path(
    path: &crate::path::PathPaint,
    scale: f32,
    offset: Vec2,
    rotation: beui_types::Rotation,
    color: beui_types::Color32,
) -> PathShape {
    let (sin, cos) = rotation.angle.sin_cos();
    let pivot = rotation.pivot;
    let place = |at: beui_types::Pos2| {
        let (x, y) = (at.x * scale + offset.x, at.y * scale + offset.y);
        match rotation.turns() {
            false => [x, y],
            true => {
                let (x, y) = (x - pivot.x, y - pivot.y);
                [pivot.x + x * cos - y * sin, pivot.y + x * sin + y * cos]
            }
        }
    };
    let commands = path
        .path()
        .elements()
        .iter()
        .map(|element| match *element {
            PathElement::MoveTo(to) => PathCommand::MoveTo(place(to)),
            PathElement::LineTo(to) => PathCommand::LineTo(place(to)),
            PathElement::QuadTo(control, to) => PathCommand::QuadTo(place(control), place(to)),
            PathElement::CubicTo(first, second, to) => {
                PathCommand::CubicTo(place(first), place(second), place(to))
            }
            PathElement::Close => PathCommand::Close,
        })
        .collect();
    let style = match path.style() {
        beui_types::PathStyle::Fill(rule) => PathStyle::Fill {
            even_odd: *rule == beui_types::FillRule::EvenOdd,
        },
        // As the renderer strokes it (`kurbo_stroke`): dots and odd dash lists made even.
        beui_types::PathStyle::Stroke(stroke) => {
            let drawn = crate::path::kurbo_stroke(stroke, scale);
            PathStyle::Stroke(PathStroke {
                width: drawn.width as f32,
                join: match drawn.join {
                    kurbo::Join::Miter => Join::Miter,
                    kurbo::Join::Round => Join::Round,
                    kurbo::Join::Bevel => Join::Bevel,
                },
                miter_limit: drawn.miter_limit as f32,
                cap: match drawn.start_cap {
                    kurbo::Cap::Butt => Cap::Butt,
                    kurbo::Cap::Round => Cap::Round,
                    kurbo::Cap::Square => Cap::Square,
                },
                dashes: drawn.dash_pattern.iter().map(|dash| *dash as f32).collect(),
                dash_offset: drawn.dash_offset as f32,
            })
        }
    };
    PathShape {
        commands,
        style,
        color: color.to_array(),
        seamed: path.seamed(),
    }
}

fn saved(fade: crate::fade::Fade) -> Fade {
    match fade.is_none() {
        true => Fade::NONE,
        false => Fade {
            rect: [
                fade.rect.min.x,
                fade.rect.min.y,
                fade.rect.max.x,
                fade.rect.max.y,
            ],
            widths: fade.widths,
        },
    }
}

fn loaded(fade: Fade) -> crate::fade::Fade {
    let [left, top, right, bottom] = fade.rect;
    crate::fade::Fade::new(
        Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
        fade.widths,
    )
}

fn turned(turn: crate::draw::Turn, pixels_per_point: f32) -> Turn {
    Turn {
        pivot: [
            turn.pivot[0] / pixels_per_point,
            turn.pivot[1] / pixels_per_point,
        ],
        angle: turn.sin.atan2(turn.cos),
    }
}

fn mesh(
    rect: [f32; 4],
    source: [f32; 4],
    texture: TextureKey,
    color: [u8; 4],
    turn: Turn,
) -> Vec<Triangle> {
    let corner = |x: f32, y: f32, u: f32, v: f32| {
        let [x, y] = spun(turn, [x, y]);
        Vertex {
            pos: [x, y],
            uv: [u, v],
            color,
        }
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

fn spun(turn: Turn, point: [f32; 2]) -> [f32; 2] {
    if !turn.turns() {
        return point;
    }
    let (sin, cos) = turn.angle.sin_cos();
    let (x, y) = (point[0] - turn.pivot[0], point[1] - turn.pivot[1]);
    [
        turn.pivot[0] + x * cos - y * sin,
        turn.pivot[1] + x * sin + y * cos,
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

fn white(textures: &mut BTreeMap<TextureKey, Pixels>) -> Result<TextureKey, String> {
    let pixels = [[255, 255, 255, 255]];
    Ok(keep(textures, [1, 1], pixels.to_vec()))
}

fn picture(
    textures: &mut BTreeMap<TextureKey, Pixels>,
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
    Ok(keep(textures, size, pixels))
}

fn texture(
    textures: &mut BTreeMap<TextureKey, Pixels>,
    image: Option<&GlyphImage>,
) -> Result<TextureKey, String> {
    let image = image.ok_or("a glyph that could not be rasterized")?;
    let size = [image.width, image.height];
    let pixels: Vec<[u8; 4]> = match image.color {
        true => image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
            .collect(),
        false => image.pixels.iter().map(|coverage| [*coverage; 4]).collect(),
    };
    Ok(keep(textures, size, pixels))
}

fn keep(
    textures: &mut BTreeMap<TextureKey, Pixels>,
    size: [u32; 2],
    rgba: Vec<[u8; 4]>,
) -> TextureKey {
    let mut hasher = std::hash::DefaultHasher::new();
    size.hash(&mut hasher);
    rgba.hash(&mut hasher);
    let key = hasher.finish();
    if let Entry::Vacant(entry) = textures.entry(key) {
        entry.insert(Pixels { size, rgba });
    }
    key
}

fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

fn outside(clip: [f32; 4], hole: [f32; 4]) -> Vec<[f32; 4]> {
    let cut = intersect(clip, hole);
    if cut[2] <= cut[0] || cut[3] <= cut[1] {
        return vec![clip];
    }
    [
        [clip[0], clip[1], clip[2], cut[1]],
        [clip[0], cut[3], clip[2], clip[3]],
        [clip[0], cut[1], cut[0], cut[3]],
        [cut[2], cut[1], clip[2], cut[3]],
    ]
    .into_iter()
    .filter(|piece| piece[2] > piece[0] && piece[3] > piece[1])
    .collect()
}
