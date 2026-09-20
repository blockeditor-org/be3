use block_client::blocks::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_PALETTE, PIXEL_RAY_TRACER_SIZE, Point, RayEntity,
};
use block_editor_plugin::beui::{Color32, Image};

pub(crate) const OVERLAY_SCALE: u32 = 4;
pub(crate) const OVERLAY_SIZE: u32 = PIXEL_RAY_TRACER_SIZE as u32 * OVERLAY_SCALE;

const SURFACE_WIDTH: f32 = 1.5;
const WATER_FILL: Color32 = Color32::from_rgba_unmultiplied(41, 173, 255, 60);
const WATER_EDGE: Color32 = Color32::from_rgb(100, 220, 255);
const LIGHT_RADIUS: f32 = 2.5;
const HANDLE_RADIUS: f32 = 1.6;
const SELECTION: Color32 = Color32::from_rgb(255, 255, 0);
const PREVIEW_WIDTH: f32 = 2.0;

pub(crate) fn palette_color(index: u8) -> Color32 {
    let rgb = PIXEL_RAY_TRACER_PALETTE[usize::from(index)];
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

pub(crate) enum Preview {
    None,
    Pixels(Vec<(u16, u16)>, u8),
    Surface(Point, Point, u8),
    Water(Point, Point),
}

pub(crate) struct Canvas {
    pixels: Vec<u8>,
}

impl Canvas {
    fn new() -> Self {
        Self {
            pixels: vec![0; (OVERLAY_SIZE * OVERLAY_SIZE * 4) as usize],
        }
    }

    fn blend(&mut self, x: i32, y: i32, color: Color32, coverage: f32) {
        if x < 0 || y < 0 || x >= OVERLAY_SIZE as i32 || y >= OVERLAY_SIZE as i32 {
            return;
        }
        let [red, green, blue, alpha] = color.to_array();
        let alpha = f32::from(alpha) / 255.0 * coverage.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        let start = ((y as u32 * OVERLAY_SIZE + x as u32) * 4) as usize;
        let held = &mut self.pixels[start..start + 4];
        let mix = |over: u8, under: u8| {
            (f32::from(over) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8
        };
        held[0] = mix(red, held[0]);
        held[1] = mix(green, held[1]);
        held[2] = mix(blue, held[2]);
        held[3] = ((alpha + f32::from(held[3]) / 255.0 * (1.0 - alpha)) * 255.0).round() as u8;
    }

    fn segment(&mut self, from: Point, to: Point, width: f32, color: Color32) {
        let from = scaled(from);
        let to = scaled(to);
        let half = (width * OVERLAY_SCALE as f32 / 2.0).max(0.6);
        let left = (from.0.min(to.0) - half - 1.0).floor() as i32;
        let right = (from.0.max(to.0) + half + 1.0).ceil() as i32;
        let top = (from.1.min(to.1) - half - 1.0).floor() as i32;
        let bottom = (from.1.max(to.1) + half + 1.0).ceil() as i32;
        for y in top..=bottom {
            for x in left..=right {
                let distance = distance_to_segment((x as f32 + 0.5, y as f32 + 0.5), from, to);
                self.blend(x, y, color, (half + 0.5 - distance).clamp(0.0, 1.0));
            }
        }
    }

    fn disc(&mut self, at: Point, radius: f32, color: Color32) {
        let at = scaled(at);
        let radius = radius * OVERLAY_SCALE as f32;
        let left = (at.0 - radius - 1.0).floor() as i32;
        let right = (at.0 + radius + 1.0).ceil() as i32;
        let top = (at.1 - radius - 1.0).floor() as i32;
        let bottom = (at.1 + radius + 1.0).ceil() as i32;
        for y in top..=bottom {
            for x in left..=right {
                let distance =
                    ((x as f32 + 0.5 - at.0).powi(2) + (y as f32 + 0.5 - at.1).powi(2)).sqrt();
                self.blend(x, y, color, (radius + 0.5 - distance).clamp(0.0, 1.0));
            }
        }
    }

    fn ring(&mut self, at: Point, radius: f32, width: f32, color: Color32) {
        let at = scaled(at);
        let radius = radius * OVERLAY_SCALE as f32;
        let half = (width * OVERLAY_SCALE as f32 / 2.0).max(0.6);
        let reach = radius + half + 1.0;
        for y in (at.1 - reach).floor() as i32..=(at.1 + reach).ceil() as i32 {
            for x in (at.0 - reach).floor() as i32..=(at.0 + reach).ceil() as i32 {
                let distance =
                    ((x as f32 + 0.5 - at.0).powi(2) + (y as f32 + 0.5 - at.1).powi(2)).sqrt();
                let edge = (distance - radius).abs();
                self.blend(x, y, color, (half + 0.5 - edge).clamp(0.0, 1.0));
            }
        }
    }

    fn rect(&mut self, start: Point, end: Point, fill: Color32, outline: Option<Color32>) {
        let (left, right) = (start.x.min(end.x), start.x.max(end.x));
        let (top, bottom) = (start.y.min(end.y), start.y.max(end.y));
        let min = scaled(Point::new(left, top));
        let max = scaled(Point::new(right, bottom));
        for y in min.1.floor() as i32..max.1.ceil() as i32 {
            for x in min.0.floor() as i32..max.0.ceil() as i32 {
                self.blend(x, y, fill, 1.0);
            }
        }
        let Some(outline) = outline else {
            return;
        };
        self.segment(Point::new(left, top), Point::new(right, top), 0.5, outline);
        self.segment(
            Point::new(right, top),
            Point::new(right, bottom),
            0.5,
            outline,
        );
        self.segment(
            Point::new(right, bottom),
            Point::new(left, bottom),
            0.5,
            outline,
        );
        self.segment(
            Point::new(left, bottom),
            Point::new(left, top),
            0.5,
            outline,
        );
    }

    fn cell(&mut self, pixel: (u16, u16), color: Color32) {
        let start = Point::new(f32::from(pixel.0), f32::from(pixel.1));
        self.rect(start, Point::new(start.x + 1.0, start.y + 1.0), color, None);
    }
}

pub(crate) fn draw(entities: &[RayEntity], selected: Option<u64>, preview: &Preview) -> Image {
    let mut canvas = Canvas::new();
    for entity in entities {
        match entity {
            RayEntity::Surface {
                start,
                end,
                color_index,
                ..
            } => canvas.segment(*start, *end, SURFACE_WIDTH, palette_color(*color_index)),
            RayEntity::Water { start, end, .. } => {
                canvas.rect(*start, *end, WATER_FILL, Some(WATER_EDGE));
            }
            RayEntity::Light {
                position,
                color_index,
                ..
            } => {
                canvas.disc(*position, LIGHT_RADIUS, palette_color(*color_index));
                canvas.ring(*position, LIGHT_RADIUS, 0.4, Color32::WHITE);
            }
        }
        if selected != Some(entity.id()) {
            continue;
        }
        match entity {
            RayEntity::Light { position, .. } => {
                canvas.ring(*position, LIGHT_RADIUS + 1.2, 0.6, SELECTION);
            }
            RayEntity::Surface { start, end, .. } | RayEntity::Water { start, end, .. } => {
                canvas.disc(*start, HANDLE_RADIUS, Color32::WHITE);
                canvas.disc(*end, HANDLE_RADIUS, Color32::WHITE);
            }
        }
    }
    match preview {
        Preview::None => {}
        Preview::Pixels(points, color_index) => {
            let color = palette_color(*color_index);
            for point in points {
                canvas.cell(*point, color);
            }
        }
        Preview::Surface(start, end, color_index) => {
            canvas.segment(*start, *end, PREVIEW_WIDTH, palette_color(*color_index));
        }
        Preview::Water(start, end) => canvas.rect(*start, *end, WATER_FILL, None),
    }
    Image::from_rgba(OVERLAY_SIZE, OVERLAY_SIZE, canvas.pixels)
}

fn scaled(point: Point) -> (f32, f32) {
    (
        point.x * OVERLAY_SCALE as f32,
        point.y * OVERLAY_SCALE as f32,
    )
}

fn distance_to_segment(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let length = (end.0 - start.0).powi(2) + (end.1 - start.1).powi(2);
    if length == 0.0 {
        return ((point.0 - start.0).powi(2) + (point.1 - start.1).powi(2)).sqrt();
    }
    let t = (((point.0 - start.0) * (end.0 - start.0) + (point.1 - start.1) * (end.1 - start.1))
        / length)
        .clamp(0.0, 1.0);
    let closest = (
        start.0 + t * (end.0 - start.0),
        start.1 + t * (end.1 - start.1),
    );
    ((point.0 - closest.0).powi(2) + (point.1 - closest.1).powi(2)).sqrt()
}

#[cfg(test)]
mod tests;
