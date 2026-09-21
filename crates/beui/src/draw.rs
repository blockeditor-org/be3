use crate::color::Color32;
use crate::context::FrameOutput;
use crate::drawing::Drawing;
use crate::font::Glyph;
use crate::geometry::{Rect, Rotation, vec2};
use crate::image::Image;
use crate::painter::Shape;

pub enum Quad {
    Rect {
        rect: [f32; 4],
        clip: [f32; 4],
        color: Color32,
        corner_radius: f32,
        stroke_width: f32,
        turn: Turn,
    },
    Glyph {
        rect: [f32; 4],
        clip: [f32; 4],
        color: Color32,
        glyph: Glyph,
        turn: Turn,
    },
    Image {
        rect: [f32; 4],
        clip: [f32; 4],
        source: [f32; 4],
        image: Image,
        tint: Color32,
        corner_radius: f32,
        smooth: bool,
        turn: Turn,
    },
    Line {
        rect: [f32; 4],
        clip: [f32; 4],
        segment: [f32; 4],
        width: f32,
        color: Color32,
    },
    Punch {
        rect: [f32; 4],
        clip: [f32; 4],
        corner_radius: f32,
        turn: Turn,
    },
    Drawing {
        rect: [f32; 4],
        clip: [f32; 4],
        drawing: Drawing,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn {
    pub pivot: [f32; 2],
    pub cos: f32,
    pub sin: f32,
}

impl Turn {
    pub const NONE: Self = Self {
        pivot: [0.0, 0.0],
        cos: 1.0,
        sin: 0.0,
    };

    fn of(rotation: Rotation, pixels_per_point: f32) -> Self {
        if !rotation.turns() {
            return Self::NONE;
        }
        let (sin, cos) = rotation.angle.sin_cos();
        Self {
            pivot: [
                rotation.pivot.x * pixels_per_point,
                rotation.pivot.y * pixels_per_point,
            ],
            cos,
            sin,
        }
    }

    fn swept(self, rect: [f32; 4]) -> [f32; 4] {
        if self == Self::NONE {
            return rect;
        }
        let turned = |x: f32, y: f32| {
            let (x, y) = (x - self.pivot[0], y - self.pivot[1]);
            [
                self.pivot[0] + x * self.cos - y * self.sin,
                self.pivot[1] + x * self.sin + y * self.cos,
            ]
        };
        let corners = [
            turned(rect[0], rect[1]),
            turned(rect[2], rect[1]),
            turned(rect[2], rect[3]),
            turned(rect[0], rect[3]),
        ];
        corners.into_iter().fold(
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

pub struct Quads {
    pub list: Vec<Quad>,
    pub filtered: usize,
}

pub fn quads(output: &FrameOutput, pixels_per_point: f32) -> Quads {
    quads_within(output, pixels_per_point, None)
}

pub fn quads_within(
    output: &FrameOutput,
    pixels_per_point: f32,
    damaged: Option<[f32; 4]>,
) -> Quads {
    let boundary = output.filtered_shapes();
    let mut filtered = 0;
    let mut quads = Vec::new();
    for (index, shape) in output.shapes.iter().enumerate() {
        if boundary == Some(index) {
            filtered = quads.len();
        }
        match shape {
            Shape::Rect {
                rect,
                corner_radius,
                stroke_width,
                color,
                rotation,
                clip,
            } => {
                if !rect.is_positive() {
                    continue;
                }
                let turn = Turn::of(*rotation, pixels_per_point);
                let rect = snapped(*rect, pixels_per_point);
                let clip = bounds(*clip, pixels_per_point);
                let stroke_width = stroke(*stroke_width, pixels_per_point);
                if skipped(damaged, turn.swept(expand(rect, stroke_width)), clip) {
                    continue;
                }
                quads.push(Quad::Rect {
                    rect,
                    clip,
                    color: *color,
                    corner_radius: corner_radius * pixels_per_point,
                    stroke_width,
                    turn,
                });
            }
            Shape::Text {
                origin,
                galley,
                color,
                rotation,
                clip,
            } => {
                let turn = Turn::of(*rotation, pixels_per_point);
                let clip = bounds(*clip, pixels_per_point);
                let origin = vec2(
                    (origin.x * pixels_per_point).round(),
                    (origin.y * pixels_per_point).round(),
                );
                let [left, top, right, bottom] = galley.pixel_bounds();
                let run = [
                    origin.x + left,
                    origin.y + top,
                    origin.x + right,
                    origin.y + bottom,
                ];
                if skipped(damaged, turn.swept(run), clip) {
                    continue;
                }
                for glyph in galley.glyphs() {
                    if glyph.image.width == 0 || glyph.image.height == 0 {
                        continue;
                    }
                    let min = origin + glyph.offset;
                    let rect = [
                        min.x,
                        min.y,
                        min.x + glyph.image.width as f32,
                        min.y + glyph.image.height as f32,
                    ];
                    if skipped(damaged, turn.swept(rect), clip) {
                        continue;
                    }
                    quads.push(Quad::Glyph {
                        rect,
                        clip,
                        color: *color,
                        glyph: glyph.clone(),
                        turn,
                    });
                }
            }
            Shape::Image {
                rect,
                source,
                image,
                tint,
                corner_radius,
                smooth,
                rotation,
                clip,
            } => {
                if !rect.is_positive() {
                    continue;
                }
                let turn = Turn::of(*rotation, pixels_per_point);
                let rect = snapped(*rect, pixels_per_point);
                let clip = bounds(*clip, pixels_per_point);
                if skipped(damaged, turn.swept(rect), clip) {
                    continue;
                }
                quads.push(Quad::Image {
                    rect,
                    clip,
                    source: [source.min.x, source.min.y, source.max.x, source.max.y],
                    image: image.clone(),
                    tint: *tint,
                    corner_radius: corner_radius * pixels_per_point,
                    smooth: *smooth,
                    turn,
                });
            }
            Shape::Line {
                from,
                to,
                width,
                color,
                clip,
            } => {
                let clip = bounds(*clip, pixels_per_point);
                let segment = [
                    from.x * pixels_per_point,
                    from.y * pixels_per_point,
                    to.x * pixels_per_point,
                    to.y * pixels_per_point,
                ];
                let width = (width * pixels_per_point).max(1.0);
                let rect = [
                    segment[0].min(segment[2]) - width / 2.0 - 1.0,
                    segment[1].min(segment[3]) - width / 2.0 - 1.0,
                    segment[0].max(segment[2]) + width / 2.0 + 1.0,
                    segment[1].max(segment[3]) + width / 2.0 + 1.0,
                ];
                if skipped(damaged, rect, clip) {
                    continue;
                }
                quads.push(Quad::Line {
                    rect,
                    clip,
                    segment,
                    width,
                    color: *color,
                });
            }
            Shape::Punch {
                rect,
                corner_radius,
                rotation,
                clip,
            } => {
                if !rect.is_positive() {
                    continue;
                }
                let turn = Turn::of(*rotation, pixels_per_point);
                let rect = snapped(*rect, pixels_per_point);
                let clip = bounds(*clip, pixels_per_point);
                if skipped(damaged, turn.swept(rect), clip) {
                    continue;
                }
                quads.push(Quad::Punch {
                    rect,
                    clip,
                    corner_radius: corner_radius * pixels_per_point,
                    turn,
                });
            }
            Shape::Drawing {
                rect,
                drawing,
                clip,
            } => {
                if !rect.is_positive() {
                    continue;
                }
                let rect = snapped(*rect, pixels_per_point);
                let clip = bounds(*clip, pixels_per_point);
                if skipped(damaged, rect, clip) {
                    continue;
                }
                quads.push(Quad::Drawing {
                    rect,
                    clip,
                    drawing: drawing.clone(),
                });
            }
        }
    }
    if boundary.is_some_and(|boundary| boundary >= output.shapes.len()) {
        filtered = quads.len();
    }
    Quads {
        list: quads,
        filtered,
    }
}

fn expand(rect: [f32; 4], amount: f32) -> [f32; 4] {
    [
        rect[0] - amount,
        rect[1] - amount,
        rect[2] + amount,
        rect[3] + amount,
    ]
}

fn skipped(damaged: Option<[f32; 4]>, rect: [f32; 4], clip: [f32; 4]) -> bool {
    let Some(damaged) = damaged else {
        return false;
    };
    let left = rect[0].max(clip[0]).max(damaged[0]);
    let top = rect[1].max(clip[1]).max(damaged[1]);
    let right = rect[2].min(clip[2]).min(damaged[2]);
    let bottom = rect[3].min(clip[3]).min(damaged[3]);
    left >= right || top >= bottom
}

fn bounds(rect: Rect, pixels_per_point: f32) -> [f32; 4] {
    [
        (rect.min.x * pixels_per_point).round(),
        (rect.min.y * pixels_per_point).round(),
        (rect.max.x * pixels_per_point).round(),
        (rect.max.y * pixels_per_point).round(),
    ]
}

fn snapped(rect: Rect, pixels_per_point: f32) -> [f32; 4] {
    let [left, top, right, bottom] = bounds(rect, pixels_per_point);
    [left, top, right.max(left + 1.0), bottom.max(top + 1.0)]
}

fn stroke(width: f32, pixels_per_point: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    (width * pixels_per_point).round().max(1.0)
}
