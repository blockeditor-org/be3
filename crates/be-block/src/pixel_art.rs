use be_model::{Bounds, Change, Document, Edit, Grid, Model, ObjectId, Value};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

pub const DEFAULT_PIXEL_ART_SIZE: u16 = 32;
pub const MAX_PIXEL_ART_SIZE: u16 = 2048;
pub const MAX_PIXEL_ART_PALETTE_COLORS: usize = 32;

#[derive(Clone, Copy, Debug, Default, Deserialize, Hash, PartialEq, Eq, Serialize)]
pub struct PixelColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl PixelColor {
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
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PixelUpdate {
    pub x: u16,
    pub y: u16,
    pub color: PixelColor,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelArtAnchor {
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum PixelArtOperation {
    Paint {
        pixels: Vec<PixelUpdate>,
    },
    Fill {
        x: u16,
        y: u16,
        color: PixelColor,
    },
    ReplaceColor {
        from: PixelColor,
        to: PixelColor,
    },
    SetPalette {
        colors: Vec<PixelColor>,
    },
    Clear,
    Resize {
        width: u16,
        height: u16,
        anchor: PixelArtAnchor,
    },
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct PixelArtDocument {
    pub pixels: Grid<[u8; 4]>,
    pub palette: Option<Vec<PixelColor>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artwork {
    bounds: Bounds,
    bytes: Vec<u8>,
    palette: Vec<PixelColor>,
}

impl Artwork {
    pub fn width(&self) -> u16 {
        u16::try_from(self.bounds.width).unwrap_or(MAX_PIXEL_ART_SIZE)
    }

    pub fn height(&self) -> u16 {
        u16::try_from(self.bounds.height).unwrap_or(MAX_PIXEL_ART_SIZE)
    }

    pub fn rgba_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn palette(&self) -> &[PixelColor] {
        &self.palette
    }

    pub fn pixel(&self, x: u16, y: u16) -> Option<PixelColor> {
        if u32::from(x) >= self.bounds.width || u32::from(y) >= self.bounds.height {
            return None;
        }
        let offset = (usize::from(y) * self.bounds.width as usize + usize::from(x)) * 4;
        let [red, green, blue, alpha] = self.bytes.get(offset..offset + 4)?.try_into().ok()?;
        Some(PixelColor::new(red, green, blue, alpha))
    }

    fn at(&self, x: u16, y: u16) -> (i32, i32) {
        (
            self.bounds.left.saturating_add(i32::from(x)),
            self.bounds.top.saturating_add(i32::from(y)),
        )
    }
}

impl PixelArtDocument {
    pub fn new() -> Self {
        Self {
            pixels: Grid::new(default_bounds()),
            palette: Some(default_palette()),
        }
    }

    pub fn artwork(&self) -> Artwork {
        let (bounds, bytes) = match self.pixels.bounds().area() {
            0 => (default_bounds(), vec![0; default_bounds().area() * 4]),
            _ => (self.pixels.bounds(), self.pixels.bytes().to_vec()),
        };
        Artwork {
            bounds,
            bytes,
            palette: self.palette.clone().unwrap_or_else(default_palette),
        }
    }

    pub fn edit_for(&self, operation: &PixelArtOperation) -> Edit {
        let art = self.artwork();
        let mut changes = Vec::new();
        if self.pixels.bounds().area() == 0 {
            changes.push(Self::PIXELS.reshape(ObjectId::ROOT, art.bounds));
        }
        match operation {
            PixelArtOperation::Paint { pixels } => {
                let cells: Vec<(i32, i32, [u8; 4])> = pixels
                    .iter()
                    .filter(|update| {
                        art.pixel(update.x, update.y)
                            .is_some_and(|held| held != update.color)
                    })
                    .map(|update| {
                        let (x, y) = art.at(update.x, update.y);
                        (x, y, update.color.rgba())
                    })
                    .collect();
                changes.extend(paint(cells));
            }
            PixelArtOperation::Fill { x, y, color } => {
                changes.extend(paint(fill(&art, *x, *y, *color)))
            }
            PixelArtOperation::ReplaceColor { from, to } => {
                if from != to {
                    changes.extend(paint(recolor(&art, |held| (held == *from).then_some(*to))));
                }
            }
            PixelArtOperation::Clear => changes.extend(paint(recolor(&art, |held| {
                (held != PixelColor::TRANSPARENT).then_some(PixelColor::TRANSPARENT)
            }))),
            PixelArtOperation::SetPalette { colors } => {
                let colors = normalized_palette(colors);
                if colors != art.palette {
                    changes.push(Self::PALETTE.set(ObjectId::ROOT, &Some(colors)));
                }
            }
            PixelArtOperation::Resize {
                width,
                height,
                anchor,
            } => {
                if valid_dimension(*width)
                    && valid_dimension(*height)
                    && (*width != art.width() || *height != art.height())
                {
                    changes.push(Self::PIXELS.reshape(
                        ObjectId::ROOT,
                        resized(art.bounds, *width, *height, *anchor),
                    ));
                }
            }
        }
        if changes.len() == 1 && self.pixels.bounds().area() == 0 {
            changes.clear();
        }
        Edit(changes)
    }
}

fn paint(cells: Vec<(i32, i32, [u8; 4])>) -> Option<Change> {
    (!cells.is_empty()).then(|| PixelArtDocument::PIXELS.paint(ObjectId::ROOT, cells))
}

fn recolor(
    art: &Artwork,
    change: impl Fn(PixelColor) -> Option<PixelColor>,
) -> Vec<(i32, i32, [u8; 4])> {
    let mut cells = Vec::new();
    for y in 0..art.height() {
        for x in 0..art.width() {
            let Some(color) = art.pixel(x, y).and_then(&change) else {
                continue;
            };
            let (at_x, at_y) = art.at(x, y);
            cells.push((at_x, at_y, color.rgba()));
        }
    }
    cells
}

fn fill(art: &Artwork, x: u16, y: u16, color: PixelColor) -> Vec<(i32, i32, [u8; 4])> {
    let Some(target) = art.pixel(x, y) else {
        return Vec::new();
    };
    if target == color {
        return Vec::new();
    }
    let (width, height) = (art.width(), art.height());
    let mut seen = vec![false; usize::from(width) * usize::from(height)];
    let mut pending = vec![(x, y)];
    let mut cells = Vec::new();
    seen[usize::from(y) * usize::from(width) + usize::from(x)] = true;
    while let Some((pixel_x, pixel_y)) = pending.pop() {
        let (at_x, at_y) = art.at(pixel_x, pixel_y);
        cells.push((at_x, at_y, color.rgba()));
        let neighbors = [
            pixel_x.checked_sub(1).map(|left| (left, pixel_y)),
            (pixel_x + 1 < width).then_some((pixel_x + 1, pixel_y)),
            pixel_y.checked_sub(1).map(|up| (pixel_x, up)),
            (pixel_y + 1 < height).then_some((pixel_x, pixel_y + 1)),
        ];
        for (next_x, next_y) in neighbors.into_iter().flatten() {
            let index = usize::from(next_y) * usize::from(width) + usize::from(next_x);
            if !seen[index] && art.pixel(next_x, next_y) == Some(target) {
                seen[index] = true;
                pending.push((next_x, next_y));
            }
        }
    }
    cells
}

fn resized(bounds: Bounds, width: u16, height: u16, anchor: PixelArtAnchor) -> Bounds {
    let (old_width, old_height) = (
        u16::try_from(bounds.width).unwrap_or(MAX_PIXEL_ART_SIZE),
        u16::try_from(bounds.height).unwrap_or(MAX_PIXEL_ART_SIZE),
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

fn default_bounds() -> Bounds {
    Bounds::new(
        0,
        0,
        u32::from(DEFAULT_PIXEL_ART_SIZE),
        u32::from(DEFAULT_PIXEL_ART_SIZE),
    )
}

impl Root for PixelArtDocument {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7069_7865_6c2d_6172_742d_636f_6e74_0002);
}

pub type PixelArtContent = Document<PixelArtDocument>;

pub fn size_of(content: &PixelArtContent) -> (u16, u16) {
    let bounds = content
        .tree()
        .object(ObjectId::ROOT)
        .and_then(|root| {
            root.fields()
                .get(usize::from(PixelArtDocument::PIXELS.index()))
        })
        .and_then(|value| match value {
            Value::Grid(cells) => Some(cells.bounds()),
            _ => None,
        })
        .filter(|bounds| bounds.area() > 0)
        .unwrap_or_else(default_bounds);
    (
        u16::try_from(bounds.width).unwrap_or(MAX_PIXEL_ART_SIZE),
        u16::try_from(bounds.height).unwrap_or(MAX_PIXEL_ART_SIZE),
    )
}

pub fn default_palette() -> Vec<PixelColor> {
    [
        PixelColor::TRANSPARENT,
        PixelColor::new(0, 0, 0, 255),
        PixelColor::new(255, 255, 255, 255),
        PixelColor::new(128, 128, 128, 255),
        PixelColor::new(192, 192, 192, 255),
        PixelColor::new(136, 57, 50, 255),
        PixelColor::new(237, 118, 20, 255),
        PixelColor::new(255, 215, 0, 255),
        PixelColor::new(51, 160, 44, 255),
        PixelColor::new(40, 200, 170, 255),
        PixelColor::new(40, 110, 220, 255),
        PixelColor::new(95, 60, 190, 255),
        PixelColor::new(190, 60, 190, 255),
        PixelColor::new(230, 90, 120, 255),
        PixelColor::new(115, 75, 45, 255),
        PixelColor::new(242, 194, 156, 255),
    ]
    .to_vec()
}

fn normalized_palette(colors: &[PixelColor]) -> Vec<PixelColor> {
    let mut palette = Vec::with_capacity(colors.len().min(MAX_PIXEL_ART_PALETTE_COLORS));
    for &color in colors {
        if palette.len() == MAX_PIXEL_ART_PALETTE_COLORS {
            break;
        }
        if !palette.contains(&color) {
            palette.push(color);
        }
    }
    palette
}

#[derive(Clone, Copy)]
enum Alignment {
    Start,
    Center,
    End,
}

fn horizontal_alignment(anchor: PixelArtAnchor) -> Alignment {
    match anchor {
        PixelArtAnchor::TopLeft | PixelArtAnchor::Left | PixelArtAnchor::BottomLeft => {
            Alignment::Start
        }
        PixelArtAnchor::Top | PixelArtAnchor::Center | PixelArtAnchor::Bottom => Alignment::Center,
        PixelArtAnchor::TopRight | PixelArtAnchor::Right | PixelArtAnchor::BottomRight => {
            Alignment::End
        }
    }
}

fn vertical_alignment(anchor: PixelArtAnchor) -> Alignment {
    match anchor {
        PixelArtAnchor::TopLeft | PixelArtAnchor::Top | PixelArtAnchor::TopRight => {
            Alignment::Start
        }
        PixelArtAnchor::Left | PixelArtAnchor::Center | PixelArtAnchor::Right => Alignment::Center,
        PixelArtAnchor::BottomLeft | PixelArtAnchor::Bottom | PixelArtAnchor::BottomRight => {
            Alignment::End
        }
    }
}

fn aligned_offsets(old: u16, new: u16, overlap: u16, alignment: Alignment) -> (u16, u16) {
    match alignment {
        Alignment::Start => (0, 0),
        Alignment::Center => ((old - overlap) / 2, (new - overlap) / 2),
        Alignment::End => (old - overlap, new - overlap),
    }
}

const fn valid_dimension(value: u16) -> bool {
    value >= 1 && value <= MAX_PIXEL_ART_SIZE
}
