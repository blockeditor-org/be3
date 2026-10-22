use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod raster;

pub use raster::rasterize;

pub type TextureKey = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub size: [u32; 2],
    pub pixels_per_point: f32,
    pub background: [u8; 4],
    pub primitives: Vec<Primitive>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Primitive {
    pub clip: [f32; 4],
    pub content: Content,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Mesh(Vec<Triangle>),
    Callback([f32; 4]),
    RoundedRect(RoundedRect),
    Glyph(Glyph),
    CorneredRect(RoundedRect, [f32; 4]),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundedRect {
    pub rect: [f32; 4],
    pub corner_radius: f32,
    pub stroke_width: f32,
    pub color: [u8; 4],
    pub turn: Turn,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glyph {
    pub rect: [f32; 4],
    pub texture: TextureKey,
    pub color: [u8; 4],
    pub turn: Turn,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub pivot: [f32; 2],
    pub angle: f32,
}

impl Turn {
    pub const NONE: Self = Self {
        pivot: [0.0, 0.0],
        angle: 0.0,
    };

    pub fn turns(self) -> bool {
        self.angle != 0.0
    }

    pub fn scaled(self, scale: f32) -> Self {
        Self {
            pivot: [self.pivot[0] * scale, self.pivot[1] * scale],
            angle: self.angle,
        }
    }

    pub fn undo(self, point: [f32; 2]) -> [f32; 2] {
        if !self.turns() {
            return point;
        }
        let (sin, cos) = (-self.angle).sin_cos();
        let (x, y) = (point[0] - self.pivot[0], point[1] - self.pivot[1]);
        [
            self.pivot[0] + x * cos - y * sin,
            self.pivot[1] + x * sin + y * cos,
        ]
    }

    pub fn swept(self, rect: [f32; 4]) -> [f32; 4] {
        if !self.turns() {
            return rect;
        }
        let (sin, cos) = self.angle.sin_cos();
        let turned = |x: f32, y: f32| {
            let (x, y) = (x - self.pivot[0], y - self.pivot[1]);
            [
                self.pivot[0] + x * cos - y * sin,
                self.pivot[1] + x * sin + y * cos,
            ]
        };
        [
            turned(rect[0], rect[1]),
            turned(rect[2], rect[1]),
            turned(rect[2], rect[3]),
            turned(rect[0], rect[3]),
        ]
        .into_iter()
        .fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |bounds, [x, y]| {
                [
                    bounds[0].min(x),
                    bounds[1].min(y),
                    bounds[2].max(x),
                    bounds[3].max(y),
                ]
            },
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Triangle {
    pub texture: TextureKey,
    pub corners: [Vertex; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pixels {
    pub size: [u32; 2],
    pub rgba: Vec<[u8; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Painting {
    pub frame: Frame,
    pub textures: BTreeMap<TextureKey, Pixels>,
}

pub const PAINTING_EXTENSION: [u8; 16] = [
    192, 46, 2, 18, 134, 91, 69, 225, 177, 136, 216, 30, 7, 192, 247, 94,
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenPaintings {
    pub screens: Vec<(u64, Painting)>,
    pub overflows: Vec<(u64, Vec<String>)>,
}

impl ScreenPaintings {
    pub fn encoded_len(&self) -> usize {
        bincode::serialized_size(self).map_or(usize::MAX, |size| size as usize)
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub source: [f32; 4],
    pub corners: [[f32; 2]; 4],
    pub clip: [f32; 4],
}

struct Map {
    origin: [f32; 2],
    across: [f32; 2],
    down: [f32; 2],
    angle: f32,
    scale: f32,
}

impl Map {
    fn apply(&self, point: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + self.across[0] * point[0] + self.down[0] * point[1],
            self.origin[1] + self.across[1] * point[0] + self.down[1] * point[1],
        ]
    }

    fn rect(&self, rect: [f32; 4], turn: Turn) -> ([f32; 4], Turn) {
        if self.angle == 0.0 && !turn.turns() {
            let [left, top] = self.apply([rect[0], rect[1]]);
            let [right, bottom] = self.apply([rect[2], rect[3]]);
            return (
                [
                    left.min(right),
                    top.min(bottom),
                    left.max(right),
                    top.max(bottom),
                ],
                Turn::NONE,
            );
        }
        let pivot = match turn.turns() {
            true => turn.pivot,
            false => [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0],
        };
        let placed = self.apply(pivot);
        let offset = [
            placed[0] - pivot[0] * self.scale,
            placed[1] - pivot[1] * self.scale,
        ];
        (
            [
                rect[0] * self.scale + offset[0],
                rect[1] * self.scale + offset[1],
                rect[2] * self.scale + offset[0],
                rect[3] * self.scale + offset[1],
            ],
            Turn {
                pivot: placed,
                angle: turn.angle + self.angle,
            },
        )
    }

    fn bounds(&self, rect: [f32; 4]) -> [f32; 4] {
        [
            self.apply([rect[0], rect[1]]),
            self.apply([rect[2], rect[1]]),
            self.apply([rect[2], rect[3]]),
            self.apply([rect[0], rect[3]]),
        ]
        .into_iter()
        .fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |bounds, [x, y]| {
                [
                    bounds[0].min(x),
                    bounds[1].min(y),
                    bounds[2].max(x),
                    bounds[3].max(y),
                ]
            },
        )
    }
}

fn bounds(content: &Content) -> [f32; 4] {
    match content {
        Content::Mesh(triangles) => triangles.iter().flat_map(|triangle| triangle.corners).fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |bounds, corner| {
                [
                    bounds[0].min(corner.pos[0]),
                    bounds[1].min(corner.pos[1]),
                    bounds[2].max(corner.pos[0]),
                    bounds[3].max(corner.pos[1]),
                ]
            },
        ),
        Content::Callback(rect) => *rect,
        Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => {
            let reach = shape.stroke_width / 2.0;
            shape.turn.swept([
                shape.rect[0] - reach,
                shape.rect[1] - reach,
                shape.rect[2] + reach,
                shape.rect[3] + reach,
            ])
        }
        Content::Glyph(glyph) => glyph.turn.swept(glyph.rect),
    }
}

fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

impl Painting {
    pub fn cropped(mut self, rect: [f32; 4]) -> Self {
        let [left, top, right, bottom] = rect;
        let shift = |point: [f32; 2]| [point[0] - left, point[1] - top];
        let moved = |rect: [f32; 4]| [rect[0] - left, rect[1] - top, rect[2] - left, rect[3] - top];
        let turned = |turn: Turn| Turn {
            pivot: shift(turn.pivot),
            angle: turn.angle,
        };
        self.frame.primitives.retain_mut(|primitive| {
            let clip = intersect(primitive.clip, rect);
            let shown = intersect(clip, bounds(&primitive.content));
            if shown[2] <= shown[0] || shown[3] <= shown[1] {
                return false;
            }
            primitive.clip = moved(clip);
            match &mut primitive.content {
                Content::Mesh(triangles) => {
                    for triangle in triangles {
                        for corner in &mut triangle.corners {
                            corner.pos = shift(corner.pos);
                        }
                    }
                }
                Content::Callback(rect) => *rect = moved(*rect),
                Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => {
                    shape.rect = moved(shape.rect);
                    shape.turn = turned(shape.turn);
                }
                Content::Glyph(glyph) => {
                    glyph.rect = moved(glyph.rect);
                    glyph.turn = turned(glyph.turn);
                }
            }
            true
        });
        let used: std::collections::BTreeSet<TextureKey> = self
            .frame
            .primitives
            .iter()
            .flat_map(|primitive| match &primitive.content {
                Content::Mesh(triangles) => {
                    triangles.iter().map(|triangle| triangle.texture).collect()
                }
                Content::Glyph(glyph) => vec![glyph.texture],
                _ => Vec::new(),
            })
            .collect();
        self.textures.retain(|key, _| used.contains(key));
        let scale = self.frame.pixels_per_point;
        self.frame.size = [
            ((right - left) * scale).round().max(1.0) as u32,
            ((bottom - top) * scale).round().max(1.0) as u32,
        ];
        self
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|error| error.to_string())
    }

    pub fn placed(&self, placement: Placement) -> Vec<Primitive> {
        let frame = &self.frame;
        let width = frame.size[0].max(1) as f32 / frame.pixels_per_point;
        let height = frame.size[1].max(1) as f32 / frame.pixels_per_point;
        let [left, top, right, bottom] = placement.source;
        let (span_x, span_y) = (right - left, bottom - top);
        if span_x <= 0.0 || span_y <= 0.0 {
            return Vec::new();
        }
        let [c0, c1, _, c3] = placement.corners;
        let along = [c1[0] - c0[0], c1[1] - c0[1]];
        let below = [c3[0] - c0[0], c3[1] - c0[1]];
        let across = [along[0] / (span_x * width), along[1] / (span_x * width)];
        let down = [below[0] / (span_y * height), below[1] / (span_y * height)];
        let shift = [left * width, top * height];
        let map = Map {
            origin: [
                c0[0] - across[0] * shift[0] - down[0] * shift[1],
                c0[1] - across[1] * shift[0] - down[1] * shift[1],
            ],
            across,
            down,
            angle: along[1].atan2(along[0]),
            scale: (across[0].hypot(across[1]) + down[0].hypot(down[1])) / 2.0,
        };
        let source = [left * width, top * height, right * width, bottom * height];
        let shown = intersect(placement.clip, map.bounds(source));
        let scaled_rect = |shape: RoundedRect| {
            let (rect, turn) = map.rect(shape.rect, shape.turn);
            RoundedRect {
                rect,
                corner_radius: shape.corner_radius * map.scale,
                stroke_width: shape.stroke_width * map.scale,
                color: shape.color,
                turn,
            }
        };
        let background = (frame.background[3] > 0).then(|| Primitive {
            clip: shown,
            content: Content::RoundedRect(scaled_rect(RoundedRect {
                rect: source,
                corner_radius: 0.0,
                stroke_width: 0.0,
                color: frame.background,
                turn: Turn::NONE,
            })),
        });
        background
            .into_iter()
            .chain(frame.primitives.iter().filter_map(|primitive| {
                let within = intersect(primitive.clip, source);
                if within[2] <= within[0] || within[3] <= within[1] {
                    return None;
                }
                let clip = intersect(map.bounds(within), shown);
                if clip[2] <= clip[0] || clip[3] <= clip[1] {
                    return None;
                }
                let content = match &primitive.content {
                    Content::Mesh(triangles) => Content::Mesh(
                        triangles
                            .iter()
                            .map(|triangle| Triangle {
                                texture: triangle.texture,
                                corners: triangle.corners.map(|corner| Vertex {
                                    pos: map.apply(corner.pos),
                                    ..corner
                                }),
                            })
                            .collect(),
                    ),
                    Content::Callback(rect) => Content::Callback(map.bounds(*rect)),
                    Content::RoundedRect(shape) => Content::RoundedRect(scaled_rect(*shape)),
                    Content::CorneredRect(shape, corners) => Content::CorneredRect(
                        scaled_rect(*shape),
                        corners.map(|radius| radius * map.scale),
                    ),
                    Content::Glyph(glyph) => {
                        let (rect, turn) = map.rect(glyph.rect, glyph.turn);
                        Content::Glyph(Glyph {
                            rect,
                            turn,
                            ..*glyph
                        })
                    }
                };
                Some(Primitive { clip, content })
            }))
            .collect()
    }
}

#[cfg(test)]
mod tests;
