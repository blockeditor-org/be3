use crate::painter::Painter;
use beui_types::Color32;
use beui_types::{Pos2, Rect, Vec2};

pub const SIZE: f32 = 20.0;
pub const STROKE: f32 = 1.0;
pub const FILL: Color32 = Color32::from_rgba_unmultiplied(255, 255, 255, 96);
pub const RING: Color32 = Color32::BLACK;
const SAMPLES: u32 = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct CursorImage {
    pub width: u32,
    pub height: u32,
    pub hotspot: (u32, u32),
    pub rgba: Vec<u8>,
}

pub fn image(pixels_per_point: f32) -> CursorImage {
    let size = (SIZE * pixels_per_point).round().max(1.0) as u32;
    let radius = size as f32 / 2.0;
    let stroke = STROKE * pixels_per_point;
    let samples = SAMPLES as f32;
    let count = samples * samples;
    let [fill_red, fill_green, fill_blue, fill_alpha] = FILL.to_array();
    let [ring_red, ring_green, ring_blue, ring_alpha] = RING.to_array();
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0.0f32; 4];
            for sample_y in 0..SAMPLES {
                for sample_x in 0..SAMPLES {
                    let px = x as f32 + (sample_x as f32 + 0.5) / samples;
                    let py = y as f32 + (sample_y as f32 + 0.5) / samples;
                    let distance = (px - radius).hypot(py - radius);
                    let color = match distance {
                        d if d <= radius - stroke => [fill_red, fill_green, fill_blue, fill_alpha],
                        d if d <= radius => [ring_red, ring_green, ring_blue, ring_alpha],
                        _ => continue,
                    };
                    let alpha = f32::from(color[3]);
                    for channel in 0..3 {
                        sum[channel] += f32::from(color[channel]) * alpha;
                    }
                    sum[3] += alpha;
                }
            }
            let alpha = sum[3] / count;
            for channel in 0..3 {
                let value = if sum[3] > 0.0 {
                    sum[channel] / sum[3]
                } else {
                    0.0
                };
                rgba.push(value.round() as u8);
            }
            rgba.push(alpha.round() as u8);
        }
    }
    CursorImage {
        width: size,
        height: size,
        hotspot: (size / 2, size / 2),
        rgba,
    }
}

pub fn svg() -> String {
    let center = SIZE / 2.0;
    let radius = center - STROKE / 2.0;
    let [red, green, blue, alpha] = FILL.to_array();
    let [ring_red, ring_green, ring_blue, _] = RING.to_array();
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{SIZE}' height='{SIZE}'>\
         <circle cx='{center}' cy='{center}' r='{radius}' \
         fill='rgb({red},{green},{blue})' fill-opacity='{opacity}' \
         stroke='rgb({ring_red},{ring_green},{ring_blue})' stroke-width='{STROKE}'/></svg>",
        opacity = f32::from(alpha) / 255.0,
    )
}

pub fn css(fallback: &str) -> String {
    let encoded = svg()
        .replace('%', "%25")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('#', "%23")
        .replace('"', "%22");
    let hotspot = SIZE / 2.0;
    format!("url(\"data:image/svg+xml,{encoded}\") {hotspot} {hotspot}, {fallback}")
}

pub fn paint(painter: &Painter, center: Pos2) -> Rect {
    let rect = Rect::from_center_size(center, Vec2::splat(SIZE));
    let inner = rect.shrink(STROKE);
    painter.rect_filled(inner, inner.width() / 2.0, FILL);
    painter.rect_stroke(
        rect.shrink(STROKE / 2.0),
        (SIZE - STROKE) / 2.0,
        STROKE,
        RING,
    );
    rect
}

#[cfg(test)]
mod tests;
