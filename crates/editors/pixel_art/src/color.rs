use block_client::blocks::pixel_art::{PixelArt, PixelColor};
use block_editor_plugin::beui::{Color32, Image};

pub fn artwork_image(art: &PixelArt, dark_mode: bool) -> Image {
    let (light, dark) = checkerboard_colors(dark_mode);
    let width = usize::from(art.width());
    let height = usize::from(art.height());
    let bytes = art.rgba_bytes();
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let background = if (x + y) % 2 == 0 { light } else { dark };
            let offset = (y * width + x) * 4;
            let rgba = &bytes[offset..offset + 4];
            let composited = composite_pixel(
                PixelColor::new(rgba[0], rgba[1], rgba[2], rgba[3]),
                background,
            );
            pixels.extend_from_slice(&composited);
        }
    }
    Image::from_rgba(art.width().into(), art.height().into(), pixels)
}

pub fn preview_image(
    pixels: &[(u16, u16)],
    color: PixelColor,
    bounds: (u16, u16, u16, u16),
    dark_mode: bool,
) -> Image {
    let (left, top, right, bottom) = bounds;
    let width = usize::from(right - left + 1);
    let height = usize::from(bottom - top + 1);
    let (light, dark) = checkerboard_colors(dark_mode);
    let mut rgba = vec![0u8; width * height * 4];
    for &(x, y) in pixels {
        if x < left || x > right || y < top || y > bottom {
            continue;
        }
        let offset = ((usize::from(y - top)) * width + usize::from(x - left)) * 4;
        let background = if (usize::from(x) + usize::from(y)) % 2 == 0 {
            light
        } else {
            dark
        };
        rgba[offset..offset + 4].copy_from_slice(&composite_pixel(color, background));
    }
    Image::from_rgba(width as u32, height as u32, rgba)
}

pub fn preview_bounds(pixels: &[(u16, u16)]) -> Option<(u16, u16, u16, u16)> {
    let mut bounds: Option<(u16, u16, u16, u16)> = None;
    for &(x, y) in pixels {
        bounds = Some(match bounds {
            None => (x, y, x, y),
            Some((left, top, right, bottom)) => {
                (left.min(x), top.min(y), right.max(x), bottom.max(y))
            }
        });
    }
    bounds
}

pub fn checkerboard_colors(dark_mode: bool) -> ([u8; 3], [u8; 3]) {
    if dark_mode {
        ([82, 82, 82], [58, 58, 58])
    } else {
        ([232, 232, 232], [202, 202, 202])
    }
}

pub fn composite_pixel(color: PixelColor, background: [u8; 3]) -> [u8; 4] {
    let alpha = u16::from(color.alpha);
    let inverse = 255 - alpha;
    [
        ((u16::from(color.red) * alpha + u16::from(background[0]) * inverse) / 255) as u8,
        ((u16::from(color.green) * alpha + u16::from(background[1]) * inverse) / 255) as u8,
        ((u16::from(color.blue) * alpha + u16::from(background[2]) * inverse) / 255) as u8,
        255,
    ]
}

pub fn swatch_color(color: PixelColor) -> Color32 {
    Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha)
}

pub fn format_hex_color(color: PixelColor) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue, color.alpha
    )
}

pub fn parse_hex_color(value: &str) -> Option<PixelColor> {
    let value = value.strip_prefix('#')?;
    if value.len() != 8 || !value.is_ascii() {
        return None;
    }
    Some(PixelColor::new(
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
        u8::from_str_radix(&value[6..8], 16).ok()?,
    ))
}
