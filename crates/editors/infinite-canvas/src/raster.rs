use std::collections::BTreeSet;

use block_editor_beui::beui::icons::{
    Glyph, ICON_CIRCLE, ICON_ERASER, ICON_PAINT_BUCKET, ICON_PENCIL, ICON_PIPETTE, ICON_REPLACE,
    ICON_SLASH, ICON_SQUARE,
};
use infinite_canvas_types::CanvasPixel;

pub(crate) const MAX_BRUSH_SIZE: u16 = 64;

pub(crate) type Pixel = (i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PixelTool {
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
    ReplaceColor,
    Line,
    Rectangle,
    Ellipse,
}

impl PixelTool {
    pub(crate) const ALL: [Self; 8] = [
        Self::Pencil,
        Self::Eraser,
        Self::Fill,
        Self::Eyedropper,
        Self::ReplaceColor,
        Self::Line,
        Self::Rectangle,
        Self::Ellipse,
    ];

    pub(crate) const fn draws(self) -> bool {
        matches!(
            self,
            Self::Pencil | Self::Eraser | Self::Line | Self::Rectangle | Self::Ellipse
        )
    }

    pub(crate) const fn shapes(self) -> bool {
        matches!(self, Self::Rectangle | Self::Ellipse)
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Eraser => "Pixel eraser",
            Self::Fill => "Fill",
            Self::Eyedropper => "Eyedropper",
            Self::ReplaceColor => "Replace colour",
            Self::Line => "Pixel line",
            Self::Rectangle => "Pixel rectangle",
            Self::Ellipse => "Pixel ellipse",
        }
    }

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Pencil => "pencil",
            Self::Eraser => "pixel-eraser",
            Self::Fill => "fill",
            Self::Eyedropper => "eyedropper",
            Self::ReplaceColor => "replace-color",
            Self::Line => "pixel-line",
            Self::Rectangle => "pixel-rectangle",
            Self::Ellipse => "pixel-ellipse",
        }
    }

    pub(crate) const fn glyph(self) -> Glyph {
        match self {
            Self::Pencil => ICON_PENCIL,
            Self::Eraser => ICON_ERASER,
            Self::Fill => ICON_PAINT_BUCKET,
            Self::Eyedropper => ICON_PIPETTE,
            Self::ReplaceColor => ICON_REPLACE,
            Self::Line => ICON_SLASH,
            Self::Rectangle => ICON_SQUARE,
            Self::Ellipse => ICON_CIRCLE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BrushShape {
    Square,
    Circle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Brush {
    pub(crate) size: u16,
    pub(crate) shape: BrushShape,
    pub(crate) filled: bool,
    pub(crate) mirror_horizontal: bool,
    pub(crate) mirror_vertical: bool,
    pub(crate) constrained: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ActiveDrawing {
    pub(crate) tool: PixelTool,
    pub(crate) start: Pixel,
    pub(crate) end: Pixel,
    pub(crate) path: Vec<Pixel>,
}

impl ActiveDrawing {
    pub(crate) fn new(tool: PixelTool, pixel: Pixel) -> Self {
        Self {
            tool,
            start: pixel,
            end: pixel,
            path: vec![pixel],
        }
    }

    pub(crate) fn extend(&mut self, pixel: Pixel) {
        if self.end == pixel {
            return;
        }
        if matches!(self.tool, PixelTool::Pencil | PixelTool::Eraser) {
            self.path
                .extend(pixels_on_line(self.end, pixel).into_iter().skip(1));
        }
        self.end = pixel;
    }

    pub(crate) fn traces(&self) -> bool {
        matches!(self.tool, PixelTool::Pencil | PixelTool::Eraser)
    }
}

pub(crate) fn rasterize_drawing(
    drawing: &ActiveDrawing,
    width: u16,
    height: u16,
    brush: Brush,
) -> Vec<(u16, u16)> {
    let end = match brush.constrained
        && matches!(
            drawing.tool,
            PixelTool::Line | PixelTool::Rectangle | PixelTool::Ellipse
        ) {
        true => constrained_endpoint(drawing.start, drawing.end, drawing.tool),
        false => drawing.end,
    };
    let reach = i32::from(brush.size);
    let limits = Limits {
        left: -reach,
        top: -reach,
        right: i32::from(width) + reach,
        bottom: i32::from(height) + reach,
    };
    let filled = brush.filled && drawing.tool.shapes();
    let base = match drawing.tool {
        PixelTool::Pencil | PixelTool::Eraser => drawing.path.clone(),
        PixelTool::Line => pixels_on_line(drawing.start, end),
        PixelTool::Rectangle if filled => filled_rectangle(drawing.start, end, limits),
        PixelTool::Rectangle => rectangle_outline(drawing.start, end, limits),
        PixelTool::Ellipse => ellipse_pixels(drawing.start, end, filled, limits),
        PixelTool::Fill | PixelTool::Eyedropper | PixelTool::ReplaceColor => Vec::new(),
    };
    stamp(&base, width, height, brush, filled)
}

pub(crate) fn stamp(
    base: &[Pixel],
    width: u16,
    height: u16,
    brush: Brush,
    whole: bool,
) -> Vec<(u16, u16)> {
    let mut pixels = BTreeSet::new();
    for &center in base {
        match whole {
            true => insert_with_symmetry(&mut pixels, center, width, height, brush),
            false => {
                for pixel in brush_stamp(center, brush.size, brush.shape) {
                    insert_with_symmetry(&mut pixels, pixel, width, height, brush);
                }
            }
        }
    }
    pixels.into_iter().collect()
}

#[derive(Clone, Copy)]
struct Limits {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn constrained_endpoint(start: Pixel, end: Pixel, tool: PixelTool) -> Pixel {
    let delta_x = end.0 - start.0;
    let delta_y = end.1 - start.1;
    let (absolute_x, absolute_y) = (delta_x.abs(), delta_y.abs());
    if tool == PixelTool::Line {
        if absolute_x > absolute_y.saturating_mul(2) {
            return (end.0, start.1);
        }
        if absolute_y > absolute_x.saturating_mul(2) {
            return (start.0, end.1);
        }
    }
    let distance = absolute_x.max(absolute_y);
    let sign = |delta: i32| if delta < 0 { -1 } else { 1 };
    (
        start.0 + sign(delta_x) * distance,
        start.1 + sign(delta_y) * distance,
    )
}

fn brush_stamp(center: Pixel, size: u16, shape: BrushShape) -> Vec<Pixel> {
    let size = i32::from(size.max(1));
    let low = -((size - 1) / 2);
    let high = size / 2;
    let radius = f64::from(size) * 0.5 - 0.25;
    let middle = (f64::from(size) - 1.0) * 0.5;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for offset_y in low..=high {
        for offset_x in low..=high {
            if shape == BrushShape::Circle {
                let delta_x = f64::from(offset_x - low) - middle;
                let delta_y = f64::from(offset_y - low) - middle;
                if delta_x * delta_x + delta_y * delta_y > radius * radius {
                    continue;
                }
            }
            pixels.push((center.0 + offset_x, center.1 + offset_y));
        }
    }
    pixels
}

fn insert_with_symmetry(
    pixels: &mut BTreeSet<(u16, u16)>,
    pixel: Pixel,
    width: u16,
    height: u16,
    brush: Brush,
) {
    let (width, height) = (i32::from(width), i32::from(height));
    if pixel.0 < 0 || pixel.1 < 0 || pixel.0 >= width || pixel.1 >= height {
        return;
    }
    let (x, y) = (pixel.0 as u16, pixel.1 as u16);
    let mirrored_x = (width - 1 - pixel.0) as u16;
    let mirrored_y = (height - 1 - pixel.1) as u16;
    pixels.insert((x, y));
    if brush.mirror_horizontal {
        pixels.insert((mirrored_x, y));
    }
    if brush.mirror_vertical {
        pixels.insert((x, mirrored_y));
    }
    if brush.mirror_horizontal && brush.mirror_vertical {
        pixels.insert((mirrored_x, mirrored_y));
    }
}

fn ordered(first: i32, second: i32) -> (i32, i32) {
    (first.min(second), first.max(second))
}

fn filled_rectangle(start: Pixel, end: Pixel, limits: Limits) -> Vec<Pixel> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let (left, right) = (left.max(limits.left), right.min(limits.right));
    let (top, bottom) = (top.max(limits.top), bottom.min(limits.bottom));
    (top..=bottom)
        .flat_map(|y| (left..=right).map(move |x| (x, y)))
        .collect()
}

fn rectangle_outline(start: Pixel, end: Pixel, limits: Limits) -> Vec<Pixel> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut pixels = BTreeSet::new();
    for x in left.max(limits.left)..=right.min(limits.right) {
        pixels.insert((x, top));
        pixels.insert((x, bottom));
    }
    for y in top.max(limits.top)..=bottom.min(limits.bottom) {
        pixels.insert((left, y));
        pixels.insert((right, y));
    }
    pixels.into_iter().collect()
}

fn ellipse_pixels(start: Pixel, end: Pixel, filled: bool, limits: Limits) -> Vec<Pixel> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut interior = BTreeSet::new();
    for y in top.max(limits.top - 1)..=bottom.min(limits.bottom + 1) {
        for x in left.max(limits.left - 1)..=right.min(limits.right + 1) {
            if ellipse_contains(x, y, left, right, top, bottom) {
                interior.insert((x, y));
            }
        }
    }
    let center_x = (left + right).div_euclid(2);
    let center_y = (top + bottom).div_euclid(2);
    for x in center_x..=(left + right + 1).div_euclid(2) {
        interior.insert((x, top));
        interior.insert((x, bottom));
    }
    for y in center_y..=(top + bottom + 1).div_euclid(2) {
        interior.insert((left, y));
        interior.insert((right, y));
    }
    if filled {
        return interior.into_iter().collect();
    }
    interior
        .iter()
        .copied()
        .filter(|(x, y)| {
            [(*x - 1, *y), (*x + 1, *y), (*x, *y - 1), (*x, *y + 1)]
                .into_iter()
                .any(|neighbor| !interior.contains(&neighbor))
        })
        .collect()
}

fn ellipse_contains(x: i32, y: i32, left: i32, right: i32, top: i32, bottom: i32) -> bool {
    if left == right {
        return x == left;
    }
    if top == bottom {
        return y == top;
    }
    let center_x = (f64::from(left) + f64::from(right)) * 0.5;
    let center_y = (f64::from(top) + f64::from(bottom)) * 0.5;
    let radius_x = f64::from(right - left) * 0.5;
    let radius_y = f64::from(bottom - top) * 0.5;
    let normalized_x = (f64::from(x) - center_x) / radius_x;
    let normalized_y = (f64::from(y) - center_y) / radius_y;
    normalized_x * normalized_x + normalized_y * normalized_y <= 1.0 + f64::EPSILON
}

pub(crate) fn pixels_on_line(start: Pixel, end: Pixel) -> Vec<Pixel> {
    let (mut x, mut y) = start;
    let delta_x = (end.0 - x).abs();
    let step_x = if x < end.0 { 1 } else { -1 };
    let delta_y = -(end.1 - y).abs();
    let step_y = if y < end.1 { 1 } else { -1 };
    let mut error = delta_x + delta_y;
    let mut pixels = Vec::new();
    loop {
        pixels.push((x, y));
        if (x, y) == end {
            break;
        }
        let doubled = error * 2;
        if doubled >= delta_y {
            error += delta_y;
            x += step_x;
        }
        if doubled <= delta_x {
            error += delta_x;
            y += step_y;
        }
    }
    pixels
}

pub(crate) fn format_hex_color(color: CanvasPixel) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue, color.alpha
    )
}

pub(crate) fn parse_hex_color(value: &str) -> Option<CanvasPixel> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if !value.is_ascii() || !matches!(value.len(), 6 | 8) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&value[at..at + 2], 16).ok();
    Some(CanvasPixel::new(
        channel(0)?,
        channel(2)?,
        channel(4)?,
        match value.len() {
            8 => channel(6)?,
            _ => 255,
        },
    ))
}

pub(crate) fn nearest_upscaled(width: u16, height: u16, rgba: &[u8], factor: u32) -> Vec<u8> {
    let factor = factor.max(1) as usize;
    let (width, height) = (usize::from(width), usize::from(height));
    let mut out = Vec::with_capacity(width * height * factor * factor * 4);
    for y in 0..height {
        let row = &rgba[y * width * 4..(y + 1) * width * 4];
        let mut scaled = Vec::with_capacity(width * factor * 4);
        for pixel in row.chunks_exact(4) {
            for _ in 0..factor {
                scaled.extend_from_slice(pixel);
            }
        }
        for _ in 0..factor {
            out.extend_from_slice(&scaled);
        }
    }
    out
}
