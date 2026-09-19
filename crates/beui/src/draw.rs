use crate::color::Color32;
use crate::context::FrameOutput;
use crate::font::Glyph;
use crate::geometry::{Rect, vec2};
use crate::image::Image;
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
    Image {
        rect: [f32; 4],
        clip: [f32; 4],
        image: Image,
        tint: Color32,
        corner_radius: f32,
        smooth: bool,
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
                clip,
            } => {
                if !rect.is_positive() {
                    continue;
                }
                let rect = snapped(*rect, pixels_per_point);
                let clip = bounds(*clip, pixels_per_point);
                let stroke_width = stroke(*stroke_width, pixels_per_point);
                if skipped(damaged, expand(rect, stroke_width), clip) {
                    continue;
                }
                quads.push(Quad::Rect {
                    rect,
                    clip,
                    color: *color,
                    corner_radius: corner_radius * pixels_per_point,
                    stroke_width,
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
                let [left, top, right, bottom] = galley.pixel_bounds();
                let run = [
                    origin.x + left,
                    origin.y + top,
                    origin.x + right,
                    origin.y + bottom,
                ];
                if skipped(damaged, run, clip) {
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
                    if skipped(damaged, rect, clip) {
                        continue;
                    }
                    quads.push(Quad::Glyph {
                        rect,
                        clip,
                        color: *color,
                        glyph: glyph.clone(),
                    });
                }
            }
            Shape::Image {
                rect,
                image,
                tint,
                corner_radius,
                smooth,
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
                quads.push(Quad::Image {
                    rect,
                    clip,
                    image: image.clone(),
                    tint: *tint,
                    corner_radius: corner_radius * pixels_per_point,
                    smooth: *smooth,
                });
            }
            Shape::Punch {
                rect,
                corner_radius,
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
                quads.push(Quad::Punch {
                    rect,
                    clip,
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
