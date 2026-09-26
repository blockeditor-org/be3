use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::geometry::Vec2;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

const THUMBHASH_SIDE: u32 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ImageId(u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ImageFit {
    #[default]
    Contain,
    Cover,
    Fill,
    ScaleDown,
}

struct ImageData {
    id: ImageId,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone)]
pub struct Image(Arc<ImageData>);

impl Image {
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        assert_eq!(
            pixels.len(),
            width as usize * height as usize * 4,
            "an rgba image carries four bytes per pixel"
        );
        Self(Arc::new(ImageData {
            id: ImageId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            width,
            height,
            pixels,
        }))
    }

    pub fn from_thumbhash(hash: &[u8]) -> Option<Self> {
        let (width, height, pixels) = thumbhash::thumb_hash_to_rgba(hash).ok()?;
        (width > 0 && height > 0).then(|| Self::from_rgba(width as u32, height as u32, pixels))
    }

    pub fn thumbhash(&self) -> Vec<u8> {
        let (width, height) = (self.0.width, self.0.height);
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let scale = (THUMBHASH_SIDE as f32 / width.max(height) as f32).min(1.0);
        let to_width = ((width as f32 * scale).round() as u32).clamp(1, THUMBHASH_SIDE);
        let to_height = ((height as f32 * scale).round() as u32).clamp(1, THUMBHASH_SIDE);
        let pixels = shrink(&self.0.pixels, width, height, to_width, to_height);
        thumbhash::rgba_to_thumb_hash(to_width as usize, to_height as usize, &pixels)
    }

    pub fn id(&self) -> ImageId {
        self.0.id
    }

    pub fn width(&self) -> u32 {
        self.0.width
    }

    pub fn height(&self) -> u32 {
        self.0.height
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.0.width as f32, self.0.height as f32)
    }

    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }
}

fn shrink(pixels: &[u8], width: u32, height: u32, to_width: u32, to_height: u32) -> Vec<u8> {
    if (width, height) == (to_width, to_height) {
        return pixels.to_vec();
    }
    let mut sums = vec![[0u64; 5]; to_width as usize * to_height as usize];
    for y in 0..height {
        let row = (u64::from(y) * u64::from(to_height) / u64::from(height)) as usize;
        for x in 0..width {
            let column = (u64::from(x) * u64::from(to_width) / u64::from(width)) as usize;
            let at = (y as usize * width as usize + x as usize) * 4;
            let alpha = u64::from(pixels[at + 3]);
            let sum = &mut sums[row * to_width as usize + column];
            for channel in 0..3 {
                sum[channel] += u64::from(pixels[at + channel]) * alpha;
            }
            sum[3] += alpha;
            sum[4] += 1;
        }
    }
    sums.iter()
        .flat_map(|[red, green, blue, alpha, count]| {
            let colour = |channel: u64| match *alpha {
                0 => 0,
                alpha => (channel / alpha) as u8,
            };
            let coverage = match *count {
                0 => 0,
                count => (alpha / count) as u8,
            };
            [colour(*red), colour(*green), colour(*blue), coverage]
        })
        .collect()
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}

impl std::fmt::Debug for Image {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Image")
            .field("id", &self.0.id.0)
            .field("width", &self.0.width)
            .field("height", &self.0.height)
            .finish()
    }
}

impl ImageFit {
    pub fn place(self, into: crate::geometry::Rect, source: Vec2) -> crate::geometry::Rect {
        if source.x <= 0.0 || source.y <= 0.0 || !into.is_positive() {
            return into;
        }
        let scale = match self {
            Self::Fill => return into,
            Self::Contain => (into.width() / source.x).min(into.height() / source.y),
            Self::Cover => (into.width() / source.x).max(into.height() / source.y),
            Self::ScaleDown => (into.width() / source.x)
                .min(into.height() / source.y)
                .min(1.0),
        };
        let size = Vec2::new(source.x * scale, source.y * scale);
        crate::geometry::Rect::from_min_size(
            crate::geometry::Pos2::new(
                into.left() + (into.width() - size.x) / 2.0,
                into.top() + (into.height() - size.y) / 2.0,
            ),
            size,
        )
    }
}
