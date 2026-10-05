use freetype::freetype as ft;

use beui_core::font::GlyphImage;

pub(crate) unsafe fn set_size(face: ft::FT_Face, pixel_size: u32) -> Option<f32> {
    unsafe {
        if ft::FT_Set_Pixel_Sizes(face, 0, pixel_size) == 0 {
            return Some(1.0);
        }
        let count = (*face).num_fixed_sizes.max(0) as usize;
        if count == 0 || (*face).available_sizes.is_null() {
            return None;
        }
        let sizes = std::slice::from_raw_parts((*face).available_sizes, count);
        let wanted = pixel_size as ft::FT_Pos * 64;
        let (strike, size) = sizes
            .iter()
            .enumerate()
            .filter(|(_, size)| size.y_ppem >= wanted)
            .min_by_key(|(_, size)| size.y_ppem)
            .or_else(|| sizes.iter().enumerate().max_by_key(|(_, size)| size.y_ppem))?;
        if size.y_ppem <= 0 || ft::FT_Select_Size(face, strike as ft::FT_Int) != 0 {
            return None;
        }
        Some(wanted as f32 / size.y_ppem as f32)
    }
}

pub(crate) fn image(bitmap: &ft::FT_Bitmap, left: i32, top: i32, scale: f32) -> GlyphImage {
    let (width, height) = (bitmap.width, bitmap.rows);
    let premultiplied = bgra(bitmap);
    if scale == 1.0 || width == 0 || height == 0 {
        return GlyphImage {
            width,
            height,
            left,
            top,
            pixels: unpremultiply(&premultiplied),
            color: true,
        };
    }
    let to_width = ((width as f32 * scale).round() as u32).max(1);
    let to_height = ((height as f32 * scale).round() as u32).max(1);
    let across = resample(&premultiplied, width, height, to_width, true);
    let resized = resample(&across, to_width, height, to_height, false);
    GlyphImage {
        width: to_width,
        height: to_height,
        left: (left as f32 * scale).round() as i32,
        top: (top as f32 * scale).round() as i32,
        pixels: unpremultiply(&resized),
        color: true,
    }
}

fn bgra(bitmap: &ft::FT_Bitmap) -> Vec<f32> {
    let width = bitmap.width as usize;
    let rows = bitmap.rows as usize;
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    if bitmap.buffer.is_null() || width == 0 || rows == 0 || pitch < width * 4 {
        return vec![0.0; width * rows * 4];
    }
    let buffer = unsafe { std::slice::from_raw_parts(bitmap.buffer, pitch * rows) };
    let mut premultiplied = Vec::with_capacity(width * rows * 4);
    for row in 0..rows {
        let source = if bitmap.pitch >= 0 {
            row
        } else {
            rows - 1 - row
        };
        let line = &buffer[source * pitch..source * pitch + width * 4];
        premultiplied.extend(
            line.as_chunks::<4>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
                .map(f32::from),
        );
    }
    premultiplied
}

fn resample(pixels: &[f32], width: u32, height: u32, to: u32, horizontal: bool) -> Vec<f32> {
    let (from, lines, out_width) = match horizontal {
        true => (width, height, to),
        false => (height, width, width),
    };
    let out_height = match horizontal {
        true => height,
        false => to,
    };
    let step = from as f32 / to as f32;
    let mut out = vec![0.0; out_width as usize * out_height as usize * 4];
    for line in 0..lines {
        for target in 0..to {
            let start = target as f32 * step;
            let end = start + step;
            let mut sum = [0.0f32; 4];
            let mut weight = 0.0;
            let mut source = start.floor() as u32;
            while (source as f32) < end && source < from {
                let covered = (end.min(source as f32 + 1.0) - start.max(source as f32)).max(0.0);
                let (x, y) = match horizontal {
                    true => (source, line),
                    false => (line, source),
                };
                let at = (y as usize * width as usize + x as usize) * 4;
                for (sum, value) in sum.iter_mut().zip(&pixels[at..at + 4]) {
                    *sum += value * covered;
                }
                weight += covered;
                source += 1;
            }
            let (x, y) = match horizontal {
                true => (target, line),
                false => (line, target),
            };
            let at = (y as usize * out_width as usize + x as usize) * 4;
            if weight > 0.0 {
                for (out, sum) in out[at..at + 4].iter_mut().zip(sum) {
                    *out = sum / weight;
                }
            }
        }
    }
    out
}

fn unpremultiply(pixels: &[f32]) -> Vec<u8> {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| {
            let alpha = pixel[3];
            let channel = |value: f32| match alpha > 0.0 {
                true => (value * 255.0 / alpha).round().clamp(0.0, 255.0) as u8,
                false => 0,
            };
            [
                channel(pixel[0]),
                channel(pixel[1]),
                channel(pixel[2]),
                alpha.round().clamp(0.0, 255.0) as u8,
            ]
        })
        .collect()
}
