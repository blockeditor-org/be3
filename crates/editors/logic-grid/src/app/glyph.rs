use std::cell::RefCell;
use std::collections::HashMap;

use beui::Image;

use super::*;

pub(super) const GLYPH_WIDTH: f32 = 56.0;
pub(super) const GLYPH_HEIGHT: f32 = 37.0;
const DENSITY: f32 = 3.0;
const SAMPLES: usize = 4;
const LOCKED_COLOR: [f32; 4] = [0.376, 0.376, 0.376, 1.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Glyph {
    Tool(ToolKind),
    Locked,
    Folder,
    Component,
}

impl Glyph {
    pub(super) fn of(slot: &HotbarSlot) -> Self {
        match slot {
            HotbarSlot::Builtin(kind) => Self::Tool(*kind),
            HotbarSlot::Locked { .. } => Self::Locked,
            HotbarSlot::Folder { .. } => Self::Folder,
            HotbarSlot::Component { .. } => Self::Component,
        }
    }

    pub(super) fn image(self) -> Image {
        thread_local! {
            static IMAGES: RefCell<HashMap<Glyph, Image>> = RefCell::new(HashMap::new());
        }
        IMAGES.with(|images| {
            images
                .borrow_mut()
                .entry(self)
                .or_insert_with(|| self.paint().image())
                .clone()
        })
    }

    fn paint(self) -> Raster {
        let mut raster = Raster::new();
        let gate = DrawTriangle::GATE_COLOR;
        match self {
            Self::Tool(ToolKind::Select) => {
                raster.triangle([[0.3, 0.15], [0.3, 0.78], [0.46, 0.6]], gate);
                raster.triangle([[0.3, 0.15], [0.46, 0.6], [0.62, 0.55]], gate);
            }
            Self::Tool(ToolKind::Wire) => {
                raster.line([0.15, 0.85], [0.85, 0.15], 3.0, DrawTriangle::WIRE_COLOR);
            }
            Self::Tool(ToolKind::Not) => {
                raster.glyph_box(gate);
                raster.triangle([[0.12, 0.82], [0.88, 0.82], [0.5, 0.12]], gate);
            }
            Self::Tool(ToolKind::MergerSplitter) => {
                raster.glyph_box(gate);
                raster.triangle([[0.12, 0.82], [0.88, 0.82], [0.62, 0.5]], gate);
                raster.triangle([[0.12, 0.18], [0.62, 0.5], [0.88, 0.18]], gate);
            }
            Self::Tool(ToolKind::Led) => {
                raster.glyph_box(gate);
                raster.diamond([0.5, 0.5], 0.3, gate);
            }
            Self::Tool(ToolKind::Storage) => {
                raster.glyph_box(gate);
                raster.rect_stroke([0.22, 0.18, 0.78, 0.82], 1.5, gate);
            }
            Self::Tool(ToolKind::ConfigureStorage) => {
                raster.glyph_box(gate);
                raster.rect_stroke([0.22, 0.18, 0.78, 0.82], 1.5, gate);
                raster.diamond([0.5, 0.5], 0.12, DrawTriangle::HIGHLIGHT_COLOR);
            }
            Self::Tool(kind @ (ToolKind::Input | ToolKind::Output)) => {
                let accent = match kind {
                    ToolKind::Output => DrawTriangle::OUTPUT_COLOR,
                    _ => DrawTriangle::INPUT_COLOR,
                };
                raster.glyph_box(gate);
                raster.triangle([[0.28, 0.52], [0.72, 0.52], [0.5, 0.2]], accent);
                raster.rect_fill([0.42, 0.52, 0.58, 0.82], accent);
            }
            Self::Tool(ToolKind::Custom) => raster.glyph_box(gate),
            Self::Locked => {
                raster.glyph_box(LOCKED_COLOR);
                raster.rect_fill([0.32, 0.42, 0.68, 0.78], LOCKED_COLOR);
                raster.ring([0.5, 0.42], GLYPH_WIDTH * 0.13, 2.0, LOCKED_COLOR);
            }
            Self::Folder => {
                let folder = DrawTriangle::HIGHLIGHT_COLOR;
                raster.rect_fill([0.12, 0.3, 0.88, 0.78], folder);
                raster.rect_fill([0.18, 0.2, 0.52, 0.38], folder);
            }
            Self::Component => {
                raster.glyph_box(gate);
                for center in [[0.5, 0.08], [0.5, 0.92]] {
                    raster.disc(center, 2.5, DrawTriangle::WIRE_COLOR);
                }
            }
        }
        raster
    }
}

struct Raster {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Raster {
    fn new() -> Self {
        let width = (GLYPH_WIDTH * DENSITY) as usize;
        let height = (GLYPH_HEIGHT * DENSITY) as usize;
        Self {
            width,
            height,
            pixels: vec![[0.0; 4]; width * height],
        }
    }

    fn point(&self, at: [f32; 2]) -> [f32; 2] {
        [at[0] * GLYPH_WIDTH, at[1] * GLYPH_HEIGHT]
    }

    fn glyph_box(&mut self, color: [f32; 4]) {
        self.rect_stroke([0.08, 0.08, 0.92, 0.92], 1.5, color);
    }

    fn triangle(&mut self, points: [[f32; 2]; 3], color: [f32; 4]) {
        let points = points.map(|point| self.point(point));
        self.fill(color, move |at| {
            let side = |a: [f32; 2], b: [f32; 2]| {
                (b[0] - a[0]) * (at[1] - a[1]) - (b[1] - a[1]) * (at[0] - a[0])
            };
            let sides = [
                side(points[0], points[1]),
                side(points[1], points[2]),
                side(points[2], points[0]),
            ];
            sides.iter().all(|side| *side >= 0.0) || sides.iter().all(|side| *side <= 0.0)
        });
    }

    fn diamond(&mut self, center: [f32; 2], radius: f32, color: [f32; 4]) {
        let center = self.point(center);
        let (horizontal, vertical) = (radius * GLYPH_WIDTH, radius * GLYPH_HEIGHT);
        self.fill(color, move |at| {
            (at[0] - center[0]).abs() / horizontal + (at[1] - center[1]).abs() / vertical <= 1.0
        });
    }

    fn rect_fill(&mut self, rect: [f32; 4], color: [f32; 4]) {
        let [left, top] = self.point([rect[0], rect[1]]);
        let [right, bottom] = self.point([rect[2], rect[3]]);
        self.fill(color, move |at| {
            at[0] >= left && at[0] <= right && at[1] >= top && at[1] <= bottom
        });
    }

    fn rect_stroke(&mut self, rect: [f32; 4], width: f32, color: [f32; 4]) {
        let [left, top] = self.point([rect[0], rect[1]]);
        let [right, bottom] = self.point([rect[2], rect[3]]);
        self.fill(color, move |at| {
            let inside = at[0] >= left && at[0] <= right && at[1] >= top && at[1] <= bottom;
            let inner = at[0] >= left + width
                && at[0] <= right - width
                && at[1] >= top + width
                && at[1] <= bottom - width;
            inside && !inner
        });
    }

    fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: [f32; 4]) {
        let from = self.point(from);
        let to = self.point(to);
        let direction = [to[0] - from[0], to[1] - from[1]];
        let length = direction[0] * direction[0] + direction[1] * direction[1];
        self.fill(color, move |at| {
            let along = ((at[0] - from[0]) * direction[0] + (at[1] - from[1]) * direction[1])
                / length.max(f32::EPSILON);
            let along = along.clamp(0.0, 1.0);
            let nearest = [
                from[0] + direction[0] * along,
                from[1] + direction[1] * along,
            ];
            (at[0] - nearest[0]).hypot(at[1] - nearest[1]) <= width * 0.5
        });
    }

    fn disc(&mut self, center: [f32; 2], radius: f32, color: [f32; 4]) {
        let center = self.point(center);
        self.fill(color, move |at| {
            (at[0] - center[0]).hypot(at[1] - center[1]) <= radius
        });
    }

    fn ring(&mut self, center: [f32; 2], radius: f32, width: f32, color: [f32; 4]) {
        let center = self.point(center);
        self.fill(color, move |at| {
            ((at[0] - center[0]).hypot(at[1] - center[1]) - radius).abs() <= width * 0.5
        });
    }

    fn fill(&mut self, color: [f32; 4], inside: impl Fn([f32; 2]) -> bool) {
        let step = 1.0 / (DENSITY * SAMPLES as f32);
        for y in 0..self.height {
            for x in 0..self.width {
                let mut covered = 0;
                for sample_y in 0..SAMPLES {
                    for sample_x in 0..SAMPLES {
                        let at = [
                            x as f32 / DENSITY + (sample_x as f32 + 0.5) * step,
                            y as f32 / DENSITY + (sample_y as f32 + 0.5) * step,
                        ];
                        covered += usize::from(inside(at));
                    }
                }
                if covered == 0 {
                    continue;
                }
                let alpha = color[3] * covered as f32 / (SAMPLES * SAMPLES) as f32;
                let pixel = &mut self.pixels[y * self.width + x];
                let kept = pixel[3] * (1.0 - alpha);
                let total = alpha + kept;
                for channel in 0..3 {
                    pixel[channel] = (color[channel] * alpha + pixel[channel] * kept) / total;
                }
                pixel[3] = total;
            }
        }
    }

    fn image(&self) -> Image {
        let bytes = self
            .pixels
            .iter()
            .flat_map(|pixel| pixel.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect();
        Image::from_rgba(self.width as u32, self.height as u32, bytes)
    }
}
