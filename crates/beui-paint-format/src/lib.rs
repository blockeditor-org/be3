use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

mod raster;

pub use raster::rasterize;

pub type TextureKey = u64;

pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    ciborium::from_reader(bytes).map_err(|error| error.to_string())
}

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
    #[serde(default, skip_serializing_if = "Fade::is_none")]
    pub fade: Fade,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rounds: Vec<Round>,
    pub content: Content,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Round {
    pub rect: [f32; 4],
    pub radius: f32,
    pub turn: Turn,
}

impl Round {
    pub fn scaled(self, scale: f32) -> Self {
        Self {
            rect: self.rect.map(|value| value * scale),
            radius: self.radius * scale,
            turn: self.turn.scaled(scale),
        }
    }

    fn moved(self, by: [f32; 2]) -> Self {
        Self {
            rect: [
                self.rect[0] + by[0],
                self.rect[1] + by[1],
                self.rect[2] + by[0],
                self.rect[3] + by[1],
            ],
            turn: Turn {
                pivot: [self.turn.pivot[0] + by[0], self.turn.pivot[1] + by[1]],
                angle: self.turn.angle,
            },
            radius: self.radius,
        }
    }

    fn mapped(self, map: &Map) -> Self {
        let (rect, turn) = map.rect(self.rect, self.turn);
        Self {
            rect,
            radius: self.radius * map.scale,
            turn,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fade {
    pub rect: [f32; 4],
    pub widths: [f32; 4],
    #[serde(default = "opaque", skip_serializing_if = "is_opaque")]
    pub opacity: f32,
}

fn opaque() -> f32 {
    1.0
}

fn is_opaque(opacity: &f32) -> bool {
    *opacity >= 1.0
}

impl Default for Fade {
    fn default() -> Self {
        Self::NONE
    }
}

impl Fade {
    pub const NONE: Self = Self {
        rect: [0.0; 4],
        widths: [0.0; 4],
        opacity: 1.0,
    };

    pub fn is_none(&self) -> bool {
        self.widths == [0.0; 4] && is_opaque(&self.opacity)
    }

    pub fn alpha(self, point: [f32; 2]) -> f32 {
        let ramp = |distance: f32, width: f32| match width > 0.0 {
            true => (distance / width).clamp(0.0, 1.0),
            false => 1.0,
        };
        let [left, top, right, bottom] = self.rect;
        let [near_x, near_y, far_x, far_y] = self.widths;
        ramp(point[0] - left, near_x)
            * ramp(point[1] - top, near_y)
            * ramp(right - point[0], far_x)
            * ramp(bottom - point[1], far_y)
            * self.opacity
    }

    pub fn scaled(self, scale: f32) -> Self {
        Self {
            rect: self.rect.map(|value| value * scale),
            widths: self.widths.map(|width| width * scale),
            opacity: self.opacity,
        }
    }

    fn moved(self, by: [f32; 2]) -> Self {
        match self.is_none() {
            true => self,
            false => Self {
                rect: [
                    self.rect[0] + by[0],
                    self.rect[1] + by[1],
                    self.rect[2] + by[0],
                    self.rect[3] + by[1],
                ],
                ..self
            },
        }
    }
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
    #[serde(serialize_with = "flat", deserialize_with = "unflat")]
    pub rgba: Vec<[u8; 4]>,
}

fn flat<S: Serializer>(rgba: &[[u8; 4]], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(rgba.as_flattened())
}

fn unflat<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<[u8; 4]>, D::Error> {
    let bytes = serde_bytes::ByteBuf::deserialize(deserializer)?;
    match bytes.as_chunks::<4>() {
        (rgba, []) => Ok(rgba.to_vec()),
        _ => Err(serde::de::Error::custom(
            "pixels are not a whole number of rgba values",
        )),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Painting {
    pub frame: Frame,
    pub textures: BTreeMap<TextureKey, Pixels>,
}

const PREVIEW_KEYS: TextureKey = 0xb3e0 << 48;
const PREVIEW_MASK: TextureKey = 0xffff << 48;

pub fn preview_key(image: u64) -> TextureKey {
    PREVIEW_KEYS | (image & !PREVIEW_MASK)
}

pub fn previewed(key: TextureKey) -> Option<u64> {
    (key & PREVIEW_MASK == PREVIEW_KEYS).then_some(key & !PREVIEW_MASK)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenPaintings {
    pub screens: Vec<(u64, Painting)>,
    pub overflows: Vec<(u64, Vec<String>)>,
}

impl ScreenPaintings {
    pub fn encode(&self) -> Vec<u8> {
        encode(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        decode(bytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub source: [f32; 4],
    pub corners: [[f32; 2]; 4],
    pub clip: [f32; 4],
    pub corner_radius: f32,
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
            primitive.fade = primitive.fade.moved([-left, -top]);
            for round in &mut primitive.rounds {
                *round = round.moved([-left, -top]);
            }
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
        encode(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        decode(bytes)
    }

    pub fn previews(&self) -> Vec<u64> {
        let mut found: Vec<u64> = self
            .frame
            .primitives
            .iter()
            .filter_map(|primitive| self.preview_in(primitive))
            .map(|(image, _)| image)
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }

    pub fn with_previews(mut self, mut painting_of: impl FnMut(u64) -> Option<Painting>) -> Self {
        let primitives = std::mem::take(&mut self.frame.primitives);
        for primitive in primitives {
            let Some((image, corners)) = self.preview_in(&primitive) else {
                self.frame.primitives.push(primitive);
                continue;
            };
            let Some(preview) = painting_of(image) else {
                continue;
            };
            self.frame.primitives.extend(preview.placed(Placement {
                source: [0.0, 0.0, 1.0, 1.0],
                corners,
                clip: primitive.clip,
                corner_radius: 0.0,
            }));
            for (key, pixels) in preview.textures {
                self.textures.entry(key).or_insert(pixels);
            }
        }
        self
    }

    fn preview_in(&self, primitive: &Primitive) -> Option<(u64, [[f32; 2]; 4])> {
        let Content::Mesh(triangles) = &primitive.content else {
            return None;
        };
        let first = triangles.first()?.texture;
        let image = previewed(first).filter(|_| !self.textures.contains_key(&first))?;
        let mut corners = [None; 4];
        for triangle in triangles {
            if triangle.texture != first {
                return None;
            }
            for vertex in triangle.corners {
                let at = match vertex.uv {
                    [0.0, 0.0] => 0,
                    [1.0, 0.0] => 1,
                    [1.0, 1.0] => 2,
                    [0.0, 1.0] => 3,
                    _ => return None,
                };
                corners[at] = Some(vertex.pos);
            }
        }
        Some((image, [corners[0]?, corners[1]?, corners[2]?, corners[3]?]))
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
        let round = (placement.corner_radius > 0.0).then(|| {
            let (rect, turn) = map.rect([0.0, 0.0, width, height], Turn::NONE);
            Round {
                rect,
                radius: placement.corner_radius,
                turn,
            }
        });
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
            fade: Fade::NONE,
            rounds: round.into_iter().collect(),
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
                let fade = match primitive.fade.is_none() {
                    true => Fade::NONE,
                    false => Fade {
                        rect: map.bounds(primitive.fade.rect),
                        widths: primitive.fade.widths.map(|width| width * map.scale),
                        opacity: primitive.fade.opacity,
                    },
                };
                let rounds = primitive
                    .rounds
                    .iter()
                    .map(|held| held.mapped(&map))
                    .chain(round)
                    .collect();
                Some(Primitive {
                    clip,
                    fade,
                    rounds,
                    content,
                })
            }))
            .collect()
    }
}

#[cfg(test)]
mod tests;
