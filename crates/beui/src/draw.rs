use crate::color::Color32;
use crate::context::FrameOutput;
use crate::font::Glyph;
use crate::geometry::{Rect, vec2};
use crate::painter::Shape;

pub enum Quad {
    Rect {
        rect: [f32; 4],
        clip: [f32; 4],
        color: Color32,
        corner_radius: f32,
        stroke_width: f32,
    },
    Glyph {
        rect: [f32; 4],
        clip: [f32; 4],
        color: Color32,
        glyph: Glyph,
    },
    Punch {
        rect: [f32; 4],
        clip: [f32; 4],
        corner_radius: f32,
    },
}

pub struct Quads {
    pub list: Vec<Quad>,
    pub filtered: usize,
}

pub fn quads(output: &FrameOutput, pixels_per_point: f32) -> Quads {
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
                clip,
            } => {
                if !rect.is_positive() {
                    continue;
                }
                quads.push(Quad::Rect {
                    rect: snapped(*rect, pixels_per_point),
                    clip: bounds(*clip, pixels_per_point),
                    color: *color,
                    corner_radius: corner_radius * pixels_per_point,
                    stroke_width: stroke(*stroke_width, pixels_per_point),
                });
            }
            Shape::Text {
                origin,
                galley,
                color,
                clip,
            } => {
                let clip = bounds(*clip, pixels_per_point);
                let origin = vec2(
                    (origin.x * pixels_per_point).round(),
                    (origin.y * pixels_per_point).round(),
                );
                for glyph in galley.glyphs() {
                    if glyph.image.width == 0 || glyph.image.height == 0 {
                        continue;
                    }
                    let min = origin + glyph.offset;
                    quads.push(Quad::Glyph {
                        rect: [
                            min.x,
                            min.y,
                            min.x + glyph.image.width as f32,
                            min.y + glyph.image.height as f32,
                        ],
                        clip,
                        color: *color,
                        glyph: glyph.clone(),
                    });
                }
            }
            Shape::Punch {
                rect,
                corner_radius,
                clip,
            } => {
                if !rect.is_positive() {
                    continue;
                }
                quads.push(Quad::Punch {
                    rect: snapped(*rect, pixels_per_point),
                    clip: bounds(*clip, pixels_per_point),
                    corner_radius: corner_radius * pixels_per_point,
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
