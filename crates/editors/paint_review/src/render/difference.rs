use block_editor_plugin::beui::{Color32, Image};

use super::Painted;

const HIGHLIGHT: [f32; 3] = [255.0, 59.0, 92.0];
const MISSING: Color32 = Color32::from_rgb(96, 16, 34);
const PAPER: f32 = 236.0;
const GHOST: f32 = 0.22;
const TINT: f32 = 0.55;

pub fn difference(approved: &Image, current: &Image) -> Painted {
    let width = approved.width().max(current.width()) as usize;
    let height = approved.height().max(current.height()) as usize;
    let mut pixels = Vec::with_capacity(width * height * 4);
    let mut changed = 0usize;
    let mut region: Option<[usize; 4]> = None;

    for y in 0..height {
        for x in 0..width {
            let before = at(approved, x, y);
            let after = at(current, x, y);
            let pixel = match (before, after) {
                (Some(before), Some(after)) if before == after => ghost(after),
                (None, None) => MISSING,
                (_, Some(after)) => tinted(after),
                (Some(_), None) => MISSING,
            };
            if before != after {
                changed += 1;
                region = Some(match region {
                    None => [x, y, x, y],
                    Some(held) => [
                        held[0].min(x),
                        held[1].min(y),
                        held[2].max(x),
                        held[3].max(y),
                    ],
                });
            }
            pixels.extend_from_slice(&pixel.to_array());
        }
    }

    Painted {
        image: Image::from_rgba(width as u32, height as u32, pixels),
        description: describe(approved, current, changed, region),
    }
}

fn describe(
    approved: &Image,
    current: &Image,
    changed: usize,
    region: Option<[usize; 4]>,
) -> String {
    let resized = (approved.size() != current.size()).then(|| {
        format!(
            "the painting is {}x{}, it used to be {}x{}; ",
            current.width(),
            current.height(),
            approved.width(),
            approved.height()
        )
    });
    let Some([left, top, right, bottom]) = region else {
        return "these frames are the same, pixel for pixel".to_owned();
    };
    format!(
        "{}{changed} {} differ, in a {}x{} region at ({left}, {top})",
        resized.unwrap_or_default(),
        if changed == 1 { "pixel" } else { "pixels" },
        right - left + 1,
        bottom - top + 1,
    )
}

fn at(image: &Image, x: usize, y: usize) -> Option<Color32> {
    let width = image.width() as usize;
    if x >= width || y >= image.height() as usize {
        return None;
    }
    let start = (y * width + x) * 4;
    let pixel = image.pixels().get(start..start + 4)?;
    Some(Color32::from_rgba_unmultiplied(
        pixel[0], pixel[1], pixel[2], pixel[3],
    ))
}

fn ghost(pixel: Color32) -> Color32 {
    let [red, green, blue, _] = pixel.to_array();
    let luma = 0.299 * red as f32 + 0.587 * green as f32 + 0.114 * blue as f32;
    let value = PAPER + (luma - PAPER) * GHOST;
    Color32::from_gray(value.round().clamp(0.0, 255.0) as u8)
}

fn tinted(pixel: Color32) -> Color32 {
    let [red, green, blue, _] = pixel.to_array();
    let channels = [red, green, blue];
    let mixed = channels
        .iter()
        .zip(HIGHLIGHT)
        .map(|(channel, highlight)| {
            (*channel as f32 * (1.0 - TINT) + highlight * TINT)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect::<Vec<u8>>();
    Color32::from_rgb(mixed[0], mixed[1], mixed[2])
}
