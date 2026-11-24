use std::io::Cursor;

use skrifa::bitmap::{BitmapData, BitmapGlyph, Origin};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId as FontGlyph, MetadataProvider};

use beui_core::font::GlyphImage;
use zeno::Scratch;

use crate::outline::Outline;

/// The palette entry COLR uses for the text's own colour. beui does not pass it down, so
/// such a layer is drawn black, as FreeType draws it by default.
const FOREGROUND: u16 = 0xffff;

/// The layers of a COLR v0 glyph, bottom first, each with its colour from the first
/// palette. None for a glyph without them; a COLR v1 glyph is drawn as its plain outline.
pub(crate) fn layers(font: &FontRef, glyph: FontGlyph) -> Option<Vec<(FontGlyph, [u8; 4])>> {
    let colr = font.colr().ok()?;
    let range = colr.v0_base_glyph(glyph).ok()??;
    let cpal = font.cpal().ok();
    let records = cpal
        .as_ref()
        .and_then(|cpal| cpal.color_records_array()?.ok())
        .unwrap_or_default();
    let first = cpal
        .as_ref()
        .and_then(|cpal| cpal.color_record_indices().first())
        .map_or(0, |index| usize::from(index.get()));
    range
        .map(|index| {
            let (layer, palette) = colr.v0_layer(index).ok()?;
            let color = match palette {
                FOREGROUND => [0, 0, 0, 255],
                entry => records
                    .get(first + usize::from(entry))
                    .map_or([0, 0, 0, 255], |record| {
                        [record.red, record.green, record.blue, record.alpha]
                    }),
            };
            Some((FontGlyph::from(layer), color))
        })
        .collect()
}

/// Layers filled and laid over each other in their colours, as one colour glyph.
pub(crate) fn paint_layers(layers: &[(Outline, [u8; 4])], scratch: &mut Scratch) -> GlyphImage {
    let filled: Vec<_> = layers
        .iter()
        .map(|(outline, color)| (outline.fill(scratch), *color))
        .filter(|((_, placement), _)| placement.width > 0 && placement.height > 0)
        .collect();
    let Some(left) = filled.iter().map(|((_, at), _)| at.left).min() else {
        return GlyphImage {
            width: 0,
            height: 0,
            left: 0,
            top: 0,
            pixels: Vec::new(),
            color: true,
        };
    };
    let top = filled.iter().map(|((_, at), _)| at.top).min().unwrap_or(0);
    let right = filled
        .iter()
        .map(|((_, at), _)| at.left + at.width as i32)
        .max()
        .unwrap_or(left);
    let bottom = filled
        .iter()
        .map(|((_, at), _)| at.top + at.height as i32)
        .max()
        .unwrap_or(top);
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    let mut premultiplied = vec![0.0f32; width as usize * height as usize * 4];
    for ((mask, at), color) in &filled {
        for row in 0..at.height as usize {
            for column in 0..at.width as usize {
                let coverage = f32::from(mask[row * at.width as usize + column]) / 255.0;
                let alpha = coverage * f32::from(color[3]) / 255.0;
                if alpha == 0.0 {
                    continue;
                }
                let x = (at.left - left) as usize + column;
                let y = (at.top - top) as usize + row;
                let pixel = &mut premultiplied[(y * width as usize + x) * 4..][..4];
                for (channel, value) in pixel.iter_mut().zip(color) {
                    let source = f32::from(*value) * alpha;
                    *channel = source + *channel * (1.0 - alpha);
                }
                pixel[3] = 255.0 * alpha + pixel[3] * (1.0 - alpha);
            }
        }
    }
    GlyphImage {
        width,
        height,
        left,
        top: -top,
        pixels: unpremultiply(&premultiplied),
        color: true,
    }
}

