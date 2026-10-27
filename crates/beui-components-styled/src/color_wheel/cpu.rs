use std::cell::RefCell;
use std::rc::Rc;

use beui_components_unstyled::WheelGeometry;
use beui_view::reactive::Painter;
use beui_view::types::{Color, Color32, Hsva, Image, Oklch, Rect, Vec2, pos2};

use super::{Face, Model};

const TIP_CHROMA: f32 = 0.38;
const GAMUT_LINE_OPACITY: f32 = 0.7;
const DEGENERATE: f32 = 1e-6;

#[derive(Clone, Default)]
pub(super) struct Faces {
    held: Rc<RefCell<Option<(Face, u32, Image)>>>,
}

impl Faces {
    pub(super) fn paint(&self, painter: &Painter, bounds: Rect, face: Face) {
        let side = (bounds.width() * painter.pixel_grid().scale()).ceil().max(1.0) as u32;
        let image = {
            let mut held = self.held.borrow_mut();
            match held.as_ref() {
                Some((drawn, drawn_side, image)) if *drawn == face && *drawn_side == side => {
                    image.clone()
                }
                _ => {
                    let image = face_image(face, side, side as f32 / bounds.width());
                    *held = Some((face, side, image.clone()));
                    image
                }
            }
        };
        let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(bounds, whole, &image, Color::WHITE, 0.0, true);
    }
}

fn face_image(face: Face, side: u32, scale: f32) -> Image {
    let size = side as f32;
    let wheel = WheelGeometry::new(
        Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(size)),
        face.ring_width * scale,
    );
    let corners = wheel.corners(face.hue);
    let cusp = Oklch::cusp(face.hue);
    let gap = face.ring_gap * scale;
    let line = face.gamut_line * scale;
    let mut pixels = vec![0u8; (side * side * 4) as usize];
    let excess: Vec<f32> = match face.model {
        Model::Hsv => Vec::new(),
        Model::Oklch => (0..side * side)
            .map(|index| {
                let at = pos2((index % side) as f32 + 0.5, (index / side) as f32 + 0.5);
                let (saturation, value) = triangle_at(corners, at);
                let (lightness, chroma) = oklch_at(cusp, saturation, value);
                chroma - Oklch::max_chroma(lightness, face.hue)
            })
            .collect(),
    };
    for y in 0..side {
        for x in 0..side {
            let at = pos2(x as f32 + 0.5, y as f32 + 0.5);
            let offset = at - wheel.centre;
            let distance = offset.length();
            let ring = coverage((distance - (wheel.inner + gap)).min(wheel.outer - distance));
            let (color, alpha) = if ring > 0.0 {
                (ring_color(face.model, wheel.hue_at(at)), ring)
            } else {
                let triangle = coverage(inside(corners, at));
                if triangle <= 0.0 {
                    continue;
                }
                let (saturation, value) = triangle_at(corners, at);
                let color = match face.model {
                    Model::Hsv => Hsva::new(face.hue, saturation, value, 1.0).to_color(),
                    Model::Oklch => {
                        let index = (y * side + x) as usize;
                        let (lightness, chroma) = oklch_at(cusp, saturation, value);
                        let color = Oklch::new(
                            lightness,
                            chroma - excess[index].max(0.0),
                            face.hue,
                            1.0,
                        )
                        .to_color();
                        let slope = slope(&excess, side, x, y);
                        match slope > 1e-7 {
                            true => {
                                let strength = coverage(line / 2.0 - (excess[index] / slope).abs())
                                    * GAMUT_LINE_OPACITY;
                                gamut_line(color, strength)
                            }
                            false => color,
                        }
                    }
                };
                (color, triangle)
            };
            let [red, green, blue, _] = color.to_array();
            let at = ((y * side + x) * 4) as usize;
            pixels[at..at + 4].copy_from_slice(&[
                red,
                green,
                blue,
                (alpha * 255.0).round() as u8,
            ]);
        }
    }
    Image::from_rgba(side, side, pixels)
}

fn coverage(depth: f32) -> f32 {
    (depth + 0.5).clamp(0.0, 1.0)
}

fn ring_color(model: Model, hue: f32) -> Color32 {
    match model {
        Model::Hsv => Hsva::new(hue, 1.0, 1.0, 1.0).to_color(),
        Model::Oklch => {
            let tip = Oklch::cusp(hue);
            Oklch::new(tip.lightness, tip.chroma.min(TIP_CHROMA), hue, 1.0).to_color()
        }
    }
}

fn oklch_at(cusp: Oklch, saturation: f32, value: f32) -> (f32, f32) {
    let tip = value * saturation;
    let lightness = (value - tip * (1.0 - cusp.lightness)).clamp(0.0, 1.0);
    (lightness, tip * TIP_CHROMA)
}

fn slope(excess: &[f32], side: u32, x: u32, y: u32) -> f32 {
    let at = |x: u32, y: u32| excess[(y * side + x) as usize];
    let across = match x + 1 < side {
        true => at(x + 1, y) - at(x, y),
        false => at(x, y) - at(x.saturating_sub(1), y),
    };
    let down = match y + 1 < side {
        true => at(x, y + 1) - at(x, y),
        false => at(x, y) - at(x, y.saturating_sub(1)),
    };
    across.hypot(down)
}

fn gamut_line(color: Color32, strength: f32) -> Color32 {
    let [red, green, blue, alpha] = color.to_array();
    let luminance =
        (0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue)) / 255.0;
    let ink = match luminance > 140.0 / 255.0 {
        true => 0.0,
        false => 255.0,
    };
    let mix = |channel: u8| (f32::from(channel) + (ink - f32::from(channel)) * strength).round() as u8;
    Color32::from_rgba_unmultiplied(mix(red), mix(green), mix(blue), alpha)
}

fn inside([tip, white, black]: [beui_view::types::Pos2; 3], at: beui_view::types::Pos2) -> f32 {
    [(tip, white), (white, black), (black, tip)]
        .into_iter()
        .map(|(start, end)| {
            let edge = end - start;
            let length = edge.length().max(DEGENERATE);
            (edge.y * (at.x - start.x) - edge.x * (at.y - start.y)) / length
        })
        .fold(f32::INFINITY, f32::min)
}

fn triangle_at(
    [tip, white, black]: [beui_view::types::Pos2; 3],
    at: beui_view::types::Pos2,
) -> (f32, f32) {
    let (to_tip, to_white, to_at) = (tip - black, white - black, at - black);
    let area = to_tip.x * to_white.y - to_tip.y * to_white.x;
    if area.abs() < DEGENERATE {
        return (0.0, 0.0);
    }
    let tip_weight = (to_at.x * to_white.y - to_at.y * to_white.x) / area;
    let white_weight = (to_tip.x * to_at.y - to_tip.y * to_at.x) / area;
    let lit = tip_weight + white_weight;
    if lit.abs() < DEGENERATE {
        return (0.0, 0.0);
    }
    (
        (tip_weight / lit).clamp(0.0, 1.0),
        lit.clamp(0.0, 1.0),
    )
}
