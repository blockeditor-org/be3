use be_model::{Bounds, Change, Grid, ObjectId};
use serde::{Deserialize, Serialize};

use crate::canvas::{CanvasPoint, CanvasTransform, Entity};

pub const DEFAULT_RASTER_SIZE: u16 = 32;
pub const MAX_RASTER_SIZE: u16 = 2048;
pub const MAX_RASTER_PALETTE_COLORS: usize = 32;

#[derive(Clone, Copy, Debug, Default, Deserialize, Hash, PartialEq, Eq, Serialize)]
pub struct CanvasPixel {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl CanvasPixel {
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn rgba(self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }

    pub const fn from_rgba([red, green, blue, alpha]: [u8; 4]) -> Self {
        Self::new(red, green, blue, alpha)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CanvasPixelUpdate {
    pub x: u16,
    pub y: u16,
    pub color: CanvasPixel,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasRasterAnchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasRaster {
    pixels: Grid<[u8; 4]>,
}

impl CanvasRaster {
    pub(crate) fn of(pixels: &Grid<[u8; 4]>) -> Option<Self> {
        (pixels.bounds().area() > 0).then(|| Self {
            pixels: pixels.clone(),
        })
    }

    pub fn from_rgba(width: u16, height: u16, rgba: &[u8]) -> Option<Self> {
        if !valid_dimension(width)
            || !valid_dimension(height)
            || rgba.len() != usize::from(width) * usize::from(height) * 4
        {
            return None;
        }
        let mut pixels = Grid::new(Bounds::new(0, 0, u32::from(width), u32::from(height)));
        let mut rows = rgba.as_chunks::<4>().0.chunks(usize::from(width));
        for y in 0..i32::from(height) {
            for (x, pixel) in (0..).zip(rows.next().unwrap_or_default()) {
                pixels.set(x, y, *pixel);
            }
        }
        Some(Self { pixels })
    }

    pub(crate) fn grid(&self) -> &Grid<[u8; 4]> {
        &self.pixels
    }

    pub fn width(&self) -> u16 {
        u16::try_from(self.pixels.bounds().width).unwrap_or(MAX_RASTER_SIZE)
    }

    pub fn height(&self) -> u16 {
        u16::try_from(self.pixels.bounds().height).unwrap_or(MAX_RASTER_SIZE)
    }

    pub fn rgba_bytes(&self) -> &[u8] {
        self.pixels.bytes()
    }

    pub fn pixel(&self, x: u16, y: u16) -> Option<CanvasPixel> {
        let (at_x, at_y) = self.at(x, y);
        (x < self.width() && y < self.height())
            .then(|| self.pixels.get(at_x, at_y))
            .flatten()
            .map(CanvasPixel::from_rgba)
    }

    pub(crate) fn bounds(&self) -> Bounds {
        self.pixels.bounds()
    }

    fn at(&self, x: u16, y: u16) -> (i32, i32) {
        let bounds = self.pixels.bounds();
        (
            bounds.left.saturating_add(i32::from(x)),
            bounds.top.saturating_add(i32::from(y)),
        )
    }
}

impl CanvasTransform {
    pub fn raster(center: CanvasPoint, width: u16, height: u16) -> Self {
        let size = CanvasPoint::new(f32::from(width), f32::from(height));
        let corner = CanvasPoint::new(
            (center.x - size.x * 0.5).round(),
            (center.y - size.y * 0.5).round(),
        );
        Self::new(
            CanvasPoint::new(corner.x + size.x * 0.5, corner.y + size.y * 0.5),
            size,
            0.0,
        )
    }

    pub fn pixel_at(self, width: u16, height: u16, world: CanvasPoint) -> (i32, i32) {
        let local = self.to_local(world);
        let pixel = |local: f32, count: u16| {
            ((local + 0.5) * f32::from(count))
                .floor()
                .clamp(i32::MIN as f32, i32::MAX as f32) as i32
        };
        (pixel(local.x, width), pixel(local.y, height))
    }
}

pub fn raster_dimensions(size: CanvasPoint) -> (u16, u16) {
    let count = |extent: f32| match extent.is_finite() {
        true => extent.abs().round().clamp(1.0, f32::from(MAX_RASTER_SIZE)) as u16,
        false => DEFAULT_RASTER_SIZE,
    };
    (count(size.x), count(size.y))
}

pub fn default_raster_palette() -> Vec<CanvasPixel> {
    [
        CanvasPixel::TRANSPARENT,
        CanvasPixel::new(0, 0, 0, 255),
        CanvasPixel::new(255, 255, 255, 255),
        CanvasPixel::new(128, 128, 128, 255),
        CanvasPixel::new(192, 192, 192, 255),
        CanvasPixel::new(136, 57, 50, 255),
        CanvasPixel::new(237, 118, 20, 255),
        CanvasPixel::new(255, 215, 0, 255),
        CanvasPixel::new(51, 160, 44, 255),
        CanvasPixel::new(40, 200, 170, 255),
        CanvasPixel::new(40, 110, 220, 255),
        CanvasPixel::new(95, 60, 190, 255),
        CanvasPixel::new(190, 60, 190, 255),
        CanvasPixel::new(230, 90, 120, 255),
        CanvasPixel::new(115, 75, 45, 255),
        CanvasPixel::new(242, 194, 156, 255),
    ]
    .to_vec()
}

pub(crate) fn normalized_palette(colors: &[CanvasPixel]) -> Vec<CanvasPixel> {
    let mut palette = Vec::with_capacity(colors.len().min(MAX_RASTER_PALETTE_COLORS));
    for &color in colors {
        if palette.len() == MAX_RASTER_PALETTE_COLORS {
            break;
        }
        if !palette.contains(&color) {
            palette.push(color);
        }
    }
    palette
}

pub(crate) type Cells = Vec<(i32, i32, [u8; 4])>;

pub(crate) fn paint(object: ObjectId, cells: Cells) -> Option<Change> {
    (!cells.is_empty()).then(|| Entity::PIXELS.paint(object, cells))
}

pub(crate) fn painted(raster: &CanvasRaster, pixels: &[CanvasPixelUpdate]) -> Cells {
    pixels
        .iter()
        .filter(|update| {
            raster
                .pixel(update.x, update.y)
                .is_some_and(|held| held != update.color)
        })
        .map(|update| {
            let (x, y) = raster.at(update.x, update.y);
            (x, y, update.color.rgba())
        })
        .collect()
}

pub(crate) fn recolored(
    raster: &CanvasRaster,
    change: impl Fn(CanvasPixel) -> Option<CanvasPixel>,
) -> Cells {
    let mut cells = Vec::new();
    for y in 0..raster.height() {
        for x in 0..raster.width() {
            let Some(color) = raster.pixel(x, y).and_then(&change) else {
                continue;
            };
            let (at_x, at_y) = raster.at(x, y);
            cells.push((at_x, at_y, color.rgba()));
        }
    }
    cells
}

pub(crate) fn filled(raster: &CanvasRaster, x: u16, y: u16, color: CanvasPixel) -> Cells {
    let Some(target) = raster.pixel(x, y) else {
        return Vec::new();
    };
    if target == color {
        return Vec::new();
    }
    let (width, height) = (raster.width(), raster.height());
    let mut seen = vec![false; usize::from(width) * usize::from(height)];
    let mut pending = vec![(x, y)];
    let mut cells = Vec::new();
    seen[usize::from(y) * usize::from(width) + usize::from(x)] = true;
    while let Some((pixel_x, pixel_y)) = pending.pop() {
        let (at_x, at_y) = raster.at(pixel_x, pixel_y);
        cells.push((at_x, at_y, color.rgba()));
        let neighbors = [
            pixel_x.checked_sub(1).map(|left| (left, pixel_y)),
            (pixel_x + 1 < width).then_some((pixel_x + 1, pixel_y)),
            pixel_y.checked_sub(1).map(|up| (pixel_x, up)),
            (pixel_y + 1 < height).then_some((pixel_x, pixel_y + 1)),
        ];
        for (next_x, next_y) in neighbors.into_iter().flatten() {
            let index = usize::from(next_y) * usize::from(width) + usize::from(next_x);
            if !seen[index] && raster.pixel(next_x, next_y) == Some(target) {
                seen[index] = true;
                pending.push((next_x, next_y));
            }
        }
    }
    cells
}

pub(crate) fn resized(
    bounds: Bounds,
    width: u16,
    height: u16,
    anchor: CanvasRasterAnchor,
) -> Bounds {
    let (old_width, old_height) = (
        u16::try_from(bounds.width).unwrap_or(MAX_RASTER_SIZE),
        u16::try_from(bounds.height).unwrap_or(MAX_RASTER_SIZE),
    );
    let (source_x, destination_x) = aligned_offsets(
        old_width,
        width,
        old_width.min(width),
        horizontal_alignment(anchor),
    );
    let (source_y, destination_y) = aligned_offsets(
        old_height,
        height,
        old_height.min(height),
        vertical_alignment(anchor),
    );
    Bounds::new(
        bounds.left + i32::from(source_x) - i32::from(destination_x),
        bounds.top + i32::from(source_y) - i32::from(destination_y),
        u32::from(width),
        u32::from(height),
    )
}

#[derive(Clone, Copy)]
enum Alignment {
    Start,
    Center,
    End,
}

fn horizontal_alignment(anchor: CanvasRasterAnchor) -> Alignment {
    match anchor {
        CanvasRasterAnchor::TopLeft | CanvasRasterAnchor::Left | CanvasRasterAnchor::BottomLeft => {
            Alignment::Start
        }
        CanvasRasterAnchor::Top | CanvasRasterAnchor::Center | CanvasRasterAnchor::Bottom => {
            Alignment::Center
        }
        CanvasRasterAnchor::TopRight
        | CanvasRasterAnchor::Right
        | CanvasRasterAnchor::BottomRight => Alignment::End,
    }
}

fn vertical_alignment(anchor: CanvasRasterAnchor) -> Alignment {
    match anchor {
        CanvasRasterAnchor::TopLeft | CanvasRasterAnchor::Top | CanvasRasterAnchor::TopRight => {
            Alignment::Start
        }
        CanvasRasterAnchor::Left | CanvasRasterAnchor::Center | CanvasRasterAnchor::Right => {
            Alignment::Center
        }
        CanvasRasterAnchor::BottomLeft
        | CanvasRasterAnchor::Bottom
        | CanvasRasterAnchor::BottomRight => Alignment::End,
    }
}

fn aligned_offsets(old: u16, new: u16, overlap: u16, alignment: Alignment) -> (u16, u16) {
    match alignment {
        Alignment::Start => (0, 0),
        Alignment::Center => ((old - overlap) / 2, (new - overlap) / 2),
        Alignment::End => (old - overlap, new - overlap),
    }
}

pub(crate) const fn valid_dimension(value: u16) -> bool {
    value >= 1 && value <= MAX_RASTER_SIZE
}