/// A glyph of a face made of bitmaps alone: from the smallest strike at least as large as
/// the text, or the largest there is, scaled to the text and then to the raster.
pub(crate) fn bitmap(
    font: &FontRef,
    glyph: FontGlyph,
    pixel_size: u32,
    raster: f32,
) -> Option<GlyphImage> {
    let (bitmap, scale) = strike_glyph(font, glyph, pixel_size)?;
    let (width, height, premultiplied) = match bitmap.data.clone() {
        BitmapData::Png(bytes) => png(bytes)?,
        BitmapData::Bgra(bytes) => {
            let premultiplied = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
                .map(f32::from)
                .collect::<Vec<_>>();
            (bitmap.width, bitmap.height, premultiplied)
        }
        BitmapData::Mask(_) => return None,
    };
    if premultiplied.len() != width as usize * height as usize * 4 {
        return None;
    }
    let (left, top) = bearings(&bitmap, height);
    Some(image(
        premultiplied,
        width,
        height,
        left.round() as i32,
        top.round() as i32,
        scale * raster,
    ))
}

/// Where a bitmap glyph draws at the text's size, y down: its strike's box scaled as
/// `bitmap` scales it.
pub(crate) fn bitmap_extents(font: &FontRef, glyph: FontGlyph, pixel_size: u32) -> Option<[f32; 4]> {
    let (bitmap, scale) = strike_glyph(font, glyph, pixel_size)?;
    let (left, top) = bearings(&bitmap, bitmap.height);
    let left = (left * scale).round();
    let top = -(top * scale).round();
    let width = (bitmap.width as f32 * scale).round().max(1.0);
    let height = (bitmap.height as f32 * scale).round().max(1.0);
    Some([left, top, left + width, top + height])
}

/// A glyph from the smallest strike at least as large as the text, or the largest there is,
/// and how much it is scaled to the text.
fn strike_glyph<'a>(
    font: &FontRef<'a>,
    glyph: FontGlyph,
    pixel_size: u32,
) -> Option<(BitmapGlyph<'a>, f32)> {
    let strikes = font.bitmap_strikes();
    let wanted = pixel_size as f32;
    let strike = strikes
        .iter()
        .filter(|strike| strike.ppem() >= wanted)
        .min_by(|a, b| a.ppem().total_cmp(&b.ppem()))
        .or_else(|| strikes.iter().max_by(|a, b| a.ppem().total_cmp(&b.ppem())))?;
    if strike.ppem() <= 0.0 {
        return None;
    }
    Some((strike.get(glyph)?, wanted / strike.ppem()))
}

/// A bitmap's left and top bearings at its strike's size, as FreeType reads them.
fn bearings(bitmap: &BitmapGlyph, height: u32) -> (f32, f32) {
    let top = match bitmap.placement_origin {
        Origin::TopLeft => bitmap.inner_bearing_y,
        Origin::BottomLeft => bitmap.inner_bearing_y + height as f32,
    };
    (bitmap.inner_bearing_x, top)
}

/// A PNG strike's pixels, premultiplied.
fn png(bytes: &[u8]) -> Option<(u32, u32, Vec<f32>)> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    let pixels = &buffer[..info.buffer_size()];
    let rgba: Vec<[u8; 4]> = match info.color_type {
        png::ColorType::Rgba => pixels.as_chunks::<4>().0.to_vec(),
        png::ColorType::GrayscaleAlpha => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[gray, alpha]| [*gray, *gray, *gray, *alpha])
            .collect(),
        _ => return None,
    };
    let premultiplied = rgba
        .iter()
        .flat_map(|pixel| {
            let alpha = f32::from(pixel[3]) / 255.0;
            [
                f32::from(pixel[0]) * alpha,
                f32::from(pixel[1]) * alpha,
                f32::from(pixel[2]) * alpha,
                f32::from(pixel[3]),
            ]
        })
        .collect();
    Some((info.width, info.height, premultiplied))
}

fn image(
    premultiplied: Vec<f32>,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    scale: f32,
) -> GlyphImage {
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
