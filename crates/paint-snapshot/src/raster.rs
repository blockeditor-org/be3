use std::collections::BTreeMap;

use image::RgbaImage;
use paint_format::Pixels;

use crate::{Frame, Snapshot};

pub fn render(snapshot: &Snapshot, frame: usize) -> Result<RgbaImage, String> {
    let frame: &Frame = snapshot.frame(frame)?;
    let mut textures = BTreeMap::new();
    for (key, texture) in &snapshot.textures {
        textures.insert(
            *key,
            Pixels {
                size: texture.size,
                rgba: texture.pixels()?,
            },
        );
    }
    let background = frame.background;
    let pixels = paint_format::rasterize(
        frame,
        [background[0], background[1], background[2], 255],
        |key| textures.get(&key),
    )?;
    RgbaImage::from_vec(
        pixels.size[0],
        pixels.size[1],
        pixels.rgba.into_iter().flatten().collect(),
    )
    .ok_or_else(|| "the rasterized painting does not match its size".to_owned())
}
