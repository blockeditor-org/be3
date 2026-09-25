use image::{Rgba, RgbaImage};

const CHANGED: [u8; 4] = [255, 0, 255, 255];

pub struct Highlight {
    pub image: RgbaImage,
    pub changed: u64,
}

pub fn highlight(before: &RgbaImage, after: &RgbaImage) -> Highlight {
    let width = before.width().max(after.width());
    let height = before.height().max(after.height());
    let mut changed = 0;
    let image = RgbaImage::from_fn(width, height, |x, y| {
        let before = before.get_pixel_checked(x, y);
        let after = after.get_pixel_checked(x, y);
        match (before, after) {
            (Some(before), Some(after)) if before == after => faded(after.0),
            _ => {
                changed += 1;
                Rgba(CHANGED)
            }
        }
    });
    Highlight { image, changed }
}

fn faded([red, green, blue, _]: [u8; 4]) -> Rgba<u8> {
    let luma = (u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114) / 1000;
    let grey = (40 + luma * 2 / 5) as u8;
    Rgba([grey, grey, grey, 255])
}
